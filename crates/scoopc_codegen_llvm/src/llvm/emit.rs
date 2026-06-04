//! LLVM emit API 与 module build 入口。
//!
//! 这层负责：
//! - 消费 codegen-owned LLVM stage handoff 组装单个 LLVM module；
//! - 在进入 backend lowering 前完成 reachability 与 eager inclusion。
//!
//! 它不负责定义 LLVM pass pipeline，也不在根模块中继续承载大段实现。

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use inkwell::context::Context;
use inkwell::targets::{FileType, TargetData};

use crate::opt::OptLevel;
use crate::source::{SourceFile, SourceId, SourceMap};
use scoopc_lir_facts::LirFacts;

use super::pipeline::run_pass_pipeline;
use super::reachability::collect_reachable_top_level_funs;
use super::{
    EntryMainArgShape, EntryRef, LlvmCodegenStageOutput, LlvmDepLirArtifactHandoff, LlvmEmitError,
    LlvmStageBaseContext, codegen, configure_llvm_global_options_once, target,
};

struct LoweredCodegenEntry<'a> {
    base_context: &'a LlvmStageBaseContext,
    late_lowered_program: &'a crate::effect_lowered::LateLoweredProgram,
    late_lowered_lir_facts: &'a scoopc_lir_facts::LirFacts,
    late_lowered_types: &'a crate::ty::TypeStore,
    abi_program: &'a crate::effect_lowered::LateLoweredProgram,
    abi_lir_facts: &'a scoopc_lir_facts::LirFacts,
    abi_types: &'a crate::ty::TypeStore,
    cached_dep_artifacts: &'a [LlvmDepLirArtifactHandoff],
}

#[derive(Clone, Copy)]
pub struct StageEmitInput<'a> {
    base_context: &'a LlvmStageBaseContext,
    lir: &'a crate::effect_lowered::LateLoweredProgram,
    lir_facts: &'a LirFacts,
    abi_visibility_lir: Option<&'a crate::effect_lowered::LateLoweredProgram>,
    abi_visibility_lir_facts: Option<&'a LirFacts>,
    abi_visibility_types: Option<&'a crate::ty::TypeStore>,
    cached_dep_artifacts: &'a [LlvmDepLirArtifactHandoff],
}

impl<'a> StageEmitInput<'a> {
    pub fn new(
        base_context: &'a LlvmStageBaseContext,
        lir: &'a crate::effect_lowered::LateLoweredProgram,
        lir_facts: &'a LirFacts,
        abi_visibility_lir: Option<&'a crate::effect_lowered::LateLoweredProgram>,
        abi_visibility_lir_facts: Option<&'a LirFacts>,
        abi_visibility_types: Option<&'a crate::ty::TypeStore>,
        cached_dep_artifacts: &'a [LlvmDepLirArtifactHandoff],
    ) -> Self {
        let has_abi_visibility = abi_visibility_lir.is_some();
        assert_eq!(
            has_abi_visibility,
            abi_visibility_lir_facts.is_some(),
            "ABI visibility LIR and LIR facts must be provided together"
        );
        assert_eq!(
            has_abi_visibility,
            abi_visibility_types.is_some(),
            "ABI visibility LIR and TypeStore owner must be provided together"
        );
        Self {
            base_context,
            lir,
            lir_facts,
            abi_visibility_lir,
            abi_visibility_lir_facts,
            abi_visibility_types,
            cached_dep_artifacts,
        }
    }

    pub fn from_stage_output(output: &'a LlvmCodegenStageOutput) -> Self {
        Self::new(
            output.base_context(),
            output.lir(),
            output.lir_facts(),
            output.abi_visibility_lir(),
            output.abi_visibility_lir_facts(),
            output.abi_visibility_types(),
            output.cached_dep_artifacts(),
        )
    }
}

impl<'a> LoweredCodegenEntry<'a> {
    fn from_stage_output(
        base_context: &'a LlvmStageBaseContext,
        lir: &'a crate::effect_lowered::LateLoweredProgram,
        lir_facts: &'a LirFacts,
        abi_visibility_lir: Option<&'a crate::effect_lowered::LateLoweredProgram>,
        abi_visibility_lir_facts: Option<&'a LirFacts>,
        abi_visibility_types: Option<&'a crate::ty::TypeStore>,
        cached_dep_artifacts: &'a [LlvmDepLirArtifactHandoff],
    ) -> Self {
        Self {
            base_context,
            late_lowered_program: lir,
            late_lowered_lir_facts: lir_facts,
            late_lowered_types: base_context.types(),
            abi_program: abi_visibility_lir.unwrap_or(lir),
            abi_lir_facts: abi_visibility_lir_facts.unwrap_or(lir_facts),
            abi_types: abi_visibility_types.unwrap_or_else(|| base_context.types()),
            cached_dep_artifacts,
        }
    }
}

/// 基于 LLVM stage handoff（LIR + LIR facts + LLVM base context）构建 LLVM module。
pub fn build_main_module_from_stage_output<'ctx>(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    context: &'ctx Context,
    stage_input: StageEmitInput<'_>,
    entry: Option<&EntryRef>,
) -> Result<inkwell::module::Module<'ctx>, LlvmEmitError> {
    build_main_module_from_codegen_entry(
        source_map,
        entry_source_id,
        context,
        LoweredCodegenEntry::from_stage_output(
            stage_input.base_context,
            stage_input.lir,
            stage_input.lir_facts,
            stage_input.abi_visibility_lir,
            stage_input.abi_visibility_lir_facts,
            stage_input.abi_visibility_types,
            stage_input.cached_dep_artifacts,
        ),
        entry,
    )
}

/// 基于 LLVM stage handoff 生成 LLVM IR 文本。
pub fn emit_main_ir_from_stage_output(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    stage_input: StageEmitInput<'_>,
    entry: Option<&EntryRef>,
    opt_level: OptLevel,
) -> Result<String, LlvmEmitError> {
    let context = Context::create();
    let module = build_main_module_from_stage_output(
        source_map,
        entry_source_id,
        &context,
        stage_input,
        entry,
    )?;

    let (target_machine, _target_info) = target::host_target_machine_with_opt_level(opt_level)?;
    run_pass_pipeline(&module, &target_machine, opt_level)?;
    Ok(module.print_to_string().to_string())
}

/// 基于 LLVM stage handoff 生成 LLVM IR，并写入到指定路径。
pub fn emit_main_ir_to_file_from_stage_output(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    stage_input: StageEmitInput<'_>,
    output: &Path,
    entry: Option<&EntryRef>,
    opt_level: OptLevel,
) -> Result<(), LlvmEmitError> {
    let ir =
        emit_main_ir_from_stage_output(source_map, entry_source_id, stage_input, entry, opt_level)?;
    std::fs::write(output, ir).map_err(|e| LlvmEmitError::WriteLlFailed {
        path: output.to_path_buf(),
        source: e,
    })?;
    Ok(())
}

/// 基于 LLVM stage handoff 生成 LLVM object，并写入到指定路径。
pub fn emit_main_obj_to_file_from_stage_output(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    stage_input: StageEmitInput<'_>,
    output: &Path,
    entry: Option<&EntryRef>,
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
        entry,
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

/// 基于 LLVM stage handoff 构建 Lib 模式 LLVM module（不生成 entry main 包装）。
///
/// 用于 `scoopc build-single-cone` subprocess：dep cone artifact 只需要
/// callable bodies + cone_init/thread_local_init routines，不需要也不能要求 `fun main`。
/// reachability seed 退化为 `seed_published_lir_callables`，覆盖所有发布 callable。
pub fn build_lib_module_from_stage_output<'ctx>(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    context: &'ctx Context,
    stage_input: StageEmitInput<'_>,
) -> Result<inkwell::module::Module<'ctx>, LlvmEmitError> {
    build_module_from_codegen_entry_with_root_selector(
        source_map,
        entry_source_id,
        context,
        LoweredCodegenEntry::from_stage_output(
            stage_input.base_context,
            stage_input.lir,
            stage_input.lir_facts,
            stage_input.abi_visibility_lir,
            stage_input.abi_visibility_lir_facts,
            stage_input.abi_visibility_types,
            stage_input.cached_dep_artifacts,
        ),
        RootCallableSelector::LibMode,
    )
}

/// 基于 LLVM stage handoff 生成 Lib 模式 LLVM object，并写入到指定路径。
pub fn emit_lib_obj_to_file_from_stage_output(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    stage_input: StageEmitInput<'_>,
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
        build_lib_module_from_stage_output(source_map, entry_source_id, &context, stage_input)?;

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

/// 基于 LLVM stage handoff 生成 LLVM assembly，并写入到指定路径。
pub fn emit_main_asm_to_file_from_stage_output(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    stage_input: StageEmitInput<'_>,
    output: &Path,
    entry: Option<&EntryRef>,
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
        entry,
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

fn build_main_module_from_codegen_entry<'ctx>(
    source_map: &SourceMap,
    entry_source_id: SourceId,
    context: &'ctx Context,
    codegen_entry: LoweredCodegenEntry<'_>,
    entry: Option<&EntryRef>,
) -> Result<inkwell::module::Module<'ctx>, LlvmEmitError> {
    build_module_from_codegen_entry_with_root_selector(
        source_map,
        entry_source_id,
        context,
        codegen_entry,
        RootCallableSelector::EntryMain { entry },
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
        base_context,
        late_lowered_program,
        late_lowered_lir_facts,
        late_lowered_types,
        abi_program,
        abi_lir_facts,
        abi_types,
        cached_dep_artifacts,
    } = codegen_entry;

    let entry_source = entry_source(source_map, entry_source_id);
    let module_name = module_name_from_path(entry_source.path());
    let module = context.create_module(&module_name);

    // T0803：用 host target machine 配置 module（triple + data layout），并暴露 target 信息。
    let target_info = target::configure_module_for_host(&module)?;
    let target_data = TargetData::create(&target_info.data_layout);

    base_context.verify_lir_type_context(late_lowered_lir_facts, "primary")?;
    LlvmStageBaseContext::verify_lir_type_store_owner(abi_types, abi_lir_facts, "ABI visibility")?;

    // Lib mode（subprocess single-cone artifact emit）跳过 entry main 选择：dep cone artifact
    // 只发布 callable bodies，不需要 `fun main`。EntryMain 选择失败仍按 `MissingEntryMain`
    // 早期报错，避免无声跳过 Bin 入口。
    let is_lib_mode = matches!(root_selector, RootCallableSelector::LibMode);
    let selected_root =
        select_root_callable(late_lowered_lir_facts, late_lowered_types, root_selector)?;
    let builder = context.create_builder();
    let effect_op_tags = Rc::new(RefCell::new(codegen::EffectOpTagState::new()));
    let published_late_lowered_program = Some(abi_program);
    let published_late_lowered_types = Some(abi_types);

    // T0810：在确认入口存在后，再声明/生成 `main` 可达的其它顶层函数：
    // - 避免“无 main”时把无关错误暴露给调用方；
    // - 避免因为文件里存在“当前后端不支持的函数签名”（例如泛型函数）而影响不相关的程序。
    let make_unit_codegen = |published_lir_facts| {
        codegen::CompilationUnitCodegenCx::new(codegen::CompilationUnitCodegenInputs {
            context,
            module: &module,
            builder: &builder,
            target_data: &target_data,
            host: &target_info,
            source_map,
            entry_source_id,
            stable_cone_key: base_context.stable_cone_key(),
            source_cones: base_context.source_cones(),
            stable_type_param_keys: base_context.stable_type_param_keys(),
            types: base_context.types(),
            struct_layouts: base_context.struct_layouts(),
            enum_layouts: base_context.enum_layouts(),
            top_level_vars: base_context.top_level_vars(),
            top_level_immutable_values: base_context.top_level_immutable_values(),
            object_inits: base_context.object_inits(),
            class_inits: base_context.class_inits(),
            release_hooks: base_context.release_hooks(),
            when_pat_binding_tys: base_context.when_pat_binding_tys(),
            nominal_kinds: base_context.nominal_kinds(),
            interior_mutable_nominals: base_context.interior_mutable_nominals(),
            builtins: base_context.builtins(),
            callable_sources: base_context.callable_sources(),
            extern_funs: base_context.extern_funs(),
            native_callable_funs: base_context.native_callable_funs(),
            published_late_lowered_program,
            published_late_lowered_types,
            published_lir_facts,
            effect_analysis_facts: base_context.effect_analysis_facts(),
            effect_op_tags: Rc::clone(&effect_op_tags),
        })
    };

    if let Some(selected) = selected_root.as_ref() {
        let _reachable_fqns = collect_reachable_top_level_funs(
            selected.entry.root_fqn(),
            late_lowered_program,
            late_lowered_lir_facts,
        )
        .map_err(|message| LlvmEmitError::Frontend { message })?;
    }

    let unit_codegen = make_unit_codegen(late_lowered_lir_facts);
    let mut declare = unit_codegen.fresh_main_codegen();
    let abi_query = declare.materialize_program_abi(
        abi_program,
        abi_lir_facts,
        abi_types,
        cached_dep_artifacts,
    )?;
    declare.set_active_lir_program(Some(abi_program));
    declare.set_active_lir_facts(Some(abi_lir_facts));
    declare.codegen_program_bodies(
        late_lowered_program,
        abi_program,
        late_lowered_types,
        abi_types,
        &abi_query,
    )?;
    declare.set_active_lir_facts(None);
    declare.set_active_lir_program(None);
    declare.codegen_native_callable_body_symbols(&abi_query)?;
    let cone_init_plans = unit_codegen.cone_init_routine_plans();
    let cone_init_routines = declare.ensure_cone_init_routines_defined(&cone_init_plans)?;
    let thread_local_init_plans = unit_codegen.thread_local_init_routine_plans();
    let thread_local_init_routines =
        declare.ensure_thread_local_init_routines_defined(&thread_local_init_plans)?;
    if !is_lib_mode {
        declare.ensure_thread_init_current_function_defined(&thread_local_init_routines)?;
    }

    if let Some(selected_root) = selected_root
        && let Some(arg_shape) = selected_root.entry_main_arg_shape
    {
        let root_callable = late_lowered_program
            .callable_by_id(selected_root.entry.callable_id())
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "LLVM stage handoff 缺少入口 callable `{}` 的 late-lowered body（id={:?})",
                    selected_root.entry.root_fqn(),
                    selected_root.entry.callable_id()
                ),
            })?;
        if root_callable.root_fqn() != selected_root.entry.root_fqn() {
            return Err(LlvmEmitError::Frontend {
                message: format!(
                    "LLVM stage handoff 入口 `{}` 的 EntryRef 与 LIR body `{}` 不一致",
                    selected_root.entry.root_fqn(),
                    root_callable.root_fqn()
                ),
            });
        }
        let root_source =
            root_callable
                .source_callable()
                .ok_or_else(|| LlvmEmitError::Frontend {
                    message: format!(
                        "LLVM stage handoff 入口 callable `{}` 缺少 LIR-owned source body contract",
                        selected_root.entry.root_fqn()
                    ),
                })?;
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
            selected_root.entry,
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
struct SelectedRootCallable<'a> {
    entry: &'a EntryRef,
    entry_main_arg_shape: Option<EntryMainArgShape>,
}

#[derive(Clone, Copy)]
enum RootCallableSelector<'a> {
    EntryMain { entry: Option<&'a EntryRef> },
    LibMode,
}

fn select_root_callable<'a>(
    _lir_facts: &LirFacts,
    _types: &crate::ty::TypeStore,
    selector: RootCallableSelector<'a>,
) -> Result<Option<SelectedRootCallable<'a>>, LlvmEmitError> {
    match selector {
        RootCallableSelector::EntryMain { entry } => entry
            .map(|entry| SelectedRootCallable {
                entry,
                entry_main_arg_shape: Some(entry.arg_shape()),
            })
            .ok_or(LlvmEmitError::MissingEntryMain)
            .map(Some),
        RootCallableSelector::LibMode => Ok(None),
    }
}

fn module_name_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("scoop_module")
        .to_string()
}
