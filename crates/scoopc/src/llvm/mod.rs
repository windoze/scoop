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

use std::path::{Path, PathBuf};
use std::sync::Arc;

use inkwell::values::InstructionValueError;
use miette::{Diagnostic, NamedSource};
use thiserror::Error;

use crate::hir;
use crate::parser::ParseError;
use crate::source::SourceFile;
use crate::span::Span;

mod codegen;
mod emit;
mod frontend;
mod pipeline;
mod reachability;
mod stackmap;
mod target;
#[cfg(test)]
mod tests;

pub use emit::{
    emit_minimal_main_asm_to_file, emit_minimal_main_asm_to_file_from_lowered_hir,
    emit_minimal_main_asm_to_file_from_lowered_hir_with_entry,
    emit_minimal_main_asm_to_file_from_lowered_hir_with_entry_with_opt_level,
    emit_minimal_main_asm_to_file_from_lowered_hir_with_opt_level,
    emit_minimal_main_asm_to_file_with_opt_level, emit_minimal_main_ir,
    emit_minimal_main_ir_from_lowered_hir, emit_minimal_main_ir_to_file,
    emit_minimal_main_ir_to_file_from_lowered_hir,
    emit_minimal_main_ir_to_file_from_lowered_hir_with_entry,
    emit_minimal_main_ir_to_file_from_lowered_hir_with_entry_with_opt_level,
    emit_minimal_main_obj_to_file, emit_minimal_main_obj_to_file_from_lowered_hir,
    emit_minimal_main_obj_to_file_from_lowered_hir_with_entry,
    emit_minimal_main_obj_to_file_from_lowered_hir_with_entry_with_opt_level,
    emit_minimal_main_obj_to_file_from_lowered_hir_with_opt_level,
    emit_minimal_main_obj_to_file_with_opt_level,
};
pub use target::{HostTargetInfo, LlvmTargetError};

#[cfg(test)]
pub(crate) use emit::{
    build_main_module_from_lowered_hir, build_minimal_main_module, build_single_file_source_map,
};
#[cfg(test)]
pub(crate) use pipeline::run_pass_pipeline;

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
