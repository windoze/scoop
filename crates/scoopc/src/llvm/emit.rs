//! LLVM emit API 与 module build 入口。
//!
//! 这层负责：
//! - 面向外部的 `emit_minimal_main_*` API；
//! - 把 `hir::LoweredHir` 组装成单个 LLVM module；
//! - 在进入 backend lowering 前完成 reachability 与 eager inclusion。
//!
//! 它不负责定义 LLVM pass pipeline，也不在根模块中继续承载大段实现。

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use inkwell::context::Context;
use inkwell::targets::{FileType, TargetData};

use crate::hir;
use crate::opt::OptLevel;
use crate::session::Session;
use crate::source::{SourceFile, SourceId, SourceMap};

use super::frontend;
use super::pipeline::run_pass_pipeline;
use super::reachability::{ReachabilityInputs, collect_reachable_top_level_funs};
use super::{
    LLVM_GC_STRATEGY_STATEPOINT_EXAMPLE, LlvmEmitError, codegen,
    configure_llvm_global_options_once, target,
};

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

pub(crate) fn build_minimal_main_module<'ctx>(
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

pub(crate) fn build_main_module_from_lowered_hir<'ctx>(
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
    let unit_codegen =
        codegen::CompilationUnitCodegenCx::new(codegen::CompilationUnitCodegenInputs {
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
        });
    let mut declare = unit_codegen.fresh_main_codegen();

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
        let body_codegen = unit_codegen.fresh_main_codegen();
        body_codegen.codegen_top_level_fun(fun, llvm_fun)?;
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

    let main_codegen = unit_codegen.fresh_main_codegen();
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
pub(crate) fn build_single_file_source_map(
    session: &Session,
    source: &SourceFile,
) -> (SourceMap, SourceId) {
    let input_sources = vec![source.clone()];
    frontend::build_source_map_with_extra_sources(session, &input_sources, 0)
}

fn entry_source(source_map: &SourceMap, entry_source_id: SourceId) -> &SourceFile {
    source_map
        .source(entry_source_id)
        .expect("entry source id should exist in source map")
}

fn module_name_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("scoop_module")
        .to_string()
}
