//! LLVM emit API 与 module build 入口。
//!
//! 这层负责：
//! - 面向外部的 stage-only `emit_minimal_main_*` API；
//! - 消费 LLVM stage handoff 组装单个 LLVM module；
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
use crate::ty::{RefTypeKind, TypeKind, ValueTypeKind};
use scoopc_hir_facts::HirFacts;
use scoopc_lir_facts::{LirCallableFacts, LirFacts};

use super::frontend;
use super::pipeline::run_pass_pipeline;
use super::reachability::collect_reachable_top_level_funs;
use super::{LlvmEmitError, codegen, configure_llvm_global_options_once, target};

struct LoweredCodegenEntry<'a> {
    lowered: &'a hir::LoweredHir,
    hir_facts: &'a HirFacts,
    materialized_pass_view: Option<crate::mir::MaterializedMirPassView<'a>>,
    late_lowered_program: Option<&'a crate::effect_lowered::LateLoweredProgram>,
    late_lowered_lir_facts: Option<&'a scoopc_lir_facts::LirFacts>,
    late_lowered_types: Option<&'a crate::ty::TypeStore>,
    abi_program: Option<&'a crate::effect_lowered::LateLoweredProgram>,
    abi_lir_facts: Option<&'a scoopc_lir_facts::LirFacts>,
    abi_types: Option<&'a crate::ty::TypeStore>,
    abi_materialized_pass_view: Option<crate::mir::MaterializedMirPassView<'a>>,
}

#[derive(Clone, Copy)]
pub struct StageEmitInput<'a> {
    hir_compat_scaffold: &'a hir::LoweredHir,
    hir_facts: &'a HirFacts,
    effect_lowered_stage_output: &'a crate::pipeline::EffectLoweredStageOutput,
    abi_visibility_effect_lowered_stage_output:
        Option<&'a crate::pipeline::EffectLoweredStageOutput>,
}

impl<'a> StageEmitInput<'a> {
    pub fn new(
        hir_compat_scaffold: &'a hir::LoweredHir,
        hir_facts: &'a HirFacts,
        effect_lowered_stage_output: &'a crate::pipeline::EffectLoweredStageOutput,
        abi_visibility_effect_lowered_stage_output: Option<
            &'a crate::pipeline::EffectLoweredStageOutput,
        >,
    ) -> Self {
        Self {
            hir_compat_scaffold,
            hir_facts,
            effect_lowered_stage_output,
            abi_visibility_effect_lowered_stage_output,
        }
    }
}

fn build_single_file_stage_output(
    session: &Session,
    source: &SourceFile,
    opt_level: OptLevel,
) -> Result<crate::pipeline::LlvmCodegenStageOutput, LlvmEmitError> {
    let codegen_unit =
        frontend::prepare_single_file_codegen_unit_with_opt_level(session, source, opt_level)?;
    crate::pipeline::run_llvm_codegen_stage(
        session,
        crate::pipeline::LlvmCodegenStageInput::new(
            crate::frontend::CodegenLoweringOutput {
                lowered_hir: codegen_unit.lowered,
                materialized_mir: codegen_unit.materialized_mir,
            },
            None,
            codegen_unit.source_map,
            codegen_unit.entry_source_id,
            None,
            opt_level,
        ),
    )
}

pub(crate) fn emit_single_file_llvm_artifact_to_file_with_opt_level(
    session: &Session,
    source: &SourceFile,
    output: &Path,
    artifact: crate::pipeline::LlvmArtifactKind,
    opt_level: OptLevel,
) -> Result<(), LlvmEmitError> {
    let stage_output = build_single_file_stage_output(session, source, opt_level)?;
    let stage_input = StageEmitInput::new(
        stage_output.hir_compat_scaffold(),
        stage_output.hir_facts(),
        stage_output.effect_lowered_stage_output(),
        stage_output.abi_visibility_effect_lowered_stage_output(),
    );
    match artifact {
        crate::pipeline::LlvmArtifactKind::LlvmIr => emit_main_ir_to_file_from_stage_output(
            stage_output.source_map(),
            stage_output.entry_source_id(),
            stage_input,
            output,
            stage_output.entry_main_fqn(),
            stage_output.opt_level(),
        ),
        crate::pipeline::LlvmArtifactKind::Object => emit_main_obj_to_file_from_stage_output(
            stage_output.source_map(),
            stage_output.entry_source_id(),
            stage_input,
            output,
            stage_output.entry_main_fqn(),
            stage_output.opt_level(),
        ),
        crate::pipeline::LlvmArtifactKind::Asm => emit_main_asm_to_file_from_stage_output(
            stage_output.source_map(),
            stage_output.entry_source_id(),
            stage_input,
            output,
            stage_output.entry_main_fqn(),
            stage_output.opt_level(),
        ),
    }
}

impl<'a> LoweredCodegenEntry<'a> {
    fn from_stage_output(
        lowered: &'a hir::LoweredHir,
        hir_facts: &'a HirFacts,
        effect_lowered_stage_output: &'a crate::pipeline::EffectLoweredStageOutput,
        abi_visibility_effect_lowered_stage_output: Option<
            &'a crate::pipeline::EffectLoweredStageOutput,
        >,
    ) -> Self {
        let abi_visibility_effect_lowered_stage_output =
            abi_visibility_effect_lowered_stage_output.unwrap_or(effect_lowered_stage_output);
        Self {
            lowered,
            hir_facts,
            materialized_pass_view: Some(effect_lowered_stage_output.llvm_residual_pass_view()),
            late_lowered_program: Some(effect_lowered_stage_output.program()),
            late_lowered_lir_facts: Some(effect_lowered_stage_output.lir_facts()),
            late_lowered_types: Some(effect_lowered_stage_output.types()),
            abi_program: Some(abi_visibility_effect_lowered_stage_output.program()),
            abi_lir_facts: Some(abi_visibility_effect_lowered_stage_output.lir_facts()),
            abi_types: Some(abi_visibility_effect_lowered_stage_output.types()),
            abi_materialized_pass_view: Some(
                abi_visibility_effect_lowered_stage_output.llvm_residual_pass_view(),
            ),
        }
    }
}

/// 为一个 Scoop 程序生成默认单文件 LLVM IR（`.ll` 文本）。
///
/// 当前默认路径会先运行 LLVM stage，再消费其 authoritative handoff 发射产物。
///
/// 输出形态：
/// - 一个 LLVM module（module name 取决于输入文件名）；
/// - module target triple / data layout 设为 host；
/// - `i32 @main(i32 argc, i8** argv)` 的 body 来自 `fun main` 的 v1 子集 codegen；若 `main` 为空则返回 0。
pub fn emit_minimal_main_ir(
    session: &Session,
    source: &SourceFile,
) -> Result<String, LlvmEmitError> {
    let context = Context::create();
    let module = build_minimal_main_module_with_opt_level(session, source, &context, OptLevel::O0)?;
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

/// 基于 LLVM stage handoff（P5 late-lowered output + HIR compatibility scaffold）构建 LLVM module。
pub(crate) fn build_main_module_from_stage_output<'ctx>(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    context: &'ctx Context,
    stage_input: StageEmitInput<'_>,
    entry_main_fqn: Option<&str>,
) -> Result<inkwell::module::Module<'ctx>, LlvmEmitError> {
    build_main_module_from_codegen_entry(
        source_map,
        entry_source_id,
        context,
        LoweredCodegenEntry::from_stage_output(
            stage_input.hir_compat_scaffold,
            stage_input.hir_facts,
            stage_input.effect_lowered_stage_output,
            stage_input.abi_visibility_effect_lowered_stage_output,
        ),
        entry_main_fqn,
    )
}

/// 基于 LLVM stage handoff 生成 LLVM IR，并写入到指定路径。
pub fn emit_main_ir_to_file_from_stage_output(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    stage_input: StageEmitInput<'_>,
    output: &Path,
    entry_main_fqn: Option<&str>,
    opt_level: OptLevel,
) -> Result<(), LlvmEmitError> {
    let context = Context::create();
    let module = build_main_module_from_stage_output(
        source_map,
        entry_source_id,
        &context,
        stage_input,
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
    let module = build_minimal_main_module_with_opt_level(session, source, &context, opt_level)?;

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

/// 基于 LLVM stage handoff 生成 LLVM object，并写入到指定路径。
pub fn emit_main_obj_to_file_from_stage_output(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    stage_input: StageEmitInput<'_>,
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
    let module = build_main_module_from_stage_output(
        source_map,
        entry_source_id,
        &context,
        stage_input,
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
    let module = build_minimal_main_module_with_opt_level(session, source, &context, opt_level)?;

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

/// 基于 LLVM stage handoff 生成 LLVM assembly，并写入到指定路径。
pub fn emit_main_asm_to_file_from_stage_output(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    stage_input: StageEmitInput<'_>,
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
    let module = build_main_module_from_stage_output(
        source_map,
        entry_source_id,
        &context,
        stage_input,
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

#[cfg(test)]
pub(crate) fn build_minimal_main_module<'ctx>(
    session: &Session,
    source: &SourceFile,
    context: &'ctx Context,
) -> Result<inkwell::module::Module<'ctx>, LlvmEmitError> {
    build_minimal_main_module_with_opt_level(session, source, context, OptLevel::O0)
}

pub(crate) fn build_minimal_main_module_with_opt_level<'ctx>(
    session: &Session,
    source: &SourceFile,
    context: &'ctx Context,
    opt_level: OptLevel,
) -> Result<inkwell::module::Module<'ctx>, LlvmEmitError> {
    let stage_output = build_single_file_stage_output(session, source, opt_level)?;
    build_main_module_from_stage_output(
        stage_output.source_map(),
        stage_output.entry_source_id(),
        context,
        StageEmitInput::new(
            stage_output.hir_compat_scaffold(),
            stage_output.hir_facts(),
            stage_output.effect_lowered_stage_output(),
            stage_output.abi_visibility_effect_lowered_stage_output(),
        ),
        None,
    )
}

fn build_main_module_from_codegen_entry<'ctx>(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    context: &'ctx Context,
    codegen_entry: LoweredCodegenEntry<'_>,
    entry_main_fqn: Option<&str>,
) -> Result<inkwell::module::Module<'ctx>, LlvmEmitError> {
    build_module_from_codegen_entry_with_root_selector(
        source_map,
        entry_source_id,
        context,
        codegen_entry,
        RootCallableSelector::EntryMain { entry_main_fqn },
    )
}

fn build_module_from_codegen_entry_with_root_selector<'ctx>(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    context: &'ctx Context,
    codegen_entry: LoweredCodegenEntry<'_>,
    root_selector: RootCallableSelector<'_>,
) -> Result<inkwell::module::Module<'ctx>, LlvmEmitError> {
    configure_llvm_global_options_once();

    let LoweredCodegenEntry {
        lowered,
        hir_facts,
        materialized_pass_view,
        late_lowered_program,
        late_lowered_lir_facts,
        late_lowered_types,
        abi_program,
        abi_lir_facts,
        abi_types,
        abi_materialized_pass_view: _abi_materialized_pass_view,
    } = codegen_entry;
    let has_materialized_pass_view = materialized_pass_view.is_some();

    let entry_source = entry_source(source_map, entry_source_id);
    let module_name = module_name_from_path(entry_source.path());
    let module = context.create_module(&module_name);

    // T0803：用 host target machine 配置 module（triple + data layout），并暴露 target 信息。
    let target_info = target::configure_module_for_host(&module)?;
    let target_data = TargetData::create(&target_info.data_layout);

    let late_lowered_program = late_lowered_program.ok_or_else(|| LlvmEmitError::Frontend {
        message: "LLVM module emission now requires stage-owned late-lowered handoff".to_string(),
    })?;
    let late_lowered_lir_facts = late_lowered_lir_facts.ok_or_else(|| LlvmEmitError::Frontend {
        message: "LLVM stage handoff 缺少 primary LIR facts".to_string(),
    })?;
    let late_lowered_types = late_lowered_types.ok_or_else(|| LlvmEmitError::Frontend {
        message: "LLVM stage handoff 缺少 late-lowered TypeStore".to_string(),
    })?;

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
    let selected_root =
        select_root_callable(late_lowered_lir_facts, late_lowered_types, root_selector)?;
    let root_callable = late_lowered_program
        .callable(selected_root.root_fqn)
        .ok_or_else(|| LlvmEmitError::Frontend {
            message: format!(
                "LLVM stage handoff 缺少入口 callable `{}` 的 late-lowered body",
                selected_root.root_fqn
            ),
        })?;
    let root_source = root_callable
        .source_callable()
        .ok_or_else(|| LlvmEmitError::Frontend {
            message: format!(
                "LLVM stage handoff 入口 callable `{}` 缺少 LIR-owned source body contract",
                selected_root.root_fqn
            ),
        })?;
    let builder = context.create_builder();
    let hir_facts = Rc::new(hir_facts.clone());
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
            stable_cone_key: &lowered.stable_cone_key,
            source_cones: &lowered.source_cones,
            stable_type_param_keys: &lowered.stable_type_param_keys,
            types: &lowered.types,
            struct_layouts: &lowered.struct_layouts,
            enum_layouts: &lowered.enum_layouts,
            top_level_vars: &lowered.top_level_vars,
            top_level_immutable_values: &lowered.top_level_immutable_values,
            top_level_fun_call_sites: &lowered.top_level_fun_call_sites,
            object_inits: &lowered.object_inits,
            class_inits: &lowered.class_inits,
            class_vtables: &lowered.class_vtables,
            interfaces: &lowered.interfaces,
            class_itables: &lowered.class_itables,
            ctor_call_sites: &lowered.ctor_call_sites,
            dispatch_call_sites: &lowered.dispatch_call_sites,
            effect_op_call_sites: &lowered.effect_op_call_sites,
            continuation_resume_call_sites: &lowered.continuation_resume_call_sites,
            when_pat_binding_tys: &lowered.when_pat_binding_tys,
            nominal_kinds: &lowered.nominal_kinds,
            direct_supertypes: &lowered.direct_supertypes,
            builtins: lowered.builtins,
            extern_funs: &lowered.extern_funs,
            native_callable_funs: &lowered.native_callable_funs,
            fun_index: &fun_index,
            materialized_pass_view,
            published_late_lowered_program: abi_program.or(Some(late_lowered_program)),
            published_lir_facts: late_lowered_lir_facts,
            hir_facts: Rc::clone(&hir_facts),
            effect_op_tags: Rc::clone(&effect_op_tags),
        });
    debug_assert_eq!(
        unit_codegen.materialized_pass_view().is_some(),
        has_materialized_pass_view,
        "CompilationUnitCodegenCx 应保留 LLVM production 入口显式接入的 materialized pass view 边界"
    );
    let mut declare = unit_codegen.fresh_main_codegen();

    let _reachable_fqns =
        collect_reachable_top_level_funs(selected_root.root_fqn, late_lowered_lir_facts);

    let abi_program = abi_program.ok_or_else(|| LlvmEmitError::Frontend {
        message: "LLVM stage handoff 缺少 ABI visibility late-lowered program".to_string(),
    })?;
    let abi_lir_facts = abi_lir_facts.ok_or_else(|| LlvmEmitError::Frontend {
        message: "LLVM stage handoff 缺少 ABI visibility LIR facts".to_string(),
    })?;
    let abi_types = abi_types.unwrap_or(late_lowered_types);
    let abi_query = declare.materialize_program_abi(abi_program, abi_lir_facts, abi_types)?;
    declare.codegen_program_bodies(
        late_lowered_program,
        abi_program,
        late_lowered_types,
        abi_types,
        &abi_query,
    )?;
    declare.codegen_native_callable_body_symbols(&abi_query)?;
    let cone_init_plans = unit_codegen.cone_init_routine_plans();
    let cone_init_routines = declare.ensure_cone_init_routines_defined(&cone_init_plans)?;
    let thread_local_init_plans = unit_codegen.thread_local_init_routine_plans();
    let thread_local_init_routines =
        declare.ensure_thread_local_init_routines_defined(&thread_local_init_plans)?;
    declare.ensure_thread_init_current_function_defined(&thread_local_init_routines)?;

    if let Some(arg_shape) = selected_root.entry_main_arg_shape {
        let i32_type = context.i32_type();
        let i8_ptr_ptr_ty = context.ptr_type(inkwell::AddressSpace::default());
        let fn_type = i32_type.fn_type(&[i32_type.into(), i8_ptr_ptr_ty.into()], false);

        let main = codegen::declare_exported_abi_function(&module, "main", fn_type);
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

        let rt_init = codegen::declare_runtime_or_native_import_function(
            &module,
            "scoop_runtime_init",
            context.void_type().fn_type(&[], false),
        );
        builder.build_call(rt_init, &[], "rt_init")?;

        let mut main_codegen = unit_codegen.fresh_main_codegen();
        main_codegen.begin_function_explicit_frame_layout(main)?;
        main_codegen.emit_cone_init_calls(&cone_init_routines)?;

        let entry_argv_array = match arg_shape {
            EntryMainArgShape::None => None,
            EntryMainArgShape::ArrayString => {
                let argv_array_fn = codegen::declare_runtime_or_native_import_function(
                    &module,
                    "scoop_entry_argv_array",
                    context
                        .ptr_type(inkwell::AddressSpace::from(1u16))
                        .fn_type(&[i32_type.into(), i8_ptr_ptr_ty.into()], false),
                );
                let call =
                    builder.build_call(argv_array_fn, &[argc.into(), argv.into()], "entry_argv")?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::ModuleVerificationFailed {
                        message: "entry argv helper 未返回值".to_string(),
                    },
                )?;
                Some(raw.into_pointer_value())
            }
        };

        let exit_code = main_codegen.codegen_stage_main_exit_code(
            selected_root.root_fqn,
            entry_argv_array,
            late_lowered_types,
            late_lowered_program,
            &abi_query,
        )?;
        builder.build_return(Some(&exit_code))?;
        main_codegen.finish_function_explicit_frame_layout(root_source.span)?;
    }

    module
        .verify()
        .map_err(|e| LlvmEmitError::ModuleVerificationFailed {
            message: e.to_string(),
        })?;

    Ok(module)
}
fn entry_source(source_map: &SourceMap, entry_source_id: SourceId) -> &SourceFile {
    source_map
        .source(entry_source_id)
        .expect("entry source id should exist in source map")
}

#[derive(Clone, Copy)]
enum EntryMainArgShape {
    None,
    ArrayString,
}

#[derive(Clone, Copy)]
struct SelectedEntryMain<'a> {
    root_fqn: &'a str,
    arg_shape: EntryMainArgShape,
}

#[derive(Clone, Copy)]
struct SelectedRootCallable<'a> {
    root_fqn: &'a str,
    entry_main_arg_shape: Option<EntryMainArgShape>,
}

#[derive(Clone, Copy)]
enum RootCallableSelector<'a> {
    EntryMain { entry_main_fqn: Option<&'a str> },
}

fn classify_entry_main_arg_shape(
    types: &crate::ty::TypeStore,
    callable: &LirCallableFacts,
) -> Option<EntryMainArgShape> {
    if !matches!(
        types.kind(callable.return_ty),
        TypeKind::Value(ValueTypeKind::Unit | ValueTypeKind::Int)
    ) {
        return None;
    }

    match callable.param_tys.as_slice() {
        [] => Some(EntryMainArgShape::None),
        [param_ty] => match types.kind(*param_ty) {
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
                if nominal.fqn == "scoop.core.Array"
                    && nominal.args.len() == 1
                    && matches!(
                        types.kind(nominal.args[0]),
                        TypeKind::Ref(RefTypeKind::String)
                    )
                    && nominal.eff.is_none() =>
            {
                Some(EntryMainArgShape::ArrayString)
            }
            _ => None,
        },
        _ => None,
    }
}

fn select_entry_main<'a>(
    lir_facts: &'a LirFacts,
    types: &crate::ty::TypeStore,
    entry_main_fqn: Option<&str>,
) -> Result<SelectedEntryMain<'a>, LlvmEmitError> {
    let mut candidates = lir_facts
        .callables
        .values()
        .filter(|callable| callable.is_top_level_source_callable())
        .filter(|fun| {
            if let Some(entry_main_fqn) = entry_main_fqn {
                fun.root_fqn() == entry_main_fqn
            } else {
                callable_source_name(fun.root_fqn()) == "main"
            }
        })
        .filter_map(|fun| {
            classify_entry_main_arg_shape(types, fun).map(|arg_shape| SelectedEntryMain {
                root_fqn: fun.root_fqn(),
                arg_shape,
            })
        })
        .collect::<Vec<_>>();

    match candidates.len() {
        0 => Err(LlvmEmitError::MissingEntryMain),
        1 => Ok(candidates.pop().expect("len checked above")),
        count => Err(LlvmEmitError::AmbiguousEntryMain {
            entry: entry_main_fqn.unwrap_or("main").to_string(),
            count,
        }),
    }
}

fn select_root_callable<'a>(
    lir_facts: &'a LirFacts,
    types: &crate::ty::TypeStore,
    selector: RootCallableSelector<'_>,
) -> Result<SelectedRootCallable<'a>, LlvmEmitError> {
    match selector {
        RootCallableSelector::EntryMain { entry_main_fqn } => {
            let selected_main = select_entry_main(lir_facts, types, entry_main_fqn)?;
            Ok(SelectedRootCallable {
                root_fqn: selected_main.root_fqn,
                entry_main_arg_shape: Some(selected_main.arg_shape),
            })
        }
    }
}

fn callable_source_name(root_fqn: &str) -> &str {
    let base = root_fqn
        .rsplit_once("::<")
        .map(|(base, _)| base)
        .unwrap_or(root_fqn);
    let base = base
        .split_once("$overload$")
        .map(|(base, _)| base)
        .unwrap_or(base);
    base.rsplit_once('.').map(|(_, name)| name).unwrap_or(base)
}

fn module_name_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("scoop_module")
        .to_string()
}
