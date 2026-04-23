//! LLVM 后端（inkwell）——可回归的最小 codegen 落点（T0802～T0810）。
//!
//! 当前阶段目标：
//! 1) 初始化 host target（target triple + data layout）。
//! 2) 生成一个 LLVM module，包含入口 `i32 @main(i32 argc, i8** argv)`（C ABI）：
//!    - 若源文件中存在顶层 `fun main`，则对其 body 做早期子集 codegen，并将返回值作为进程退出码；
//!    - 同时生成/声明 `main` 调用到的顶层函数（T0810：先按简单 C ABI）。
//!
//! 说明：
//! - 目前仍只支持“表达式/语句最小子集”；复杂控制流需要 MIR/CFG codegen（后续任务）。
//! - 目前仍只生成单个 LLVM module，但该 module 已可覆盖整个 compilation unit；
//!   多文件顶层值读取与跨文件泛型实例化走同一模块内的 codegen 主线。
//! - 尚未做多模块拆分与真正的链接管理（后续任务）。

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use inkwell::context::Context;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::FileType;
use inkwell::targets::TargetData;
use inkwell::values::InstructionValueError;
use miette::{Diagnostic, NamedSource};
use thiserror::Error;

use crate::ast;
use crate::hir;
use crate::opt::OptLevel;
use crate::parser::ParseError;
use crate::session::Session;
use crate::source::{SourceFile, SourceId, SourceMap};
use crate::span::Span;

mod codegen;
mod frontend;
mod stackmap;
mod target;
pub use target::{HostTargetInfo, LlvmTargetError};

/// LLVM statepoint GC 策略名（内置于 LLVM）。
///
/// 说明：
/// - `rewrite-statepoints-for-gc` 只会重写带 `gc "<strategy>"` 的函数；
/// - 当前阶段先复用 LLVM 内置的 `statepoint-example`，后续若需要更精细的 roots 策略再引入自定义 GC strategy。
pub(crate) const LLVM_GC_STRATEGY_STATEPOINT_EXAMPLE: &str = "statepoint-example";

fn configure_llvm_global_options_once() {
    // 说明：
    // - 我们使用 LLVM statepoint + stackmap 做 moving GC roots 枚举/更新；
    // - runtime 目前只支持更新“可写回的 spill slots roots”（栈槽 `void**`），不支持把 GC 指针放在寄存器里；
    // - LLVM 后端在某些情况下会把 GC pointers 放进 callee-saved registers，并依赖
    //   `fixup-statepoint-caller-saved` 等机器层 pass 做额外处理；
    // - 在 `SCOOP_GC_STRESS=1` 下（频繁触发 compaction），若存在寄存器 roots，
    //   可能导致 roots 未被更新而产生 use-after-move（典型症状：值“偶发变回 None/0”，T1606c）。
    //
    // v0 策略：强制禁用 “GC Ptrs in registers”，让所有 roots 走 spill slots，从而匹配 runtime 能力边界。
    #[cfg(feature = "llvm")]
    {
        use std::sync::Once;

        static ONCE: Once = Once::new();
        ONCE.call_once(|| unsafe {
            use std::ffi::c_char;

            let arg0 = b"scoopc\0";
            let arg1 = b"-fixup-max-csr-statepoints=0\0";
            let args: [*const c_char; 2] = [arg0.as_ptr().cast(), arg1.as_ptr().cast()];

            // Safety: LLVM global CLI options; must be called before codegen.
            llvm_sys::support::LLVMParseCommandLineOptions(
                args.len() as i32,
                args.as_ptr(),
                std::ptr::null(),
            );
        });
    }
}

/// LLVM codegen（早期阶段）的错误集合。
#[derive(Debug, Error, Diagnostic)]
pub enum LlvmEmitError {
    #[error("LLVM 单文件前端准备失败：{message}")]
    #[diagnostic(code(scoop::llvm::frontend_prepare_failed))]
    Frontend { message: String },

    #[error(transparent)]
    #[diagnostic(transparent)]
    Parse(#[from] ParseError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    HirLower(#[from] hir::HirLowerError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Target(#[from] LlvmTargetError),

    #[error("LLVM IR 构造失败：{0}")]
    #[diagnostic(code(scoop::llvm::builder_error))]
    Builder(#[from] inkwell::builder::BuilderError),

    #[error("LLVM 指令构造失败：{0}")]
    #[diagnostic(code(scoop::llvm::instruction_error))]
    Instruction(#[from] InstructionValueError),

    #[error("找不到入口函数 `main`（当前阶段仅支持顶层 `fun main() {{ ... }}`）")]
    #[diagnostic(code(scoop::llvm::missing_entry_main))]
    MissingEntryMain,

    #[error("暂不支持的 main 代码生成节点：{kind}")]
    #[diagnostic(code(scoop::llvm::unsupported_main_body))]
    UnsupportedMainBody {
        kind: &'static str,
        #[label("这里")]
        at: miette::SourceSpan,
    },

    #[error("字面量解析失败：{kind}（{file}:{line}:{column}，原文：{text}，原因：{reason}）")]
    #[diagnostic(code(scoop::llvm::invalid_literal))]
    InvalidLiteral {
        kind: &'static str,
        reason: &'static str,
        file: String,
        line: usize,
        column: usize,
        text: String,
        #[source_code]
        src: Arc<NamedSource<String>>,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("LLVM module 校验失败：{message}")]
    #[diagnostic(code(scoop::llvm::module_verification_failed))]
    ModuleVerificationFailed { message: String },

    #[error("运行 LLVM pass 失败（passes={passes}）：{message}")]
    #[diagnostic(code(scoop::llvm::run_passes_failed))]
    RunPassesFailed { passes: String, message: String },

    #[error("写入 LLVM IR 失败：{path}: {source}")]
    #[diagnostic(code(scoop::llvm::write_ll_failed))]
    WriteLlFailed {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("输出路径不是有效 UTF-8：{path}")]
    #[diagnostic(code(scoop::llvm::invalid_output_path))]
    InvalidOutputPath { path: PathBuf },

    #[error("写入 object 文件失败：{path}: {message}")]
    #[diagnostic(code(scoop::llvm::write_obj_failed))]
    WriteObjFailed { path: PathBuf, message: String },

    #[error("写入 assembly 文件失败：{path}: {message}")]
    #[diagnostic(code(scoop::llvm::write_asm_failed))]
    WriteAsmFailed { path: PathBuf, message: String },
}

impl LlvmEmitError {
    pub(crate) fn invalid_literal(
        source: &SourceFile,
        span: Span,
        kind: &'static str,
        reason: &'static str,
        text: &str,
    ) -> Self {
        let file = diagnostic_source_name(source.path());
        let (line, column) = source.offset_to_line_col(span.start).unwrap_or((1, 1));
        Self::InvalidLiteral {
            kind,
            reason,
            file: file.clone(),
            line,
            column,
            text: literal_text_preview(text),
            src: Arc::new(NamedSource::new(file, source.text().to_string())),
            span: span.into(),
        }
    }
}

/// 为一个 Scoop 程序生成 LLVM IR（`.ll` 文本）。
///
/// 当前阶段（T0808）的输出形态：
/// - 一个 LLVM module（module name 取决于输入文件名）；
/// - module target triple / data layout 设为 host；
/// - `i32 @main(i32 argc, i8** argv)` 的 body 来自 `fun main` 的 v1 子集 codegen；若 `main` 为空则返回 0。
pub fn emit_minimal_main_ir(
    session: &Session,
    source: &SourceFile,
) -> Result<String, LlvmEmitError> {
    let context = Context::create();
    let module = build_minimal_main_module(session, source, &context)?;
    Ok(module.print_to_string().to_string())
}

/// 基于“已完成 resolver 的 AST lowering 结果”（`hir::LoweredHir`）生成 LLVM IR。
///
/// 用途（T1107）：
/// - `scoop build` 在多包（cone 依赖）场景下，需要复用同一套“已注入 `.cone` 依赖”的编译单元，
///   避免后端再次独立 parse/resolve 导致 import 失败或语义分叉。
pub fn emit_minimal_main_ir_from_lowered_hir(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    lowered: &hir::LoweredHir,
) -> Result<String, LlvmEmitError> {
    let context = Context::create();
    let module =
        build_main_module_from_lowered_hir(source_map, entry_source_id, &context, lowered, None)?;
    Ok(module.print_to_string().to_string())
}

/// 生成最小 LLVM IR，并写入到指定路径（通常为 `.ll`）。
pub fn emit_minimal_main_ir_to_file(
    session: &Session,
    source: &SourceFile,
    output: &Path,
) -> Result<(), LlvmEmitError> {
    let ir = emit_minimal_main_ir(session, source)?;

    std::fs::write(output, ir).map_err(|e| LlvmEmitError::WriteLlFailed {
        path: output.to_path_buf(),
        source: e,
    })?;

    Ok(())
}

/// 基于 `hir::LoweredHir` 生成最小 LLVM IR，并写入到指定路径（通常为 `.ll`）。
pub fn emit_minimal_main_ir_to_file_from_lowered_hir(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    lowered: &hir::LoweredHir,
    output: &Path,
) -> Result<(), LlvmEmitError> {
    let ir = emit_minimal_main_ir_from_lowered_hir(source_map, entry_source_id, lowered)?;
    std::fs::write(output, ir).map_err(|e| LlvmEmitError::WriteLlFailed {
        path: output.to_path_buf(),
        source: e,
    })?;
    Ok(())
}

/// 基于 `hir::LoweredHir` 生成最小 LLVM IR，并写入到指定路径（允许显式指定入口 `main` 的 FQN）。
pub fn emit_minimal_main_ir_to_file_from_lowered_hir_with_entry(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    lowered: &hir::LoweredHir,
    output: &Path,
    entry_main_fqn: Option<&str>,
) -> Result<(), LlvmEmitError> {
    let context = Context::create();
    let module = build_main_module_from_lowered_hir(
        source_map,
        entry_source_id,
        &context,
        lowered,
        entry_main_fqn,
    )?;
    let ir = module.print_to_string().to_string();

    std::fs::write(output, ir).map_err(|e| LlvmEmitError::WriteLlFailed {
        path: output.to_path_buf(),
        source: e,
    })?;
    Ok(())
}

/// 基于 `hir::LoweredHir` 生成最小 LLVM IR，并写入到指定路径（允许显式指定入口 `main` 的 FQN）。
///
/// 与 `emit_minimal_main_ir_to_file_from_lowered_hir_with_entry` 的区别：
/// - 该版本会按 `opt_level` 运行 LLVM PassBuilder pipeline（包含 statepoint 重写），确保 `--emit-llvm`
///   的输出能反映优化等级差异，便于 build fixtures 断言与回归。
pub fn emit_minimal_main_ir_to_file_from_lowered_hir_with_entry_with_opt_level(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    lowered: &hir::LoweredHir,
    output: &Path,
    entry_main_fqn: Option<&str>,
    opt_level: OptLevel,
) -> Result<(), LlvmEmitError> {
    let context = Context::create();
    let module = build_main_module_from_lowered_hir(
        source_map,
        entry_source_id,
        &context,
        lowered,
        entry_main_fqn,
    )?;

    let (target_machine, _target_info) = target::host_target_machine_with_opt_level(opt_level)?;
    run_pass_pipeline(&module, &target_machine, opt_level)?;

    let ir = module.print_to_string().to_string();
    std::fs::write(output, ir).map_err(|e| LlvmEmitError::WriteLlFailed {
        path: output.to_path_buf(),
        source: e,
    })?;
    Ok(())
}

/// 生成最小 LLVM object，并写入到指定路径（通常为 `.o`）。
pub fn emit_minimal_main_obj_to_file(
    session: &Session,
    source: &SourceFile,
    output: &Path,
) -> Result<(), LlvmEmitError> {
    emit_minimal_main_obj_to_file_with_opt_level(session, source, output, OptLevel::O0)
}

/// 生成最小 LLVM object，并写入到指定路径（通常为 `.o`）。
pub fn emit_minimal_main_obj_to_file_with_opt_level(
    session: &Session,
    source: &SourceFile,
    output: &Path,
    opt_level: OptLevel,
) -> Result<(), LlvmEmitError> {
    // `TargetMachine::write_to_file` 内部会 `path.to_str().expect(...)`，为了避免 panic，
    // 这里提前做 UTF-8 校验并返回结构化诊断。
    if output.to_str().is_none() {
        return Err(LlvmEmitError::InvalidOutputPath {
            path: output.to_path_buf(),
        });
    }

    let context = Context::create();
    let module = build_minimal_main_module(session, source, &context)?;

    let (target_machine, _target_info) = target::host_target_machine_with_opt_level(opt_level)?;
    run_pass_pipeline(&module, &target_machine, opt_level)?;
    target_machine
        .write_to_file(&module, FileType::Object, output)
        .map_err(|e| LlvmEmitError::WriteObjFailed {
            path: output.to_path_buf(),
            message: e.to_string(),
        })?;

    Ok(())
}

/// 基于 `hir::LoweredHir` 生成最小 LLVM object，并写入到指定路径（通常为 `.o`）。
pub fn emit_minimal_main_obj_to_file_from_lowered_hir(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    lowered: &hir::LoweredHir,
    output: &Path,
) -> Result<(), LlvmEmitError> {
    emit_minimal_main_obj_to_file_from_lowered_hir_with_opt_level(
        source_map,
        entry_source_id,
        lowered,
        output,
        OptLevel::O0,
    )
}

/// 基于 `hir::LoweredHir` 生成最小 LLVM object，并写入到指定路径（通常为 `.o`）。
pub fn emit_minimal_main_obj_to_file_from_lowered_hir_with_opt_level(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    lowered: &hir::LoweredHir,
    output: &Path,
    opt_level: OptLevel,
) -> Result<(), LlvmEmitError> {
    if output.to_str().is_none() {
        return Err(LlvmEmitError::InvalidOutputPath {
            path: output.to_path_buf(),
        });
    }

    let context = Context::create();
    let module =
        build_main_module_from_lowered_hir(source_map, entry_source_id, &context, lowered, None)?;

    let (target_machine, _target_info) = target::host_target_machine_with_opt_level(opt_level)?;
    run_pass_pipeline(&module, &target_machine, opt_level)?;
    target_machine
        .write_to_file(&module, FileType::Object, output)
        .map_err(|e| LlvmEmitError::WriteObjFailed {
            path: output.to_path_buf(),
            message: e.to_string(),
        })?;
    Ok(())
}

/// 基于 `hir::LoweredHir` 生成最小 LLVM object，并写入到指定路径（允许显式指定入口 `main` 的 FQN）。
pub fn emit_minimal_main_obj_to_file_from_lowered_hir_with_entry(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    lowered: &hir::LoweredHir,
    output: &Path,
    entry_main_fqn: Option<&str>,
) -> Result<(), LlvmEmitError> {
    emit_minimal_main_obj_to_file_from_lowered_hir_with_entry_with_opt_level(
        source_map,
        entry_source_id,
        lowered,
        output,
        entry_main_fqn,
        OptLevel::O0,
    )
}

/// 基于 `hir::LoweredHir` 生成最小 LLVM object，并写入到指定路径（允许显式指定入口 `main` 的 FQN）。
pub fn emit_minimal_main_obj_to_file_from_lowered_hir_with_entry_with_opt_level(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    lowered: &hir::LoweredHir,
    output: &Path,
    entry_main_fqn: Option<&str>,
    opt_level: OptLevel,
) -> Result<(), LlvmEmitError> {
    if output.to_str().is_none() {
        return Err(LlvmEmitError::InvalidOutputPath {
            path: output.to_path_buf(),
        });
    }

    let context = Context::create();
    let module = build_main_module_from_lowered_hir(
        source_map,
        entry_source_id,
        &context,
        lowered,
        entry_main_fqn,
    )?;

    let (target_machine, _target_info) = target::host_target_machine_with_opt_level(opt_level)?;
    run_pass_pipeline(&module, &target_machine, opt_level)?;
    target_machine
        .write_to_file(&module, FileType::Object, output)
        .map_err(|e| LlvmEmitError::WriteObjFailed {
            path: output.to_path_buf(),
            message: e.to_string(),
        })?;
    Ok(())
}

/// 生成最小 LLVM assembly，并写入到指定路径（通常为 `.s` / `.asm`）。
pub fn emit_minimal_main_asm_to_file(
    session: &Session,
    source: &SourceFile,
    output: &Path,
) -> Result<(), LlvmEmitError> {
    emit_minimal_main_asm_to_file_with_opt_level(session, source, output, OptLevel::O0)
}

/// 生成最小 LLVM assembly，并写入到指定路径（通常为 `.s` / `.asm`）。
pub fn emit_minimal_main_asm_to_file_with_opt_level(
    session: &Session,
    source: &SourceFile,
    output: &Path,
    opt_level: OptLevel,
) -> Result<(), LlvmEmitError> {
    // `TargetMachine::write_to_file` 内部会 `path.to_str().expect(...)`，为了避免 panic，
    // 这里提前做 UTF-8 校验并返回结构化诊断。
    if output.to_str().is_none() {
        return Err(LlvmEmitError::InvalidOutputPath {
            path: output.to_path_buf(),
        });
    }

    let context = Context::create();
    let module = build_minimal_main_module(session, source, &context)?;

    let (target_machine, _target_info) = target::host_target_machine_with_opt_level(opt_level)?;
    run_pass_pipeline(&module, &target_machine, opt_level)?;
    target_machine
        .write_to_file(&module, FileType::Assembly, output)
        .map_err(|e| LlvmEmitError::WriteAsmFailed {
            path: output.to_path_buf(),
            message: e.to_string(),
        })?;

    Ok(())
}

/// 基于 `hir::LoweredHir` 生成最小 LLVM assembly，并写入到指定路径（通常为 `.s` / `.asm`）。
pub fn emit_minimal_main_asm_to_file_from_lowered_hir(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    lowered: &hir::LoweredHir,
    output: &Path,
) -> Result<(), LlvmEmitError> {
    emit_minimal_main_asm_to_file_from_lowered_hir_with_opt_level(
        source_map,
        entry_source_id,
        lowered,
        output,
        OptLevel::O0,
    )
}

/// 基于 `hir::LoweredHir` 生成最小 LLVM assembly，并写入到指定路径（通常为 `.s` / `.asm`）。
pub fn emit_minimal_main_asm_to_file_from_lowered_hir_with_opt_level(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    lowered: &hir::LoweredHir,
    output: &Path,
    opt_level: OptLevel,
) -> Result<(), LlvmEmitError> {
    if output.to_str().is_none() {
        return Err(LlvmEmitError::InvalidOutputPath {
            path: output.to_path_buf(),
        });
    }

    let context = Context::create();
    let module =
        build_main_module_from_lowered_hir(source_map, entry_source_id, &context, lowered, None)?;

    let (target_machine, _target_info) = target::host_target_machine_with_opt_level(opt_level)?;
    run_pass_pipeline(&module, &target_machine, opt_level)?;
    target_machine
        .write_to_file(&module, FileType::Assembly, output)
        .map_err(|e| LlvmEmitError::WriteAsmFailed {
            path: output.to_path_buf(),
            message: e.to_string(),
        })?;
    Ok(())
}

/// 基于 `hir::LoweredHir` 生成最小 LLVM assembly，并写入到指定路径（允许显式指定入口 `main` 的 FQN）。
pub fn emit_minimal_main_asm_to_file_from_lowered_hir_with_entry(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    lowered: &hir::LoweredHir,
    output: &Path,
    entry_main_fqn: Option<&str>,
) -> Result<(), LlvmEmitError> {
    emit_minimal_main_asm_to_file_from_lowered_hir_with_entry_with_opt_level(
        source_map,
        entry_source_id,
        lowered,
        output,
        entry_main_fqn,
        OptLevel::O0,
    )
}

/// 基于 `hir::LoweredHir` 生成最小 LLVM assembly，并写入到指定路径（允许显式指定入口 `main` 的 FQN）。
pub fn emit_minimal_main_asm_to_file_from_lowered_hir_with_entry_with_opt_level(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    lowered: &hir::LoweredHir,
    output: &Path,
    entry_main_fqn: Option<&str>,
    opt_level: OptLevel,
) -> Result<(), LlvmEmitError> {
    if output.to_str().is_none() {
        return Err(LlvmEmitError::InvalidOutputPath {
            path: output.to_path_buf(),
        });
    }

    let context = Context::create();
    let module = build_main_module_from_lowered_hir(
        source_map,
        entry_source_id,
        &context,
        lowered,
        entry_main_fqn,
    )?;

    let (target_machine, _target_info) = target::host_target_machine_with_opt_level(opt_level)?;
    run_pass_pipeline(&module, &target_machine, opt_level)?;
    target_machine
        .write_to_file(&module, FileType::Assembly, output)
        .map_err(|e| LlvmEmitError::WriteAsmFailed {
            path: output.to_path_buf(),
            message: e.to_string(),
        })?;
    Ok(())
}

fn build_minimal_main_module<'ctx>(
    session: &Session,
    source: &SourceFile,
    context: &'ctx Context,
) -> Result<inkwell::module::Module<'ctx>, LlvmEmitError> {
    let codegen_unit = frontend::prepare_single_file_codegen_unit(session, source)?;
    build_main_module_from_lowered_hir(
        &codegen_unit.source_map,
        codegen_unit.entry_source_id,
        context,
        &codegen_unit.lowered,
        None,
    )
}

fn build_main_module_from_lowered_hir<'ctx>(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    context: &'ctx Context,
    lowered: &hir::LoweredHir,
    entry_main_fqn: Option<&str>,
) -> Result<inkwell::module::Module<'ctx>, LlvmEmitError> {
    configure_llvm_global_options_once();

    let entry_source = entry_source(source_map, entry_source_id);
    let module_name = module_name_from_path(entry_source.path());
    let module = context.create_module(&module_name);

    // T0803：用 host target machine 配置 module（triple + data layout），并暴露 target 信息。
    let target_info = target::configure_module_for_host(&module)?;
    let target_data = TargetData::create(&target_info.data_layout);

    let hir_main = if let Some(entry_main_fqn) = entry_main_fqn {
        lowered.file.items.iter().find_map(|item| match item {
            hir::Item::Fun(fun) if fun.fqn == entry_main_fqn => Some(fun),
            _ => None,
        })
    } else {
        lowered.file.items.iter().find_map(|item| match item {
            hir::Item::Fun(fun) if fun.name == "main" => Some(fun),
            _ => None,
        })
    }
    .ok_or(LlvmEmitError::MissingEntryMain)?;

    let builder = context.create_builder();

    let fun_index: HashMap<String, &hir::FunDecl> = lowered
        .file
        .items
        .iter()
        .filter_map(|item| match item {
            hir::Item::Fun(fun) => Some(fun),
            _ => None,
        })
        .chain(lowered.member_funs.iter())
        .map(|fun| (fun.fqn.clone(), fun))
        .collect();
    let effect_op_tags = Rc::new(RefCell::new(codegen::EffectOpTagState::new()));

    // T0810：在确认入口存在后，再声明/生成 `main` 可达的其它顶层函数：
    // - 避免“无 main”时把无关错误暴露给调用方；
    // - 避免因为文件里存在“当前后端不支持的函数签名”（例如泛型函数）而影响不相关的程序。
    let mut declare = codegen::MainCodegen::new(codegen::MainCodegenInputs {
        context,
        module: &module,
        builder: &builder,
        target_data: &target_data,
        host: &target_info,
        source_map,
        entry_source_id,
        types: &lowered.types,
        struct_layouts: &lowered.struct_layouts,
        enum_layouts: &lowered.enum_layouts,
        top_level_vars: &lowered.top_level_vars,
        top_level_consts: &lowered.top_level_consts,
        top_level_immutable_values: &lowered.top_level_immutable_values,
        object_inits: &lowered.object_inits,
        class_inits: &lowered.class_inits,
        class_vtables: &lowered.class_vtables,
        interfaces: &lowered.interfaces,
        class_itables: &lowered.class_itables,
        ctor_call_sites: &lowered.ctor_call_sites,
        effect_op_call_sites: &lowered.effect_op_call_sites,
        handle_payload_tuple_tys: &lowered.handle_payload_tuple_tys,
        continuation_resume_call_sites: &lowered.continuation_resume_call_sites,
        non_pure_continuation_resume_call_sites: &lowered.non_pure_continuation_resume_call_sites,
        when_pat_binding_tys: &lowered.when_pat_binding_tys,
        nominal_kinds: &lowered.nominal_kinds,
        nominal_variances: &lowered.nominal_variances,
        direct_supertypes: &lowered.direct_supertypes,
        builtins: lowered.builtins,
        extern_funs: &lowered.extern_funs,
        fun_index: &fun_index,
        effect_op_tags: Rc::clone(&effect_op_tags),
    });

    let mut reachable: Vec<&hir::FunDecl> = collect_reachable_top_level_funs(
        hir_main,
        &fun_index,
        ReachabilityInputs {
            class_inits: &lowered.class_inits,
            class_vtables: &lowered.class_vtables,
            class_itables: &lowered.class_itables,
            ctor_call_sites: &lowered.ctor_call_sites,
            top_level_consts: &lowered.top_level_consts,
            top_level_immutable_values: &lowered.top_level_immutable_values,
        },
    );

    // T0111: Eagerly include struct member methods (operator overloads like `plus`, `compareTo`
    // are dispatched at codegen time from `Binary` expressions, which the reachability scanner
    // cannot detect since HIR types for VarRef are `Any`).
    {
        let reachable_fqns: std::collections::HashSet<&str> =
            reachable.iter().map(|f| f.fqn.as_str()).collect();
        for struct_fqn in lowered.struct_layouts.keys() {
            let prefix = format!("{struct_fqn}.");
            for (fqn, fun) in &fun_index {
                if fqn.starts_with(&prefix) && !reachable_fqns.contains(fqn.as_str()) {
                    reachable.push(fun);
                }
            }
        }
    }

    // T0126: Eagerly include monomorphized generic class member methods.
    // When a generic class method like `Box.get` is reachable, also include all its
    // monomorphized variants (e.g., `Box.get::<Int>`, `Box.get::<String>`).
    {
        let reachable_fqns: std::collections::HashSet<&str> =
            reachable.iter().map(|f| f.fqn.as_str()).collect();
        let mut monomorphized: Vec<&hir::FunDecl> = Vec::new();
        for (fqn, fun) in &fun_index {
            // Monomorphized member methods have `::<` in their FQN.
            if fqn.contains("::<") && !reachable_fqns.contains(fqn.as_str()) {
                // Check if the base (non-monomorphized) method is reachable.
                if let Some(base_fqn) = fqn.split("::<").next()
                    && reachable_fqns.contains(base_fqn)
                {
                    monomorphized.push(fun);
                }
            }
        }
        reachable.extend(monomorphized);
    }

    // T0126: Helper to check if a function's signature contains TypeKind::Param
    // (recursively, including inside Nominal type args like `Printer<T>`).
    let ty_contains_param = |types: &crate::ty::TypeStore, ty: crate::ty::TypeId| -> bool {
        let mut stack = vec![ty];
        while let Some(id) = stack.pop() {
            match types.kind(id) {
                crate::ty::TypeKind::Param(_) => return true,
                crate::ty::TypeKind::Ref(crate::ty::RefTypeKind::Nominal(n))
                | crate::ty::TypeKind::Value(crate::ty::ValueTypeKind::Nominal(n)) => {
                    stack.extend(n.args.iter().copied());
                }
                _ => {}
            }
        }
        false
    };
    let fun_has_param_types = |fun: &hir::FunDecl| -> bool {
        fun.params
            .iter()
            .any(|p| ty_contains_param(&lowered.types, p.ty))
            || ty_contains_param(&lowered.types, fun.return_ty)
    };

    reachable.sort_by(|a, b| a.fqn.cmp(&b.fqn));

    for fun in &reachable {
        // T0126: Skip generic (unmonomorphized) member methods — they contain Param types
        // that cannot be lowered to LLVM types. The monomorphized variants handle these.
        if fun_has_param_types(fun) {
            continue;
        }
        let _ = declare.declare_top_level_fun(fun)?;
    }

    for fun in &reachable {
        if fun.body.is_none() {
            continue;
        }
        // T0126: Skip generic member methods (same as above).
        if fun_has_param_types(fun) {
            continue;
        }
        let llvm_fun = module
            .get_function(&fun.fqn)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "missing declared function",
                at: fun.span.into(),
            })?;
        codegen::MainCodegen::new(codegen::MainCodegenInputs {
            context,
            module: &module,
            builder: &builder,
            target_data: &target_data,
            host: &target_info,
            source_map,
            entry_source_id,
            types: &lowered.types,
            struct_layouts: &lowered.struct_layouts,
            enum_layouts: &lowered.enum_layouts,
            top_level_vars: &lowered.top_level_vars,
            top_level_consts: &lowered.top_level_consts,
            top_level_immutable_values: &lowered.top_level_immutable_values,
            object_inits: &lowered.object_inits,
            class_inits: &lowered.class_inits,
            class_vtables: &lowered.class_vtables,
            interfaces: &lowered.interfaces,
            class_itables: &lowered.class_itables,
            ctor_call_sites: &lowered.ctor_call_sites,
            effect_op_call_sites: &lowered.effect_op_call_sites,
            handle_payload_tuple_tys: &lowered.handle_payload_tuple_tys,
            continuation_resume_call_sites: &lowered.continuation_resume_call_sites,
            non_pure_continuation_resume_call_sites: &lowered
                .non_pure_continuation_resume_call_sites,
            when_pat_binding_tys: &lowered.when_pat_binding_tys,
            nominal_kinds: &lowered.nominal_kinds,
            nominal_variances: &lowered.nominal_variances,
            direct_supertypes: &lowered.direct_supertypes,
            builtins: lowered.builtins,
            extern_funs: &lowered.extern_funs,
            fun_index: &fun_index,
            effect_op_tags: Rc::clone(&effect_op_tags),
        })
        .codegen_top_level_fun(fun, llvm_fun)?;
    }

    let i32_type = context.i32_type();
    let i8_ptr_ptr_ty = context.ptr_type(inkwell::AddressSpace::default());
    let fn_type = i32_type.fn_type(&[i32_type.into(), i8_ptr_ptr_ty.into()], false);

    let main = module.add_function("main", fn_type, None);
    // statepoint 只对带 `gc "<strategy>"` 的函数生效；入口 main 里包含用户代码的最小 codegen，
    // 因此这里需要显式标注 GC strategy，让 `rewrite-statepoints-for-gc` 能把 `scoop_alloc_typed` 等调用点
    // 重写为 statepoint 并产出 stackmap records。
    main.set_gc(LLVM_GC_STRATEGY_STATEPOINT_EXAMPLE);
    let entry = context.append_basic_block(main, "entry");
    builder.position_at_end(entry);

    let argc = main
        .get_nth_param(0)
        .ok_or(LlvmEmitError::ModuleVerificationFailed {
            message: "entry main 缺少 argc 参数".to_string(),
        })?
        .into_int_value();
    let argv = main
        .get_nth_param(1)
        .ok_or(LlvmEmitError::ModuleVerificationFailed {
            message: "entry main 缺少 argv 参数".to_string(),
        })?
        .into_pointer_value();
    argc.set_name("argc");
    argv.set_name("argv");

    // T1318c：process.args 需要能读取 argv；在最早期保存参数指针，供 runtime 查询。
    let process_init = module
        .get_function("scoop_process_init")
        .unwrap_or_else(|| {
            module.add_function(
                "scoop_process_init",
                context
                    .void_type()
                    .fn_type(&[i32_type.into(), i8_ptr_ptr_ty.into()], false),
                None,
            )
        });
    builder.build_call(process_init, &[argc.into(), argv.into()], "process_init")?;

    // T0815：在入口函数里调用 runtime init（当前阶段先只调用一次）。
    let rt_init = module
        .get_function("scoop_runtime_init")
        .unwrap_or_else(|| {
            module.add_function(
                "scoop_runtime_init",
                context.void_type().fn_type(&[], false),
                None,
            )
        });
    builder.build_call(rt_init, &[], "rt_init")?;

    let make_main_codegen_inputs = || codegen::MainCodegenInputs {
        context,
        module: &module,
        builder: &builder,
        target_data: &target_data,
        host: &target_info,
        source_map,
        entry_source_id,
        types: &lowered.types,
        struct_layouts: &lowered.struct_layouts,
        enum_layouts: &lowered.enum_layouts,
        top_level_vars: &lowered.top_level_vars,
        top_level_consts: &lowered.top_level_consts,
        top_level_immutable_values: &lowered.top_level_immutable_values,
        object_inits: &lowered.object_inits,
        class_inits: &lowered.class_inits,
        class_vtables: &lowered.class_vtables,
        interfaces: &lowered.interfaces,
        class_itables: &lowered.class_itables,
        ctor_call_sites: &lowered.ctor_call_sites,
        effect_op_call_sites: &lowered.effect_op_call_sites,
        handle_payload_tuple_tys: &lowered.handle_payload_tuple_tys,
        continuation_resume_call_sites: &lowered.continuation_resume_call_sites,
        non_pure_continuation_resume_call_sites: &lowered.non_pure_continuation_resume_call_sites,
        when_pat_binding_tys: &lowered.when_pat_binding_tys,
        nominal_kinds: &lowered.nominal_kinds,
        nominal_variances: &lowered.nominal_variances,
        direct_supertypes: &lowered.direct_supertypes,
        builtins: lowered.builtins,
        extern_funs: &lowered.extern_funs,
        fun_index: &fun_index,
        effect_op_tags: Rc::clone(&effect_op_tags),
    };
    let main_codegen = codegen::MainCodegen::new(make_main_codegen_inputs());
    let exit_code = main_codegen.codegen_main_exit_code(hir_main)?;
    builder.build_return(Some(&exit_code))?;

    module
        .verify()
        .map_err(|e| LlvmEmitError::ModuleVerificationFailed {
            message: e.to_string(),
        })?;

    Ok(module)
}

#[cfg(test)]
fn build_single_file_source_map(session: &Session, source: &SourceFile) -> (SourceMap, SourceId) {
    let input_sources = vec![source.clone()];
    frontend::build_source_map_with_extra_sources(session, &input_sources, 0)
}

fn entry_source(source_map: &SourceMap, entry_source_id: SourceId) -> &SourceFile {
    source_map
        .source(entry_source_id)
        .expect("entry source id should exist in source map")
}

fn run_pass_pipeline<'ctx>(
    module: &inkwell::module::Module<'ctx>,
    target_machine: &inkwell::targets::TargetMachine,
    opt_level: OptLevel,
) -> Result<(), LlvmEmitError> {
    // 说明：
    // - T1503b：从手工 stackmap probe 迁移到 statepoint 产出的 stackmaps；
    // - C2a：在 statepoint 重写前跑 SROA，把“聚合值里的 GC ref 字段”拆解为可追踪 SSA 值，
    //   避免需要在源码里手工提取字段 keepalive。
    // - T1602：按 opt-level 启用 LLVM 默认优化 pipeline；同时保证大多数优化发生在 statepoint 重写之前。
    // - `rewrite-statepoints-for-gc` 会把带 `gc "<strategy>"` 的函数内调用点重写为 statepoints，
    //   并产出 stackmap records（用于 runtime 枚举/更新 spill slots roots）。
    // - 注意：LLVM 18.1.8（Homebrew）下 `place-safepoints` pass 会在 `opt` 上稳定触发 SIGSEGV，
    //   因此当前阶段不应把它纳入默认管线；需要 safepoint placement 时再结合上游修复/替代方案接入。
    let passes = llvm_pass_pipeline_for_opt_level(opt_level);
    let options = PassBuilderOptions::create();
    options.set_verify_each(true);
    module
        .run_passes(passes.as_str(), target_machine, options)
        .map_err(|e| LlvmEmitError::RunPassesFailed {
            passes: passes.clone(),
            message: e.to_string(),
        })?;

    module
        .verify()
        .map_err(|e| LlvmEmitError::ModuleVerificationFailed {
            message: e.to_string(),
        })?;

    Ok(())
}

fn llvm_pass_pipeline_for_opt_level(opt_level: OptLevel) -> String {
    let default_pipeline = match opt_level {
        OptLevel::O0 => None,
        OptLevel::O1 => Some("default<O1>"),
        OptLevel::O2 => Some("default<O2>"),
        OptLevel::O3 => Some("default<O3>"),
        OptLevel::Os => Some("default<Os>"),
        OptLevel::Oz => Some("default<Oz>"),
    };

    let mut passes = String::new();
    if let Some(default_pipeline) = default_pipeline {
        passes.push_str(default_pipeline);
        passes.push(',');
    }

    // GC/statepoint 约束：大多数优化放在 rewrite 之前；rewrite 之后只跑轻量清理，避免在
    // `gc.statepoint/gc.relocate` 之后引入更多不确定性。
    // 注意：moving GC 的 roots 更新目前只支持“可写回的 spill slots”（栈槽），不支持寄存器 roots。
    // 在 LLVM 后端启用 mem2reg 后，某些 GC 指针可能会长时间停留在寄存器中，导致 compaction 后
    // root 未被更新，从而在 `SCOOP_GC_STRESS=1` 下出现 use-after-move/语义错乱（T1606c）。
    //
    // v0 策略：在 statepoint rewrite 之前只跑 SROA，不跑 mem2reg，尽量让 roots 落在可写回的栈槽。
    passes.push_str("function(sroa),rewrite-statepoints-for-gc");
    if opt_level != OptLevel::O0 {
        passes.push_str(",function(instcombine,simplifycfg)");
    }

    passes
}

#[cfg(test)]
mod clayout_tests {
    use super::*;
    use inkwell::values::InstructionOpcode;

    #[test]
    fn clayout_packed_struct_has_expected_field_offsets() {
        let session = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/clayout_packed.scoop",
            r#"
package fixtures.clayout

import scoop.core.*

@CLayout(packed: 1)
struct Packed(val a: UInt8, val b: Int64)

fun main() {
    val a0: UInt8 = 1
    val b0: Int64 = 2
    val s = Packed { a: a0, b: b0 }
    println(0)
}
"#,
        );

        let context = Context::create();
        let module = build_minimal_main_module(&session, &source, &context).unwrap();
        let data_layout = module.get_data_layout();
        let target_data = TargetData::create(data_layout.as_str().to_str().unwrap());

        let packed = context
            .get_struct_type("fixtures.clayout.Packed")
            .expect("missing llvm struct type for fixtures.clayout.Packed");
        assert!(
            packed.is_packed(),
            "expected @CLayout(packed=1) struct to be packed in LLVM"
        );
        assert_eq!(
            target_data.offset_of_element(&packed, 1).unwrap(),
            1,
            "expected second field offset to be 1 for packed struct"
        );
    }

    #[test]
    fn clayout_aligned_struct_sets_alloca_alignment() {
        let session = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/clayout_aligned.scoop",
            r#"
package fixtures.clayout

import scoop.core.*

@CLayout(aligned: 16, packed: 1)
struct AlignedPacked(val a: UInt8, val b: Int64)

fun main() {
    val a0: UInt8 = 1
    val b0: Int64 = 2
    val s = AlignedPacked { a: a0, b: b0 }
    println(0)
}
"#,
        );

        let context = Context::create();
        let module = build_minimal_main_module(&session, &source, &context).unwrap();

        let fun = module
            .get_function("main")
            .expect("missing entry function main");
        let entry = fun
            .get_first_basic_block()
            .expect("function has no entry block");

        let mut found_align: Option<u32> = None;
        let mut inst = entry.get_first_instruction();
        while let Some(i) = inst {
            if i.get_opcode() == InstructionOpcode::Alloca {
                let name = i.get_name().and_then(|n| n.to_str().ok()).unwrap_or("");
                if name == "s" {
                    found_align = Some(i.get_alignment().unwrap());
                    break;
                }
            }
            inst = i.get_next_instruction();
        }

        assert_eq!(
            found_align,
            Some(16),
            "expected local alloca for `s` to have align 16 due to @CLayout(aligned=16)"
        );
    }

    #[test]
    fn clayout_packed_field_load_uses_align_1() {
        let session = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/clayout_packed_field_load.scoop",
            r#"
package fixtures.clayout

import scoop.core.*

@CLayout(packed: 1)
struct Packed(val a: UInt8, val b: Int64)

fun main() {
    val a0: UInt8 = 1
    val b0: Int64 = 2
    val s: Packed = Packed { a: a0, b: b0 }
    val x: Int64 = s.b
    println(0)
}
"#,
        );

        let context = Context::create();
        let module = build_minimal_main_module(&session, &source, &context).unwrap();
        let fun = module
            .get_function("main")
            .expect("missing entry function main");

        let mut found: Option<u32> = None;
        for bb in fun.get_basic_blocks() {
            let mut inst = bb.get_first_instruction();
            while let Some(i) = inst {
                if i.get_opcode() == InstructionOpcode::Load {
                    let name = i.get_name().and_then(|n| n.to_str().ok()).unwrap_or("");
                    if name.starts_with("load_field") {
                        found = Some(i.get_alignment().unwrap());
                        break;
                    }
                }
                inst = i.get_next_instruction();
            }
            if found.is_some() {
                break;
            }
        }

        assert_eq!(
            found,
            Some(1),
            "expected field load from @CLayout(packed=1) struct to use align 1"
        );
    }
}

struct ReachabilityInputs<'a> {
    class_inits: &'a hir::ClassInitIndex,
    class_vtables: &'a crate::vtable::ClassVtableIndex,
    class_itables: &'a crate::itable::ClassItableIndex,
    ctor_call_sites: &'a hir::CtorCallSiteIndex,
    top_level_consts: &'a hir::TopLevelConstIndex,
    top_level_immutable_values: &'a hir::TopLevelImmutableValueIndex,
}

fn collect_reachable_top_level_funs<'a>(
    entry: &'a hir::FunDecl,
    fun_index: &'a HashMap<String, &'a hir::FunDecl>,
    inputs: ReachabilityInputs<'a>,
) -> Vec<&'a hir::FunDecl> {
    let ReachabilityInputs {
        class_inits,
        class_vtables,
        class_itables,
        ctor_call_sites,
        top_level_consts,
        top_level_immutable_values,
    } = inputs;
    let mut collector = ReachabilityCollector {
        fun_index,
        class_inits,
        class_vtables,
        class_itables,
        ctor_call_sites,
        top_level_consts,
        top_level_immutable_values,
        seen_calls: HashSet::new(),
        fun_queue: VecDeque::new(),
        reachable_funs: HashSet::new(),
        seen_ctors: HashSet::new(),
        ctor_queue: VecDeque::new(),
        scanned_class_init_steps: HashSet::new(),
        scanned_top_level_consts: HashSet::new(),
        scanned_top_level_immutable_values: HashSet::new(),
        current_source_path: None,
    };

    // 入口：扫描 `main` 的函数体，但不把 `main` 本身加入 reachable 集合（它由 `codegen_main_exit_code` 生成）。
    collector.scan_fun(entry);

    // BFS：同时处理“顶层函数调用”和“class ctor 调用”（会引入 class init / ctor delegation 中的调用点）。
    loop {
        let mut progressed = false;

        if let Some(fqn) = collector.fun_queue.pop_front() {
            progressed = true;
            let Some(fun) = collector.fun_index.get(&fqn).copied() else {
                // 外部/内建函数：不在本文件 fun_index 里（例如 runtime intrinsics），跳过。
                continue;
            };
            if fun.name == "main" {
                continue;
            }
            if !collector.reachable_funs.insert(fqn.clone()) {
                continue;
            }
            collector.scan_fun(fun);
        }

        if let Some((class_fqn, ctor_span)) = collector.ctor_queue.pop_front() {
            progressed = true;
            collector.scan_ctor(&class_fqn, ctor_span);
        }

        if !progressed {
            break;
        }
    }

    collector
        .reachable_funs
        .into_iter()
        .filter_map(|fqn| collector.fun_index.get(&fqn).copied())
        .collect()
}

struct ReachabilityCollector<'a> {
    fun_index: &'a HashMap<String, &'a hir::FunDecl>,
    class_inits: &'a hir::ClassInitIndex,
    class_vtables: &'a crate::vtable::ClassVtableIndex,
    class_itables: &'a crate::itable::ClassItableIndex,
    ctor_call_sites: &'a hir::CtorCallSiteIndex,
    top_level_consts: &'a hir::TopLevelConstIndex,
    top_level_immutable_values: &'a hir::TopLevelImmutableValueIndex,

    seen_calls: HashSet<String>,
    fun_queue: VecDeque<String>,
    reachable_funs: HashSet<String>,

    seen_ctors: HashSet<(String, Option<Span>)>,
    ctor_queue: VecDeque<(String, Option<Span>)>,

    scanned_class_init_steps: HashSet<String>,
    scanned_top_level_consts: HashSet<String>,
    scanned_top_level_immutable_values: HashSet<String>,
    current_source_path: Option<PathBuf>,
}

impl<'a> ReachabilityCollector<'a> {
    fn with_source_path<T>(&mut self, source_path: &Path, f: impl FnOnce(&mut Self) -> T) -> T {
        let prev = self.current_source_path.replace(source_path.to_path_buf());
        let out = f(self);
        self.current_source_path = prev;
        out
    }

    fn current_call_site(&self, span: Span) -> Option<hir::CallSite> {
        self.current_source_path
            .as_ref()
            .map(|path| hir::CallSite::new(path.clone(), span))
    }

    fn enqueue_fun(&mut self, fqn: String) {
        if self.seen_calls.insert(fqn.clone()) {
            self.fun_queue.push_back(fqn);
        }
    }

    fn scan_top_level_const(&mut self, fqn: &str) {
        if !self.scanned_top_level_consts.insert(fqn.to_string()) {
            return;
        }
        let Some(top_level_const) = self.top_level_consts.get(fqn).cloned() else {
            return;
        };
        self.with_source_path(top_level_const.source_path.as_path(), |this| {
            if let Some(init) = top_level_const.init.as_ref() {
                this.scan_expr(init);
            }
        });
    }

    fn scan_top_level_immutable_value(&mut self, fqn: &str) {
        if !self
            .scanned_top_level_immutable_values
            .insert(fqn.to_string())
        {
            return;
        }
        let Some(value) = self.top_level_immutable_values.get(fqn).cloned() else {
            return;
        };
        self.with_source_path(value.source_path.as_path(), |this| {
            if let Some(init) = value.init.as_ref() {
                this.scan_expr(init);
            }
        });
    }

    fn enqueue_vtable_impls(&mut self, class_fqn: &str) {
        let Some(slots) = self.class_vtables.get(class_fqn) else {
            return;
        };
        for slot in slots {
            self.enqueue_fun(slot.impl_member_fqn.clone());
        }
    }

    fn enqueue_itable_impls(&mut self, class_fqn: &str) {
        let Some(entries) = self.class_itables.get(class_fqn) else {
            return;
        };
        for entry in entries {
            for fqn in &entry.method_impl_fqns {
                if fqn.is_empty() {
                    continue;
                }
                self.enqueue_fun(fqn.clone());
            }
        }
    }

    fn enqueue_ctor(&mut self, class_fqn: String, ctor_span: Option<Span>) {
        let key = (class_fqn, ctor_span);
        if self.seen_ctors.insert(key.clone()) {
            self.ctor_queue.push_back(key);
        }
    }

    fn enqueue_ctor_call_site(&mut self, call_span: Span) {
        let Some(call_site) = self.current_call_site(call_span) else {
            return;
        };
        let Some(info) = self.ctor_call_sites.get(&call_site) else {
            return;
        };

        self.enqueue_ctor(info.class_fqn.clone(), info.ctor_span);
    }

    fn pick_ctor_by_call_target<'b>(
        &self,
        class: &'b hir::ClassInit,
        ctor_span: Option<Span>,
    ) -> Option<&'b hir::ClassCtor> {
        match ctor_span {
            Some(span) => class.ctors.iter().find(|ctor| ctor.span == span),
            None => {
                if class.ctors.is_empty() {
                    return None;
                }
                let mut matching: Vec<&hir::ClassCtor> = class
                    .ctors
                    .iter()
                    .filter(|ctor| ctor.params.is_empty())
                    .collect();
                if matching.len() != 1 {
                    return None;
                }
                Some(matching.pop().expect("len == 1"))
            }
        }
    }

    fn scan_call_arg(&mut self, arg: &hir::CallArg) {
        match arg {
            hir::CallArg::Positional(expr) => self.scan_expr(expr),
            hir::CallArg::Named { value, .. } => self.scan_expr(value),
        }
    }

    fn scan_fun(&mut self, fun: &hir::FunDecl) {
        self.with_source_path(fun.source_path.as_path(), |this| {
            let Some(body) = fun.body.as_ref() else {
                return;
            };
            this.scan_block(body);
        });
    }

    fn scan_block(&mut self, block: &hir::Block) {
        for stmt in &block.stmts {
            self.scan_stmt(stmt);
        }
    }

    fn scan_stmt(&mut self, stmt: &hir::Stmt) {
        match &stmt.kind {
            hir::StmtKind::Empty => {}
            hir::StmtKind::Expr(expr) => self.scan_expr(expr),
            hir::StmtKind::Val(decl) => {
                if let Some(init) = decl.init.as_ref() {
                    self.scan_expr(init);
                }
            }
            hir::StmtKind::Assign { lhs, rhs, .. } => {
                self.scan_expr(lhs);
                self.scan_expr(rhs);
            }
            hir::StmtKind::Return { value } => {
                if let Some(expr) = value.as_ref() {
                    self.scan_expr(expr);
                }
            }
            hir::StmtKind::While { cond, body } => {
                self.scan_expr(cond);
                self.scan_block(body);
            }
            hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Todo(_) => {}
        }
    }

    fn scan_expr(&mut self, expr: &hir::Expr) {
        match &expr.kind {
            hir::ExprKind::Missing | hir::ExprKind::Todo(_) => {}
            hir::ExprKind::Literal(_) | hir::ExprKind::UnresolvedIdent { .. } => {}
            hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) => {
                self.scan_top_level_const(fqn);
                self.scan_top_level_immutable_value(fqn);
            }
            hir::ExprKind::VarRef(hir::ValueRef::Local { .. }) => {}
            hir::ExprKind::StructLit { fields, .. } => {
                for f in fields {
                    self.scan_expr(&f.value);
                }
            }
            hir::ExprKind::TupleLit { elements } => {
                for e in elements {
                    self.scan_expr(e);
                }
            }
            hir::ExprKind::InterpolatedString { parts, .. } => {
                for p in parts {
                    if let hir::InterpolatedStringPart::Expr { expr } = p {
                        self.scan_expr(expr);
                    }
                }
            }
            hir::ExprKind::Unary { expr: inner, .. } => self.scan_expr(inner),
            hir::ExprKind::Binary { lhs, rhs, .. } => {
                self.scan_expr(lhs);
                self.scan_expr(rhs);
            }
            hir::ExprKind::TypeCheck { expr, .. } | hir::ExprKind::Cast { expr, .. } => {
                self.scan_expr(expr);
            }
            hir::ExprKind::Block(block) => self.scan_block(block),
            hir::ExprKind::Call { callee, args } => {
                // 顶层函数调用：收集 callee fqn。
                if let hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) = &callee.kind {
                    if self.fun_index.contains_key(fqn) {
                        self.enqueue_fun(fqn.clone());
                    } else {
                        self.scan_expr(callee);
                    }
                } else {
                    // callee 也可能是 `helper().member` / `foo()()` 这类复合表达式；
                    // 需要继续扫描 callee，避免漏掉其中嵌套的顶层函数或顶层 const 引用。
                    self.scan_expr(callee);
                }

                // constructor call：调用 span 会在 HIR side table 中出现已选 ctor 绑定。
                self.enqueue_ctor_call_site(expr.span);

                for arg in args {
                    self.scan_call_arg(arg);
                }
            }
            hir::ExprKind::Closure(c) => self.scan_expr(&c.body),
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.scan_expr(cond);
                self.scan_expr(then_branch);
                if let Some(e) = else_branch.as_ref() {
                    self.scan_expr(e);
                }
            }
            hir::ExprKind::When { subject, arms } => {
                self.scan_expr(subject);
                for arm in arms {
                    if let Some(guard) = arm.guard.as_ref() {
                        self.scan_expr(guard);
                    }
                    self.scan_expr(&arm.body);
                }
            }
            hir::ExprKind::MemberAccess { receiver, .. } => self.scan_expr(receiver),
            hir::ExprKind::Perform { args, .. } => {
                for arg in args {
                    match arg {
                        hir::CallArg::Positional(e) => self.scan_expr(e),
                        hir::CallArg::Named { value, .. } => self.scan_expr(value),
                    }
                }
            }
            hir::ExprKind::Handle(h) => {
                self.scan_block(&h.body);
                for arm in &h.arms {
                    self.scan_expr(&arm.body);
                }
                if let Some(finally) = h.finally.as_ref() {
                    self.scan_block(finally);
                }
            }
        }
    }

    fn scan_class_init_steps(&mut self, class: &hir::ClassInit) {
        self.with_source_path(class.source_path.as_path(), |this| {
            for step in &class.steps {
                match step {
                    hir::ClassInitStep::PropertyInit { init, .. } => this.scan_expr(init),
                    hir::ClassInitStep::InitBlock { block } => this.scan_block(block),
                }
            }
        });
    }

    fn scan_ctor(&mut self, class_fqn: &str, ctor_span: Option<Span>) {
        let Some(class) = self.class_inits.get(class_fqn).cloned() else {
            return;
        };

        // T1508b：vtable 虚调用需要确保“可达的 class”其 vtable 实现成员也会被后端声明/生成。
        // - class ctor 可达 ⇒ 该 class 的对象可能被分配并参与动态分发；
        // - 因此这里把 vtable slots 指向的实现成员（impl_member_fqn）加入可达集合。
        self.enqueue_vtable_impls(class_fqn);

        // T1508c：interface dispatch 同样依赖 itable entries 中的目标成员可达（含默认方法）。
        self.enqueue_itable_impls(class_fqn);

        // class init steps（property initializer / init blocks）对所有构造路径都可达：只扫描一次。
        if self.scanned_class_init_steps.insert(class.fqn.clone()) {
            self.scan_class_init_steps(&class);
        }

        self.with_source_path(class.source_path.as_path(), |this| {
            let ctor = this.pick_ctor_by_call_target(&class, ctor_span);

            // delegation / super ctor args
            match ctor {
                Some(ctor) if ctor.kind == hir::ClassCtorKind::Secondary => {
                    if let Some(deleg) = ctor.delegation.as_ref() {
                        for arg in &deleg.args {
                            this.scan_call_arg(arg);
                        }
                        if let Some(call) = deleg.call.as_ref() {
                            this.enqueue_ctor(call.class_fqn.clone(), call.ctor_span);
                        } else {
                            match deleg.kind {
                                ast::CtorDelegationKind::This => {
                                    this.enqueue_ctor(class.fqn.clone(), None);
                                }
                                ast::CtorDelegationKind::Super => {
                                    if let Some(super_fqn) = class.super_class_fqn.as_deref() {
                                        this.enqueue_ctor(super_fqn.to_string(), None);
                                    }
                                }
                            }
                        }
                    } else {
                        // secondary ctor（无 delegation）：走 class header 的 super ctor args。
                        for arg in &class.super_ctor_args {
                            this.scan_call_arg(arg);
                        }
                        if let Some(call) = class.super_ctor_call.as_ref() {
                            this.enqueue_ctor(call.class_fqn.clone(), call.ctor_span);
                        } else if let Some(super_fqn) = class.super_class_fqn.as_deref() {
                            this.enqueue_ctor(super_fqn.to_string(), None);
                        }
                    }

                    // secondary ctor body
                    if let Some(body) = ctor.body.as_ref() {
                        this.scan_block(body);
                    }
                }
                _ => {
                    // primary ctor（或隐式 0-参 primary ctor）：走 class header 的 super ctor args。
                    for arg in &class.super_ctor_args {
                        this.scan_call_arg(arg);
                    }
                    if let Some(call) = class.super_ctor_call.as_ref() {
                        this.enqueue_ctor(call.class_fqn.clone(), call.ctor_span);
                    } else if let Some(super_fqn) = class.super_class_fqn.as_deref() {
                        this.enqueue_ctor(super_fqn.to_string(), None);
                    }
                }
            }

            if let Some(ctor) = ctor {
                for param in &ctor.params {
                    if let Some(default_value) = param.default_value.as_ref() {
                        this.scan_expr(default_value);
                    }
                }
            }
        });
    }
}

fn module_name_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("scoop_module")
        .to_string()
}

fn diagnostic_source_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| path.to_str().unwrap_or("<source>"))
        .to_string()
}

fn literal_text_preview(text: &str) -> String {
    const LIMIT: usize = 80;

    let mut out = String::new();
    let mut truncated = false;

    for (count, ch) in text.chars().enumerate() {
        if count == LIMIT {
            truncated = true;
            break;
        }
        match ch {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }

    if truncated {
        out.push_str("...");
    }

    out
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::parser::parse_file;
    use crate::resolve::Index;
    use crate::ty::TypeStore;
    use object::Object;
    use object::ObjectSection;

    fn make_temp_dir(prefix: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("scoopc_{prefix}_{}_{}", std::process::id(), nanos));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn minimal_main_ir_contains_main_and_ret0() {
        let source = SourceFile::new_virtual("<mem>", "package a\nfun main() {}");
        let session = Session::new().unwrap();
        let ir = emit_minimal_main_ir(&session, &source).unwrap();

        // `main` 为 C ABI：`i32 @main(i32 argc, i8** argv)`（inkwell/LLVM 版本可能影响参数命名）。
        assert!(ir.contains("define i32 @main("));
        assert!(
            ir.contains("call void @scoop_runtime_init()"),
            "生成的 main 应调用 scoop_runtime_init"
        );
        assert!(ir.contains("ret i32 0"));
        assert!(ir.contains("target datalayout ="));
        assert!(ir.contains("target triple ="));
    }

    #[test]
    fn float_builtin_types_lower_to_llvm_scalars() {
        let source = SourceFile::new_virtual(
            "<mem>",
            r#"
package a

import scoop.core.*

@Extern(name = "scoop_test_seed64")
fun seed64(): Float64

@Extern(name = "scoop_test_seed32")
fun seed32(): Float32

fun id64(x: Float64): Float64 {
    return x
}

fun id32(x: Float32): Float32 {
    return x
}

fun choose(flag: Bool, left: Float64, right: Float64): Float64 {
    if (flag) {
        return left
    }
    return right
}

fun main() {
    val a64: Float64 = @Unsafe do { seed64() }
    val a32: Float32 = @Unsafe do { seed32() }
    val b64: Float64 = id64(a64)
    val b32: Float32 = id32(a32)
    val c64: Float64 = choose(true, b64, a64)
}
"#,
        );
        let session = Session::new().unwrap();
        let ir = emit_minimal_main_ir(&session, &source).unwrap();

        assert!(
            ir.contains("define double @a.id64("),
            "Float64 should lower to LLVM double in function signatures"
        );
        assert!(
            ir.contains("define float @a.id32("),
            "Float32 should lower to LLVM float in function signatures"
        );
        assert!(
            ir.contains("declare double @scoop_test_seed64()"),
            "extern Float64 function should keep double ABI"
        );
        assert!(
            ir.contains("declare float @scoop_test_seed32()"),
            "extern Float32 function should keep float ABI"
        );
        assert!(
            ir.contains("call double @a.choose("),
            "Float64 return values should stay on the LLVM scalar path through calls"
        );
    }

    #[test]
    fn float_builtin_methods_lower_to_runtime_calls_and_hash_bits() {
        let source = SourceFile::new_virtual(
            "<mem>",
            r#"
package a

import scoop.core.*

@Extern(name = "scoop_test_seed64")
fun seed64(): Float64

@Extern(name = "scoop_test_seed32")
fun seed32(): Float32

fun main() {
    val a64: Float64 = @Unsafe do { seed64() }
    val a32: Float32 = @Unsafe do { seed32() }

    val s64: String = a64.toString()
    val s32: String = a32.toString()
    val i64: Int = a64.toInt()
    val i32: Int = a32.toInt()
    val h64: Int = a64.hash()
    val h32: Int = a32.hash()
}
"#,
        );
        let session = Session::new().unwrap();
        let ir = emit_minimal_main_ir(&session, &source).unwrap();

        assert!(
            ir.contains("@scoop_float64_to_string("),
            "Float64.toString should declare the runtime conversion symbol"
        );
        assert!(
            ir.contains("@scoop_float32_to_string("),
            "Float32.toString should declare the runtime conversion symbol"
        );
        assert!(
            ir.contains("@scoop_float64_to_int("),
            "Float64.toInt should declare the runtime conversion symbol"
        );
        assert!(
            ir.contains("@scoop_float32_to_int("),
            "Float32.toInt should declare the runtime conversion symbol"
        );
        assert!(
            ir.contains("f64_hash_bits"),
            "Float64.hash should lower via float-bit reinterpretation"
        );
        assert!(
            ir.contains("f32_hash_bits"),
            "Float32.hash should lower via float-bit reinterpretation"
        );
    }

    #[test]
    fn float_literals_lower_to_arithmetic_comparisons_and_narrowing() {
        let source = SourceFile::new_virtual(
            "<mem>",
            r#"
package a

import scoop.core.*

val topWide: Float64 = 1.25
val topNarrow: Float32 = 1.5

fun main() {
    val wideBase: Float64 = 1.25
    val narrowBase: Float32 = 1.5
    val wideSum: Float64 = wideBase + 2.75
    val narrowSum: Float32 = narrowBase + 0.5f
    val narrowRem: Float32 = narrowSum % 1.5f
    val absorbed: Float32 = 1.5
    val negWide: Float64 = -wideBase
    val lt: Bool = wideSum < 10.0
    val eq: Bool = narrowBase == 1.5
    val ne: Bool = narrowBase != 2.5
    val text: String = 1.25e2.toString()
    val whole: Int = 3.75.toInt()
}
"#,
        );
        let session = Session::new().unwrap();
        let ir = emit_minimal_main_ir(&session, &source).unwrap();

        assert!(
            ir.contains("fadd double"),
            "Float64 arithmetic should lower via LLVM floating-point add"
        );
        assert!(
            ir.contains("fadd float"),
            "Float32 arithmetic should lower via LLVM floating-point add"
        );
        assert!(
            ir.contains("frem float"),
            "Float32 remainder should lower via LLVM floating-point remainder"
        );
        assert!(
            ir.contains("store float 1.500000e+00, ptr %absorbed"),
            "Unsuffixed Float literals in Float32 contexts should lower as LLVM float constants"
        );
        assert!(
            ir.contains("fcmp olt double"),
            "Float comparisons should use ordered LLVM floating-point predicates"
        );
        assert!(
            ir.contains("fcmp oeq float"),
            "Float equality should use ordered equality for NaN-sensitive semantics"
        );
        assert!(
            ir.contains("fcmp une float"),
            "Float inequality should treat NaN as not-equal"
        );
        assert!(
            ir.contains("fneg double"),
            "Unary Float negation should lower to LLVM floating-point negation"
        );
        assert!(
            ir.contains("@scoop_float64_to_string("),
            "Float literal member calls should reuse Float.toString runtime lowering"
        );
        assert!(
            ir.contains("@scoop_float64_to_int("),
            "Float literal member calls should reuse Float.toInt runtime lowering"
        );
    }

    #[test]
    fn lowered_call_results_keep_concrete_types_for_local_bindings() {
        let session = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>",
            r#"
package a

import scoop.core.*

fun id(x: Int): Int { return x }

fun main() {
    val n = id(1)
    val mag = (-2.5).abs()
    val inf = (1.0 / 0.0).isInfinite()

    println(n.toString())
    println(mag.toString())
    println(inf.toString())
}
"#,
        );

        let mut ast = parse_file(&source).unwrap();
        let index = {
            let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
            for file in &session.sysroot().files {
                pairs.push((&file.source, &file.ast));
            }
            pairs.push((&source, &ast));
            Index::build(&pairs).unwrap()
        };

        let headers = crate::resolve::check_file_headers(&source, &ast, &index).unwrap();
        crate::resolve::check_file_bodies(&source, &mut ast, &index, &headers).unwrap();

        let mut env = crate::typecheck::TypeEnv::from_sysroot(session.sysroot(), &index).unwrap();
        env.extend_from_file(&source, &ast, &index).unwrap();

        let mut typecheck_types = TypeStore::new();
        let builtins = typecheck_types.intern_builtins();
        crate::typecheck::check_file_annotations(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut typecheck_types,
            builtins,
        )
        .unwrap();
        crate::typecheck::check_file_type_refs(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut typecheck_types,
            builtins,
        )
        .unwrap();
        crate::typecheck::check_file_exprs(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut typecheck_types,
            builtins,
        )
        .unwrap();

        let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for file in &session.sysroot().files {
            unit.push((&file.source, &file.ast));
        }
        unit.push((&source, &ast));

        let files_to_lower = vec![(&source, &ast)];
        let lowered = hir::lower_for_compilation_unit_multi_files(
            &source,
            &index,
            &unit,
            &files_to_lower,
            &[],
            &typecheck_types,
        )
        .unwrap();
        let (source_map, entry_source_id) = build_single_file_source_map(&session, &source);
        let ir =
            emit_minimal_main_ir_from_lowered_hir(&source_map, entry_source_id, &lowered).unwrap();

        assert!(
            ir.contains("@scoop_int_to_string("),
            "Unannotated local Int call results should keep Int through lowering/codegen"
        );
        assert!(
            ir.contains("@scoop_float64_to_string("),
            "Unannotated local Float call results should keep Float64 through lowering/codegen"
        );
        assert!(
            ir.contains("@scoop_bool_to_string("),
            "Unannotated local Bool call results should keep Bool through lowering/codegen"
        );
    }

    #[test]
    fn lowered_hir_codegen_accepts_multi_file_source_map() {
        let session = Session::new().unwrap();

        let src_lib = SourceFile::new_virtual(
            "<lib>",
            r#"
package fixtures.t0150b

import scoop.core.*

fun helper(x: Int): Int { return x + 1 }
"#,
        );
        let src_main = SourceFile::new_virtual(
            "<main>",
            r#"
package fixtures.t0150b

import scoop.core.*

fun main(): Int { return helper(41) }
"#,
        );

        let mut ast_lib = parse_file(&src_lib).unwrap();
        let mut ast_main = parse_file(&src_main).unwrap();

        let index = {
            let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
            for file in &session.sysroot().files {
                pairs.push((&file.source, &file.ast));
            }
            pairs.push((&src_lib, &ast_lib));
            pairs.push((&src_main, &ast_main));
            Index::build(&pairs).unwrap()
        };

        let headers_lib = crate::resolve::check_file_headers(&src_lib, &ast_lib, &index).unwrap();
        crate::resolve::check_file_bodies(&src_lib, &mut ast_lib, &index, &headers_lib).unwrap();

        let headers_main =
            crate::resolve::check_file_headers(&src_main, &ast_main, &index).unwrap();
        crate::resolve::check_file_bodies(&src_main, &mut ast_main, &index, &headers_main).unwrap();

        let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for file in &session.sysroot().files {
            unit.push((&file.source, &file.ast));
        }
        unit.push((&src_lib, &ast_lib));
        unit.push((&src_main, &ast_main));

        let files_to_lower = vec![(&src_lib, &ast_lib), (&src_main, &ast_main)];
        let typecheck_types = TypeStore::new();
        let lowered = hir::lower_for_compilation_unit_multi_files(
            &src_main,
            &index,
            &unit,
            &files_to_lower,
            &[],
            &typecheck_types,
        )
        .unwrap();

        let mut source_map = SourceMap::new();
        for file in &session.sysroot().files {
            let _ = source_map.add_source_clone(&file.source);
        }
        let _ = source_map.add_source_clone(&src_lib);
        let entry_source_id = source_map.add_source_clone(&src_main);

        let ir =
            emit_minimal_main_ir_from_lowered_hir(&source_map, entry_source_id, &lowered).unwrap();

        assert!(ir.contains("define i32 @main("));
        assert!(
            ir.contains("@fixtures.t0150b.helper"),
            "expected reachable helper from non-entry file to be present in IR"
        );
    }

    #[test]
    fn cross_file_class_ctor_literal_codegen_uses_correct_source_with_utf8_comments() {
        let session = Session::new().unwrap();

        let src_lib = SourceFile::new_virtual(
            "<lib>",
            r#"
package fixtures.t4016t5a

import scoop.core.*

// 中文注释：跨文件构造器参数不应把 caller span 绑到这里。
class Box(val value: Int)
"#,
        );
        let src_main = SourceFile::new_virtual(
            "<main>",
            r#"
package fixtures.t4016t5a

import scoop.core.*

fun main(): Int {
    val box: Box = Box(7)
    return box.value
}
"#,
        );

        let mut ast_lib = parse_file(&src_lib).unwrap();
        let mut ast_main = parse_file(&src_main).unwrap();

        let index = {
            let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
            for file in &session.sysroot().files {
                pairs.push((&file.source, &file.ast));
            }
            pairs.push((&src_lib, &ast_lib));
            pairs.push((&src_main, &ast_main));
            Index::build(&pairs).unwrap()
        };

        let headers_lib = crate::resolve::check_file_headers(&src_lib, &ast_lib, &index).unwrap();
        crate::resolve::check_file_bodies(&src_lib, &mut ast_lib, &index, &headers_lib).unwrap();

        let headers_main =
            crate::resolve::check_file_headers(&src_main, &ast_main, &index).unwrap();
        crate::resolve::check_file_bodies(&src_main, &mut ast_main, &index, &headers_main).unwrap();

        let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for file in &session.sysroot().files {
            unit.push((&file.source, &file.ast));
        }
        unit.push((&src_lib, &ast_lib));
        unit.push((&src_main, &ast_main));

        let files_to_lower = vec![(&src_lib, &ast_lib), (&src_main, &ast_main)];
        let typecheck_types = TypeStore::new();
        let lowered = hir::lower_for_compilation_unit_multi_files(
            &src_main,
            &index,
            &unit,
            &files_to_lower,
            &[],
            &typecheck_types,
        )
        .unwrap();

        let mut source_map = SourceMap::new();
        for file in &session.sysroot().files {
            let _ = source_map.add_source_clone(&file.source);
        }
        let _ = source_map.add_source_clone(&src_lib);
        let entry_source_id = source_map.add_source_clone(&src_main);

        let ir =
            emit_minimal_main_ir_from_lowered_hir(&source_map, entry_source_id, &lowered).unwrap();

        assert!(ir.contains("define i32 @main("));
    }

    #[test]
    fn effect_runtime_intrinsics_are_emitted_as_symbol_calls() {
        let source = SourceFile::new_virtual(
            "<mem>",
            r#"
package a

import scoop.core.*

fun main(): Int {
    __scoop_effect_clear()
    __scoop_effect_slot_write(9, 4, 33)
    __scoop_effect_slot_write2(7, 5, 11, 22)
    __scoop_effect_set_active()

    val active: Int = __scoop_effect_is_active()
    val tag: Int = __scoop_effect_slot_read_op_tag()
    val key: Int = __scoop_effect_slot_read_effect_instance_key()
    val len: Int = __scoop_effect_slot_read_len_words()
    val single: Int = __scoop_effect_slot_read_value()
    val w0: Int = __scoop_effect_slot_read_word(0)
    val w1: Int = __scoop_effect_slot_read_word(1)

    // 让返回值依赖这些调用，避免未来优化/重写时被意外删除。
    active + tag + key + len + single + w0 + w1
}
"#,
        );

        let session = Session::new().unwrap();
        let ir = emit_minimal_main_ir(&session, &source).unwrap();

        assert!(
            ir.contains("@scoop_effect_is_active"),
            "IR 应包含对 scoop_effect_is_active 的引用"
        );
        assert!(
            ir.contains("@scoop_effect_set_active"),
            "IR 应包含对 scoop_effect_set_active 的引用"
        );
        assert!(
            ir.contains("@scoop_effect_clear"),
            "IR 应包含对 scoop_effect_clear 的引用"
        );
        assert!(
            ir.contains("@scoop_effect_perform_slot_write_u64_2"),
            "IR 应包含对 scoop_effect_perform_slot_write_u64_2 的引用"
        );
        assert!(
            ir.contains("@scoop_effect_perform_slot_write_u64"),
            "IR 应包含对 scoop_effect_perform_slot_write_u64 的引用"
        );
        assert!(
            ir.contains("@scoop_effect_perform_slot_read_op_tag"),
            "IR 应包含对 scoop_effect_perform_slot_read_op_tag 的引用"
        );
        assert!(
            ir.contains("@scoop_effect_perform_slot_read_effect_instance_key"),
            "IR 应包含对 scoop_effect_perform_slot_read_effect_instance_key 的引用"
        );
        assert!(
            ir.contains("@scoop_effect_perform_slot_read_len_words"),
            "IR 应包含对 scoop_effect_perform_slot_read_len_words 的引用"
        );
        assert!(
            ir.contains("@scoop_effect_perform_slot_read_u64"),
            "IR 应包含对 scoop_effect_perform_slot_read_u64 的引用"
        );
        assert!(
            ir.contains("@scoop_effect_perform_slot_read_u64_at"),
            "IR 应包含对 scoop_effect_perform_slot_read_u64_at 的引用"
        );
    }

    #[test]
    fn indirect_multi_payload_perform_boxes_and_unboxes_tuple_transport() {
        let source = SourceFile::new_virtual(
            "main.scoop",
            r#"
package a

import scoop.core.*

effect Edge {
    fun visit(from: String, to: Int): Int
}

fun go(): Int / Edge {
    return Edge.visit("left", 6)
}

fun main(): Int {
    return handle {
        go()
    } with {
        Edge.visit(from, to) -> to + 4
    }
}
"#,
        );

        let session = Session::new().unwrap();
        let mut ast = parse_file(&source).unwrap();
        let index = {
            let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
            for file in &session.sysroot().files {
                pairs.push((&file.source, &file.ast));
            }
            pairs.push((&source, &ast));
            Index::build(&pairs).unwrap()
        };

        let headers = crate::resolve::check_file_headers(&source, &ast, &index).unwrap();
        crate::resolve::check_file_bodies(&source, &mut ast, &index, &headers).unwrap();

        let mut env = crate::typecheck::TypeEnv::from_sysroot(session.sysroot(), &index).unwrap();
        env.extend_from_file(&source, &ast, &index).unwrap();

        let mut typecheck_types = TypeStore::new();
        let builtins = typecheck_types.intern_builtins();
        crate::typecheck::check_file_annotations(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut typecheck_types,
            builtins,
        )
        .unwrap();
        crate::typecheck::check_file_type_refs(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut typecheck_types,
            builtins,
        )
        .unwrap();
        crate::typecheck::check_file_exprs(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut typecheck_types,
            builtins,
        )
        .unwrap();

        let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for file in &session.sysroot().files {
            unit.push((&file.source, &file.ast));
        }
        unit.push((&source, &ast));
        let files_to_lower = vec![(&source, &ast)];
        let lowered = hir::lower_for_compilation_unit_multi_files(
            &source,
            &index,
            &unit,
            &files_to_lower,
            &[],
            &typecheck_types,
        )
        .unwrap();

        let (source_map, entry_source_id) = build_single_file_source_map(&session, &source);
        let ir =
            emit_minimal_main_ir_from_lowered_hir(&source_map, entry_source_id, &lowered).unwrap();

        assert!(
            ir.contains("@scoop_effect_perform_slot_write_u64_with_gc_ref"),
            "ordinary callee perform should still write through the shared gc-ref transport entrypoint"
        );
        assert!(
            ir.contains("rt_alloc_effect_value_box"),
            "multi-payload perform should box the whole tuple payload instead of dropping extra args"
        );
        assert!(
            ir.contains("effect_value_box_payload"),
            "handler binder lowering should unbox the transported tuple payload before reading multiple binders"
        );
        assert!(
            !ir.contains("call void @scoop_effect_perform_slot_write_u64(i32"),
            "multi-payload perform should not fall back to the single-word slot write ABI"
        );
    }

    #[test]
    fn state_machine_multi_payload_perform_uses_tuple_transport() {
        let source = SourceFile::new_virtual(
            "main.scoop",
            r#"
package a

import scoop.core.*

effect Edge {
    fun visit(from: String, to: Int): Int
}

fun main(): Int {
    return handle {
        println("before")
        val x: Int = if (true) Edge.visit("left", 6) else 0
        println("after")
        x + 1
    } with {
        Edge.visit(from, to) , k -> {
            println(from)
            println(to)
            k.resume(to + 1)
        }
    }
}
"#,
        );

        let session = Session::new().unwrap();
        let mut ast = parse_file(&source).unwrap();
        let index = {
            let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
            for file in &session.sysroot().files {
                pairs.push((&file.source, &file.ast));
            }
            pairs.push((&source, &ast));
            Index::build(&pairs).unwrap()
        };

        let headers = crate::resolve::check_file_headers(&source, &ast, &index).unwrap();
        crate::resolve::check_file_bodies(&source, &mut ast, &index, &headers).unwrap();

        let mut typecheck_types = TypeStore::new();
        let builtins = typecheck_types.intern_builtins();
        let mut env = crate::typecheck::TypeEnv::from_sysroot(session.sysroot(), &index).unwrap();
        env.extend_from_file(&source, &ast, &index).unwrap();
        crate::typecheck::check_file_annotations(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut typecheck_types,
            builtins,
        )
        .unwrap();
        crate::typecheck::check_file_type_refs(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut typecheck_types,
            builtins,
        )
        .unwrap();
        crate::typecheck::check_file_exprs(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut typecheck_types,
            builtins,
        )
        .unwrap();

        let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for file in &session.sysroot().files {
            unit.push((&file.source, &file.ast));
        }
        unit.push((&source, &ast));
        let files_to_lower = vec![(&source, &ast)];
        let lowered = hir::lower_for_compilation_unit_multi_files(
            &source,
            &index,
            &unit,
            &files_to_lower,
            &[],
            &typecheck_types,
        )
        .unwrap();

        let (source_map, entry_source_id) = build_single_file_source_map(&session, &source);
        let ir =
            emit_minimal_main_ir_from_lowered_hir(&source_map, entry_source_id, &lowered).unwrap();

        assert!(
            ir.contains("@scoop_effect_perform_slot_write_u64_with_gc_ref"),
            "state-machine perform should also write through the shared gc-ref transport entrypoint"
        );
        assert!(
            ir.contains("rt_alloc_effect_value_box"),
            "state-machine multi-payload perform should box the tuple transport instead of rejecting 2+ args"
        );
        assert!(
            ir.contains("effect_value_box_payload"),
            "state-machine handler binder lowering should unbox the transported tuple payload before reading multiple binders"
        );
        assert!(
            ir.contains("@scoop_continuation_resume_with"),
            "Continuation.resume lowering should route through the shared payload+answer helper entry"
        );
        assert!(
            !ir.contains("@scoop_continuation_resume_into"),
            "Continuation.resume lowering should no longer stage payload by calling the lower-level answer-only helper directly"
        );
        assert!(
            !ir.contains("call void @scoop_effect_perform_slot_write_u64(i32"),
            "state-machine multi-payload perform should not fall back to the single-word slot write ABI"
        );
    }

    #[test]
    fn async_task_ir_uses_ordinary_scoop_task_helpers_not_legacy_runtime_abi() {
        let source = SourceFile::new_virtual(
            "<mem>",
            r#"
package a

import scoop.core.*

fun main(): Int {
    val task: Task<Int> = async {
        val t: Task<Int> = async { 41 }
        val x: Int = await t
        x + 1
    }
    return 0
}
"#,
        );

        let session = Session::new().unwrap();
        let ir = emit_minimal_main_ir(&session, &source).unwrap();

        assert!(
            ir.contains("scoop.core.__task_create::<Int>"),
            "async sugar 应落到 ordinary Scoop `__task_create` helper，而不是旧 runtime ABI"
        );
        assert!(
            ir.contains("scoop.core.__task_step_pending::<Int>"),
            "async task body 内的 await 应改写成 ordinary Scoop pending step helper，而不是同步 join"
        );
        assert!(
            ir.contains("scoop.core.__task_step_ready::<Int>"),
            "async task body 正常完成时应构造 ordinary Scoop ready step helper，而不是直接返回普通值"
        );
        assert!(
            !ir.contains("@scoop_task_create")
                && !ir.contains("@scoop_task_poll")
                && !ir.contains("@scoop_task_step_pending")
                && !ir.contains("@scoop_task_step_ready")
                && !ir.contains("@scoop_task_join"),
            "ordinary `__task_*` 路径不应再直接依赖 legacy `scoop_task_*` runtime ABI"
        );
    }

    #[test]
    fn single_file_minimal_ir_supports_handled_async_await() {
        let source = SourceFile::new_virtual(
            "<mem>",
            r#"
package a

import scoop.core.*

fun main(): Int {
    val resultTask: Task<Int> = async {
        val t: Task<Int> = async { 41 }
        val x: Int = await t
        x + 1
    }

    return handle {
        Async.await(resultTask)
    } with {
        Async.await(taskArg: Task<Int>) -> __task_join(taskArg)
    }
}
"#,
        );

        let session = Session::new().unwrap();
        let ir = emit_minimal_main_ir(&session, &source).unwrap();

        assert!(
            ir.contains("scoop.core.__task_create::<Int>"),
            "single-file LLVM 路径应继续看到 ordinary Scoop `__task_create` helper"
        );
        assert!(
            ir.contains("@scoop_effect_perform_slot_write_u64_with_gc_ref"),
            "handled Async.await(...) 的 perform site 应在最小 IR 路径上保留 effect transport lowering"
        );
        assert!(
            ir.contains("scoop.core.__task_join::<Int>"),
            "外层 handled Async.await(...) 的 arm body 应能在最小 IR 路径上看到 ordinary Scoop `__task_join` helper"
        );
        assert!(
            !ir.contains("@scoop_task_create")
                && !ir.contains("@scoop_task_poll")
                && !ir.contains("@scoop_task_join"),
            "minimal LLVM 路径里的 async / await 主线不应再回退到 legacy task runtime ABI"
        );
    }

    #[test]
    fn task_step_ir_uses_ordinary_scoop_definition_not_legacy_poll_abi() {
        let source = SourceFile::new_virtual(
            "<mem>",
            r#"
package a

import scoop.core.*

fun main(): Int {
    val task: Task<Int> = async { 41 }
    return when (task.step()) {
        TaskStep.Pending -> 0
        TaskStep.Ready(value) -> value
    }
}
"#,
        );

        let session = Session::new().unwrap();
        let ir = emit_minimal_main_ir(&session, &source).unwrap();

        assert!(
            ir.contains("scoop.core.step::<Int>"),
            "Task.step() 应落到 ordinary Scoop `scoop.core.step::<Int>` 定义"
        );
        assert!(
            ir.contains("scoop.core.__task_drive_created::<Int>"),
            "ordinary Scoop 的 `Task.step()` 实现应继续调用 `__task_drive_created::<Int>`"
        );
        assert!(
            !ir.contains("@scoop_task_poll"),
            "Task.step() 不应再直接调用 legacy `scoop_task_poll` runtime ABI"
        );
    }

    #[test]
    fn single_file_minimal_ir_includes_compilable_sysroot_string_helpers() {
        let source = SourceFile::new_virtual(
            "<mem>",
            r#"
package a

import scoop.core.*

fun main(): Int {
    val word: String = "hello".substring(1, 4)
    return if (word == "ell") 1 else 0
}
"#,
        );

        let session = Session::new().unwrap();
        let ir = emit_minimal_main_ir(&session, &source).unwrap();

        assert!(
            ir.contains("@scoop.core.substring("),
            "single-file LLVM 路径应把可编译 sysroot 源中的 substring helper 编进当前模块"
        );
    }

    #[test]
    fn box_int_to_any_uses_addrspace_1_ref_pointer() {
        let source = SourceFile::new_virtual(
            "<mem>",
            r#"
package a

import scoop.core.*

fun main(): Int {
    val a: Any = 1
    __scoop_gc_collect()
    return 0
}
"#,
        );

        let session = Session::new().unwrap();
        let ir = emit_minimal_main_ir(&session, &source).unwrap();

        assert!(
            ir.contains("addrspace(1)"),
            "IR 应包含 addrspace(1)（GC-managed 引用指针）"
        );
        assert!(
            ir.contains("@scoop_alloc_typed"),
            "装箱到 Any 应调用/声明 scoop_alloc_typed"
        );
        assert!(
            !ir.contains("addrspacecast"),
            "当前阶段的装箱路径不应依赖 addrspacecast 回退到 addrspace(0)"
        );
    }

    #[test]
    fn sync_mutex_runtime_calls_use_addrspace_1_object_pointers() {
        let source = SourceFile::new_virtual(
            "<mem>",
            r#"
package a

import scoop.core.*
import scoop.sync.*

fun main(): Int {
    val m: Mutex = mutexCreate()
    m.lock()
    m.unlock()
    m.destroy()
    return 0
}
"#,
        );

        let session = Session::new().unwrap();
        let ir = emit_minimal_main_ir(&session, &source).unwrap();

        assert!(
            ir.contains("@scoop_sync_mutex_create"),
            "IR 应包含对 scoop_sync_mutex_create 的引用"
        );
        assert!(
            ir.contains("addrspace(1)"),
            "IR 应包含 addrspace(1)（GC-managed 引用指针）"
        );
        assert!(
            !ir.contains("addrspacecast"),
            "sync 相关调用不应依赖 addrspacecast 回退到 addrspace(0)"
        );
    }

    #[test]
    fn string_literal_uses_addrspace_1_gc_string_object() {
        let source = SourceFile::new_virtual(
            "<mem>",
            r#"
package a

import scoop.core.*

fun main(): Int {
    val s: String = "hi"
    println(s)
    __scoop_gc_collect()
    println(s)
    return 0
}
"#,
        );

        let session = Session::new().unwrap();
        let ir = emit_minimal_main_ir(&session, &source).unwrap();

        assert!(
            ir.contains("@scoop_println"),
            "IR 应包含对 scoop_println 的引用"
        );
        assert!(
            ir.contains("addrspace(1)"),
            "String 应为 addrspace(1) GC-managed 指针"
        );
        assert!(
            !ir.contains("addrspacecast"),
            "String 相关调用不应依赖 addrspacecast 回退到 addrspace(0)"
        );
    }

    #[test]
    fn object_member_call_uses_gc_managed_singleton_receiver() {
        let source = SourceFile::new_virtual(
            "<mem>",
            r#"
package a

object Helper {
    fun run(): Int {
        return 7
    }
}

fun main(): Int {
    return Helper.run()
}
"#,
        );

        let session = Session::new().unwrap();
        let ir = emit_minimal_main_ir(&session, &source).unwrap();

        assert!(
            ir.contains(
                "@__scoop_object_instance__a.Helper = internal global ptr addrspace(1) null"
            ),
            "object 单例槽应保存 GC-managed receiver 指针"
        );
        assert!(
            ir.contains("@scoop_alloc_typed"),
            "object 单例值应通过 typed alloc 生成真实 Ref 对象"
        );
        assert!(
            ir.contains("call i64 @a.Helper.run(ptr addrspace(1)"),
            "object member call 应把 addrspace(1) receiver 传给成员函数"
        );
        assert!(
            !ir.contains("call i64 @a.Helper.run(ptr @__scoop_object_instance__a.Helper)"),
            "member call 不应再把默认地址空间全局地址直接当 receiver 传递"
        );
        assert!(
            !ir.contains("addrspacecast"),
            "object member call 修复不应退回 addrspacecast 打补丁"
        );
    }

    #[test]
    fn println_int_lowers_via_string_formatting_without_print_int_helpers() {
        let source = SourceFile::new_virtual(
            "<mem>",
            r#"
package a

import scoop.core.*

fun main(): Int {
    println(123)
    __scoop_gc_collect()
    println(-42)
    return 0
}
"#,
        );

        let session = Session::new().unwrap();
        let ir = emit_minimal_main_ir(&session, &source).unwrap();

        assert!(
            ir.contains("@scoop_println"),
            "IR 应包含对 scoop_println 的引用（与 String 路径对齐）"
        );
        assert!(
            ir.contains("@scoop_format_i64"),
            "IR 应通过 scoop_format_i64 走最小格式化（避免 codegen 侧 varargs snprintf）"
        );
        assert!(
            ir.contains("@scoop_alloc_typed"),
            "println(Int) 需要分配 GC-managed String，应调用/声明 scoop_alloc_typed"
        );
        assert!(
            !ir.contains("@scoop_println_i64"),
            "println(Int) 不应再依赖 runtime 的 scoop_println_i64 绕路"
        );
        assert!(
            !ir.contains("addrspacecast"),
            "println(Int)->String 的路径不应依赖 addrspacecast"
        );
    }

    #[test]
    fn array_of_any_uses_ref_element_runtime_apis_without_ptr_to_u64() {
        let source = SourceFile::new_virtual(
            "<mem>",
            r#"
package a

import scoop.core.*

fun main(): Int {
    val a: Any = 1
    val b: Any = 2
    val xs: Array<Any> = [a, b]
    val v: Any = xs.get(0)
    __scoop_gc_collect()
    return 0
}
"#,
        );

        let session = Session::new().unwrap();
        let ir = emit_minimal_main_ir(&session, &source).unwrap();

        assert!(
            ir.contains("@scoop_array_builder_push_ref"),
            "Array<Any> 的 array literal builder 应走 scoop_array_builder_push_ref"
        );
        assert!(
            ir.contains("@scoop_array_get_ref"),
            "Array<Any>.get 应走 scoop_array_get_ref"
        );
        assert!(
            !ir.contains("ptr_to_u64"),
            "ref 元素路径不应把 GC 指针编码为 u64（ptr_to_u64）"
        );
        assert!(
            !ir.contains("u64_to_ref"),
            "ref 元素路径不应从 u64 解码回 GC 指针（u64_to_ref）"
        );
        assert!(
            !ir.contains("addrspacecast"),
            "ref array 路径不应引入 addrspacecast"
        );
    }

    #[test]
    fn array_of_string_uses_ref_element_runtime_apis_without_ptr_to_u64() {
        let source = SourceFile::new_virtual(
            "<mem>",
            r#"
package a

import scoop.core.*

fun main(): Int {
    val xs: MutableArray<String> = ["a", "b"]
    xs.set(0, "z")
    val v: String = xs.get(0)
    println(v)
    return 0
}
"#,
        );

        let session = Session::new().unwrap();
        let ir = emit_minimal_main_ir(&session, &source).unwrap();

        assert!(
            ir.contains("@scoop_array_builder_push_ref"),
            "Array<String> 的 array literal builder 应走 scoop_array_builder_push_ref"
        );
        assert!(
            ir.contains("@scoop_array_get_ref"),
            "Array<String>.get 应走 scoop_array_get_ref"
        );
        assert!(
            ir.contains("@scoop_array_set_ref"),
            "MutableArray<String>.set 应走 scoop_array_set_ref"
        );
        assert!(
            !ir.contains("ptr_to_u64"),
            "String 元素路径不应把 GC 指针编码为 u64（ptr_to_u64）"
        );
        assert!(
            !ir.contains("u64_to_string"),
            "String 元素路径不应从 u64 解码回 GC 字符串指针（u64_to_string）"
        );
        assert!(
            !ir.contains("addrspacecast"),
            "String array 路径不应引入 addrspacecast"
        );
    }

    #[test]
    fn enum_single_field_non_scalar_payload_uses_boxed_variant_path() {
        let source = SourceFile::new_virtual(
            "<mem>",
            r#"
package a

import scoop.core.*

struct Point(val x: Int, val y: Int)

enum Result {
    Ok(val point: Point),
    Msg(val payload: (String, Int)),
    Err(val code: Int),
}

fun main(): Int {
    val ok: Result = Ok(Point { x: 7, y: 8 })
    val msg: Result = Msg(("hello", 30))
    return 0
}
"#,
        );

        let session = Session::new().unwrap();
        let ir = emit_minimal_main_ir(&session, &source).unwrap();

        assert!(
            ir.contains("scoop.runtime.EnumBoxedPayload__a_Result__Ok"),
            "single-field struct payload 应生成 boxed payload object type"
        );
        assert!(
            ir.contains("scoop.runtime.EnumBoxedPayload__a_Result__Msg"),
            "single-field tuple payload 应生成 boxed payload object type"
        );
        assert!(
            ir.contains("__scoop_type_desc_runtime__enum_boxed_payload__a_Result__Ok"),
            "boxed struct payload 应生成对应的类型描述符"
        );
        assert!(
            ir.contains("__scoop_type_desc_runtime__enum_boxed_payload__a_Result__Msg"),
            "boxed tuple payload 应生成对应的类型描述符"
        );
    }

    #[test]
    fn missing_main_is_reported() {
        let source = SourceFile::new_virtual("<mem>", "package a\nfun not_main() {}");
        let session = Session::new().unwrap();
        let err = emit_minimal_main_ir(&session, &source).unwrap_err();

        assert!(matches!(err, LlvmEmitError::MissingEntryMain));
    }

    #[test]
    fn minimal_main_obj_written_is_non_empty() {
        let dir = make_temp_dir("minimal_main_obj_written_is_non_empty");
        let output = dir.join("main.o");

        let source = SourceFile::new_virtual("<mem>", "package a\nfun main() {}");
        let session = Session::new().unwrap();
        emit_minimal_main_obj_to_file(&session, &source, &output).unwrap();

        let size = std::fs::metadata(&output).unwrap().len();
        assert!(size > 0, "object 文件不应为空");

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn minimal_main_obj_contains_stackmap_section_and_header_is_parseable() {
        let dir = make_temp_dir("minimal_main_obj_contains_stackmap_section");
        let output = dir.join("main.o");

        let source = SourceFile::new_virtual(
            "<mem>",
            r#"
package a

import scoop.core.*

fun main() {
    // 强制触发 `Int -> Any` 装箱（heap alloc），让 statepoint pipeline 产出 stackmap records。
    val a: Any = 1
}
"#,
        );
        let session = Session::new().unwrap();
        emit_minimal_main_obj_to_file(&session, &source, &output).unwrap();

        let bytes = std::fs::read(&output).unwrap();
        let obj = object::File::parse(&*bytes).expect("failed to parse object file");

        let stackmap_section = obj
            .sections()
            .find(|s| s.name().ok().is_some_and(|n| n.contains("llvm_stackmaps")))
            .expect("missing stackmap section (llvm_stackmaps)");
        let section_data = stackmap_section
            .data()
            .expect("failed to read stackmap section data");

        let header = super::stackmap::StackMapHeader::parse(section_data)
            .expect("stackmap header should be parseable");
        assert!(
            header.num_records > 0,
            "expected stackmap section to contain at least one record"
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn minimal_main_obj_stackmap_roots_contract_is_verifyable() {
        // GC-FIX Phase A1：
        // - 解析 stackmap records；
        // - 固化“roots locations 是可计算的连续后缀”契约；
        // - 单测层面保证：至少出现一个带 roots 的 record（否则校验形同虚设）。
        let dir = make_temp_dir("minimal_main_obj_stackmap_roots_contract_is_verifyable");
        let output = dir.join("main.o");

        let source = SourceFile::new_virtual(
            "<mem>",
            r#"
package a

import scoop.core.*

fun keepAlive(x: Any): Unit {
}

fun main(): Unit {
    val keep: Any = 1
    // 手动触发一次 GC（调用点应被 statepoint pipeline 产出 stackmap record）。
    __scoop_gc_collect()
    // 显式使用 keep，确保其在 collect 调用点是 live（应出现在 roots locations 后缀）。
    keepAlive(keep)
}
"#,
        );
        let session = Session::new().unwrap();
        emit_minimal_main_obj_to_file(&session, &source, &output).unwrap();

        let bytes = std::fs::read(&output).unwrap();
        let obj = object::File::parse(&*bytes).expect("failed to parse object file");
        let stackmap_section = obj
            .sections()
            .find(|s| s.name().ok().is_some_and(|n| n.contains("llvm_stackmaps")))
            .expect("missing stackmap section (llvm_stackmaps)");
        let section_data = stackmap_section
            .data()
            .expect("failed to read stackmap section data");

        let section = crate::stackmap::StackMapSection::parse(section_data)
            .expect("stackmap section should be parseable (v3)");

        let cfg = if cfg!(target_arch = "x86_64") {
            crate::stackmap::StackMapRootsContractConfig {
                pointer_size: 8,
                sp_dwarf_reg: 7,
                fp_dwarf_reg: Some(6),
            }
        } else if cfg!(target_arch = "aarch64") {
            crate::stackmap::StackMapRootsContractConfig {
                pointer_size: 8,
                sp_dwarf_reg: 31,
                fp_dwarf_reg: Some(29),
            }
        } else {
            panic!("unsupported test target_arch for stackmap roots contract");
        };

        section
            .verify_roots_contract(cfg)
            .expect("stackmap roots contract should hold");

        let roots_records = section
            .records
            .iter()
            .filter(|rec| {
                rec.locations.iter().any(|loc| {
                    matches!(
                        loc.kind,
                        crate::stackmap::StackMapLocationKind::Direct
                            | crate::stackmap::StackMapLocationKind::Indirect
                    ) && loc.size == cfg.pointer_size
                        && (loc.dwarf_reg == cfg.sp_dwarf_reg
                            || cfg.fp_dwarf_reg.is_some_and(|fp| fp == loc.dwarf_reg))
                })
            })
            .count();
        let sample = section
            .records
            .iter()
            .take(3)
            .enumerate()
            .map(|(i, rec)| {
                let locs = rec
                    .locations
                    .iter()
                    .enumerate()
                    .map(|(j, loc)| {
                        format!(
                            "loc[{j}] kind={:?} size={} reg={} off={}",
                            loc.kind, loc.size, loc.dwarf_reg, loc.offset
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "record[{i}] patchpoint_id=0x{:x} inst_off=0x{:x} locs=[{locs}]",
                    rec.patchpoint_id, rec.instruction_offset
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            roots_records > 0,
            "expected at least one record to contain GC roots locations\n{sample}"
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn statepoint_pipeline_rewrites_scoop_alloc_typed_callsites() {
        let source = SourceFile::new_virtual(
            "<mem>",
            r#"
package a

import scoop.core.*

fun main(): Int {
    val a: Any = 1
    return 0
}
"#,
        );
        let session = Session::new().unwrap();

        let context = Context::create();
        let module = build_minimal_main_module(&session, &source, &context).unwrap();
        let (target_machine, _target_info) =
            target::host_target_machine_with_opt_level(OptLevel::O0).unwrap();
        run_pass_pipeline(&module, &target_machine, OptLevel::O0).unwrap();

        let ir = module.print_to_string().to_string();
        assert!(
            ir.contains("llvm.experimental.gc.statepoint"),
            "expected rewrite-statepoints-for-gc to emit gc.statepoint intrinsics"
        );
        assert!(
            ir.contains("scoop_alloc_typed"),
            "expected statepoint pipeline to cover scoop_alloc_typed (alloc safepoint boundary)"
        );
        assert!(
            !ir.contains("llvm.experimental.stackmap"),
            "expected stackmap records to come from statepoints, not manual stackmap probes"
        );
    }

    #[test]
    fn minimal_main_asm_written_is_non_empty() {
        let dir = make_temp_dir("minimal_main_asm_written_is_non_empty");
        let output = dir.join("main.s");

        let source = SourceFile::new_virtual("<mem>", "package a\nfun main() {}");
        let session = Session::new().unwrap();
        emit_minimal_main_asm_to_file(&session, &source, &output).unwrap();

        let size = std::fs::metadata(&output).unwrap().len();
        assert!(size > 0, "assembly 文件不应为空");

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
