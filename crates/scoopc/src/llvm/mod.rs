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
//! - 目前只编译单模块：不会做跨文件/跨包的泛型实例化与链接管理（后续任务）。

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use inkwell::context::Context;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::FileType;
use inkwell::targets::TargetData;
use inkwell::values::InstructionValueError;
use miette::Diagnostic;
use thiserror::Error;

use crate::ast;
use crate::hir;
use crate::parser::ParseError;
use crate::session::Session;
use crate::source::SourceFile;
use crate::span::Span;

mod codegen;
mod stackmap;
mod target;
pub use target::{HostTargetInfo, LlvmTargetError};

/// LLVM statepoint GC 策略名（内置于 LLVM）。
///
/// 说明：
/// - `rewrite-statepoints-for-gc` 只会重写带 `gc "<strategy>"` 的函数；
/// - 当前阶段先复用 LLVM 内置的 `statepoint-example`，后续若需要更精细的 roots 策略再引入自定义 GC strategy。
pub(crate) const LLVM_GC_STRATEGY_STATEPOINT_EXAMPLE: &str = "statepoint-example";

/// LLVM codegen（早期阶段）的错误集合。
#[derive(Debug, Error, Diagnostic)]
pub enum LlvmEmitError {
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
    source: &SourceFile,
    lowered: &hir::LoweredHir,
) -> Result<String, LlvmEmitError> {
    let context = Context::create();
    let module = build_main_module_from_lowered_hir(source, &context, lowered)?;
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
    source: &SourceFile,
    lowered: &hir::LoweredHir,
    output: &Path,
) -> Result<(), LlvmEmitError> {
    let ir = emit_minimal_main_ir_from_lowered_hir(source, lowered)?;
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
    // `TargetMachine::write_to_file` 内部会 `path.to_str().expect(...)`，为了避免 panic，
    // 这里提前做 UTF-8 校验并返回结构化诊断。
    if output.to_str().is_none() {
        return Err(LlvmEmitError::InvalidOutputPath {
            path: output.to_path_buf(),
        });
    }

    let context = Context::create();
    let module = build_minimal_main_module(session, source, &context)?;

    let (target_machine, _target_info) = target::host_target_machine()?;
    run_statepoint_pass_pipeline(&module, &target_machine)?;
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
    source: &SourceFile,
    lowered: &hir::LoweredHir,
    output: &Path,
) -> Result<(), LlvmEmitError> {
    if output.to_str().is_none() {
        return Err(LlvmEmitError::InvalidOutputPath {
            path: output.to_path_buf(),
        });
    }

    let context = Context::create();
    let module = build_main_module_from_lowered_hir(source, &context, lowered)?;

    let (target_machine, _target_info) = target::host_target_machine()?;
    run_statepoint_pass_pipeline(&module, &target_machine)?;
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
    // `TargetMachine::write_to_file` 内部会 `path.to_str().expect(...)`，为了避免 panic，
    // 这里提前做 UTF-8 校验并返回结构化诊断。
    if output.to_str().is_none() {
        return Err(LlvmEmitError::InvalidOutputPath {
            path: output.to_path_buf(),
        });
    }

    let context = Context::create();
    let module = build_minimal_main_module(session, source, &context)?;

    let (target_machine, _target_info) = target::host_target_machine()?;
    run_statepoint_pass_pipeline(&module, &target_machine)?;
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
    source: &SourceFile,
    lowered: &hir::LoweredHir,
    output: &Path,
) -> Result<(), LlvmEmitError> {
    if output.to_str().is_none() {
        return Err(LlvmEmitError::InvalidOutputPath {
            path: output.to_path_buf(),
        });
    }

    let context = Context::create();
    let module = build_main_module_from_lowered_hir(source, &context, lowered)?;

    let (target_machine, _target_info) = target::host_target_machine()?;
    run_statepoint_pass_pipeline(&module, &target_machine)?;
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
    let lowered = hir::lower_for_dump(session, source)?;
    build_main_module_from_lowered_hir(source, context, &lowered)
}

fn build_main_module_from_lowered_hir<'ctx>(
    source: &SourceFile,
    context: &'ctx Context,
    lowered: &hir::LoweredHir,
) -> Result<inkwell::module::Module<'ctx>, LlvmEmitError> {
    let module_name = module_name_from_path(source.path());
    let module = context.create_module(&module_name);

    // T0803：用 host target machine 配置 module（triple + data layout），并暴露 target 信息。
    let target_info = target::configure_module_for_host(&module)?;
    let target_data = TargetData::create(&target_info.data_layout);

    let hir_main = lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            hir::Item::Fun(fun) if fun.name == "main" => Some(fun),
            _ => None,
        })
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

    // T0810：在确认入口存在后，再声明/生成 `main` 可达的其它顶层函数：
    // - 避免“无 main”时把无关错误暴露给调用方；
    // - 避免因为文件里存在“当前后端不支持的函数签名”（例如泛型函数）而影响不相关的程序。
    let mut declare = codegen::MainCodegen::new(
        context,
        &module,
        &builder,
        &target_data,
        &target_info,
        source,
        &lowered.types,
        &lowered.struct_layouts,
        &lowered.enum_layouts,
        &lowered.top_level_vars,
        &lowered.object_inits,
        &lowered.class_inits,
        &lowered.class_vtables,
        &lowered.interfaces,
        &lowered.class_itables,
        &lowered.ctor_call_sites,
        &lowered.extern_funs,
        &fun_index,
    );

    let mut reachable: Vec<&hir::FunDecl> = collect_reachable_top_level_funs(
        hir_main,
        &fun_index,
        &lowered.class_inits,
        &lowered.class_vtables,
        &lowered.class_itables,
        &lowered.ctor_call_sites,
    );
    reachable.sort_by(|a, b| a.fqn.cmp(&b.fqn));

    for fun in &reachable {
        let _ = declare.declare_top_level_fun(fun)?;
    }

    for fun in &reachable {
        if fun.body.is_none() {
            continue;
        }
        let llvm_fun = module
            .get_function(&fun.fqn)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "missing declared function",
                at: fun.span.into(),
            })?;
        codegen::MainCodegen::new(
            context,
            &module,
            &builder,
            &target_data,
            &target_info,
            source,
            &lowered.types,
            &lowered.struct_layouts,
            &lowered.enum_layouts,
            &lowered.top_level_vars,
            &lowered.object_inits,
            &lowered.class_inits,
            &lowered.class_vtables,
            &lowered.interfaces,
            &lowered.class_itables,
            &lowered.ctor_call_sites,
            &lowered.extern_funs,
            &fun_index,
        )
        .codegen_top_level_fun(fun, llvm_fun)?;
    }

    let i32_type = context.i32_type();
    let i8_ptr_ty = context.i8_type().ptr_type(inkwell::AddressSpace::default());
    let i8_ptr_ptr_ty = i8_ptr_ty.ptr_type(inkwell::AddressSpace::default());
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

    let exit_code = codegen::MainCodegen::new(
        context,
        &module,
        &builder,
        &target_data,
        &target_info,
        source,
        &lowered.types,
        &lowered.struct_layouts,
        &lowered.enum_layouts,
        &lowered.top_level_vars,
        &lowered.object_inits,
        &lowered.class_inits,
        &lowered.class_vtables,
        &lowered.interfaces,
        &lowered.class_itables,
        &lowered.ctor_call_sites,
        &lowered.extern_funs,
        &fun_index,
    )
    .codegen_main_exit_code(hir_main)?;
    builder.build_return(Some(&exit_code))?;

    module
        .verify()
        .map_err(|e| LlvmEmitError::ModuleVerificationFailed {
            message: e.to_string(),
        })?;

    Ok(module)
}

fn run_statepoint_pass_pipeline<'ctx>(
    module: &inkwell::module::Module<'ctx>,
    target_machine: &inkwell::targets::TargetMachine,
) -> Result<(), LlvmEmitError> {
    // 说明：
    // - T1503b：从手工 stackmap probe 迁移到 statepoint 产出的 stackmaps；
    // - C2a：在 statepoint 重写前跑 SROA，把“聚合值里的 GC ref 字段”拆解为可追踪 SSA 值，
    //   避免需要在源码里手工提取字段 keepalive。
    // - 当前阶段保持最小但足够正确的闭环：SROA + mem2reg + rewrite-statepoints-for-gc。
    //   后续再逐步补齐 place-safepoints / pipeline 优化策略等。
    const PASSES: &str = "function(sroa,mem2reg),rewrite-statepoints-for-gc";

    let options = PassBuilderOptions::create();
    options.set_verify_each(true);
    module
        .run_passes(PASSES, target_machine, options)
        .map_err(|e| LlvmEmitError::RunPassesFailed {
            passes: PASSES.to_string(),
            message: e.to_string(),
        })?;

    module
        .verify()
        .map_err(|e| LlvmEmitError::ModuleVerificationFailed {
            message: e.to_string(),
        })?;

    Ok(())
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

@CLayout(packed = 1)
struct Packed(val a: UInt8, val b: Int64)

fun main() {
    val s = Packed { a: 1, b: 2 }
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

@CLayout(aligned = 16, packed = 1)
struct AlignedPacked(val a: UInt8, val b: Int64)

fun main() {
    val s = AlignedPacked { a: 1, b: 2 }
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

@CLayout(packed = 1)
struct Packed(val a: UInt8, val b: Int64)

fun main() {
    val s: Packed = Packed { a: 1, b: 2 }
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

fn collect_reachable_top_level_funs<'a>(
    entry: &'a hir::FunDecl,
    fun_index: &'a HashMap<String, &'a hir::FunDecl>,
    class_inits: &'a hir::ClassInitIndex,
    class_vtables: &'a crate::vtable::ClassVtableIndex,
    class_itables: &'a crate::itable::ClassItableIndex,
    ctor_call_sites: &'a hir::CtorCallSiteIndex,
) -> Vec<&'a hir::FunDecl> {
    let mut collector = ReachabilityCollector {
        fun_index,
        class_inits,
        class_vtables,
        class_itables,
        ctor_call_sites,
        seen_calls: HashSet::new(),
        fun_queue: VecDeque::new(),
        reachable_funs: HashSet::new(),
        seen_ctors: HashSet::new(),
        ctor_queue: VecDeque::new(),
        scanned_class_init_steps: HashSet::new(),
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

        if let Some((class_fqn, arity)) = collector.ctor_queue.pop_front() {
            progressed = true;
            collector.scan_ctor(&class_fqn, arity);
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

    seen_calls: HashSet<String>,
    fun_queue: VecDeque<String>,
    reachable_funs: HashSet<String>,

    seen_ctors: HashSet<(String, usize)>,
    ctor_queue: VecDeque<(String, usize)>,

    scanned_class_init_steps: HashSet<String>,
}

impl<'a> ReachabilityCollector<'a> {
    fn enqueue_fun(&mut self, fqn: String) {
        if self.seen_calls.insert(fqn.clone()) {
            self.fun_queue.push_back(fqn);
        }
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

    fn enqueue_ctor(&mut self, class_fqn: String, arity: usize) {
        let key = (class_fqn, arity);
        if self.seen_ctors.insert(key.clone()) {
            self.ctor_queue.push_back(key);
        }
    }

    fn enqueue_ctor_candidates(&mut self, callee_span: Span, arg_count: usize) {
        let Some(candidates) = self.ctor_call_sites.get(&callee_span) else {
            return;
        };

        // 仅关心 class ctor call：筛出在 class init side table 中存在的候选。
        let mut class_candidates: Vec<String> = candidates
            .iter()
            .filter(|fqn| self.class_inits.contains_key(*fqn))
            .cloned()
            .collect();
        class_candidates.sort();
        class_candidates.dedup();

        for class_fqn in class_candidates {
            self.enqueue_ctor(class_fqn, arg_count);
        }
    }

    fn pick_ctor_by_arity<'b>(
        &self,
        class: &'b hir::ClassInit,
        arity: usize,
    ) -> Option<&'b hir::ClassCtor> {
        // 无显式 ctor：视为隐式 0-参 primary ctor。
        if class.ctors.is_empty() {
            return None;
        }

        let mut matching: Vec<&hir::ClassCtor> = class
            .ctors
            .iter()
            .filter(|ctor| ctor.params.len() == arity)
            .collect();
        if matching.len() != 1 {
            return None;
        }
        Some(matching.pop().expect("len == 1"))
    }

    fn scan_fun(&mut self, fun: &hir::FunDecl) {
        let Some(body) = fun.body.as_ref() else {
            return;
        };
        self.scan_block(body);
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
            hir::ExprKind::Literal(_)
            | hir::ExprKind::VarRef(_)
            | hir::ExprKind::UnresolvedIdent { .. } => {}
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
                    self.enqueue_fun(fqn.clone());
                }

                // constructor call：callee span 会在 HIR side table 中出现候选集合。
                self.enqueue_ctor_candidates(callee.span, args.len());

                for arg in args {
                    match arg {
                        hir::CallArg::Positional(e) => self.scan_expr(e),
                        hir::CallArg::Named { value, .. } => self.scan_expr(value),
                    }
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
        for step in &class.steps {
            match step {
                hir::ClassInitStep::PropertyInit { init, .. } => self.scan_expr(init),
                hir::ClassInitStep::InitBlock { block } => self.scan_block(block),
            }
        }
    }

    fn scan_ctor(&mut self, class_fqn: &str, arity: usize) {
        let Some(class) = self.class_inits.get(class_fqn) else {
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
            self.scan_class_init_steps(class);
        }

        let ctor = self.pick_ctor_by_arity(class, arity);

        // delegation / super ctor args
        match ctor {
            Some(ctor) if ctor.kind == hir::ClassCtorKind::Secondary => {
                if let Some(deleg) = ctor.delegation.as_ref() {
                    for e in &deleg.args {
                        self.scan_expr(e);
                    }
                    match deleg.kind {
                        ast::CtorDelegationKind::This => {
                            self.enqueue_ctor(class.fqn.clone(), deleg.args.len());
                        }
                        ast::CtorDelegationKind::Super => {
                            if let Some(super_fqn) = class.super_class_fqn.as_deref() {
                                self.enqueue_ctor(super_fqn.to_string(), deleg.args.len());
                            }
                        }
                    }
                } else {
                    // secondary ctor（无 delegation）：走 class header 的 super ctor args。
                    for e in &class.super_ctor_args {
                        self.scan_expr(e);
                    }
                    if let Some(super_fqn) = class.super_class_fqn.as_deref() {
                        self.enqueue_ctor(super_fqn.to_string(), class.super_ctor_args.len());
                    }
                }

                // secondary ctor body
                if let Some(body) = ctor.body.as_ref() {
                    self.scan_block(body);
                }
            }
            _ => {
                // primary ctor（或隐式 0-参 primary ctor）：走 class header 的 super ctor args。
                for e in &class.super_ctor_args {
                    self.scan_expr(e);
                }
                if let Some(super_fqn) = class.super_class_fqn.as_deref() {
                    self.enqueue_ctor(super_fqn.to_string(), class.super_ctor_args.len());
                }
            }
        }
    }
}

fn module_name_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("scoop_module")
        .to_string()
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
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
    fn effect_runtime_intrinsics_are_emitted_as_symbol_calls() {
        let source = SourceFile::new_virtual(
            "<mem>",
            r#"
package a

import scoop.core.*

fun main(): Int {
    __scoop_effect_clear()
    __scoop_effect_slot_write2(7, 11, 22)
    __scoop_effect_set_active()

    val active: Int = __scoop_effect_is_active()
    val tag: Int = __scoop_effect_slot_read_op_tag()
    val len: Int = __scoop_effect_slot_read_len_words()
    val w0: Int = __scoop_effect_slot_read_word(0)
    val w1: Int = __scoop_effect_slot_read_word(1)

    // 让返回值依赖这些调用，避免未来优化/重写时被意外删除。
    active + tag + len + w0 + w1
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
            ir.contains("@scoop_effect_perform_slot_read_op_tag"),
            "IR 应包含对 scoop_effect_perform_slot_read_op_tag 的引用"
        );
        assert!(
            ir.contains("@scoop_effect_perform_slot_read_len_words"),
            "IR 应包含对 scoop_effect_perform_slot_read_len_words 的引用"
        );
        assert!(
            ir.contains("@scoop_effect_perform_slot_read_u64_at"),
            "IR 应包含对 scoop_effect_perform_slot_read_u64_at 的引用"
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

        let header = super::stackmap::StackMapHeader::parse(section_data.as_ref())
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

        let section = crate::stackmap::StackMapSection::parse(section_data.as_ref())
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
        let (target_machine, _target_info) = target::host_target_machine().unwrap();
        run_statepoint_pass_pipeline(&module, &target_machine).unwrap();

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
