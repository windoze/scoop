use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;

use super::surface_resume::surface_resume_publication_owner_identity;
use super::*;
use crate::effect_facts::{
    CallSiteEffectFacts, CallSiteTarget, CallTargetMode, CaseTag, ImplPlan, SiteEffectFacts,
};
use crate::effect_lowered::ir::{
    BoundarySiteKind, ContinuationObjectId, LateLoweredBodyVersionKey, LateLoweredBoundary,
    LateLoweredBoundaryLowering, LateLoweredBoundaryMap, LateLoweredBoundarySource,
    LateLoweredBoundarySourceConsumption, LateLoweredCallBoundaryLowering,
    LateLoweredCallBoundaryOperandContract, LateLoweredCallable,
    LateLoweredCompletionPayloadBinding, LateLoweredCompletionPayloadSource,
    LateLoweredConsumedRuntimeErrorCase, LateLoweredContinuationObject,
    LateLoweredContinuationSurfaceResume, LateLoweredDynamicInvokeEntry, LateLoweredFrameSchema,
    LateLoweredFrameSlotKind, LateLoweredHandleDispatchContract,
    LateLoweredHandlePendingCompletion, LateLoweredOperandValueSource, LateLoweredPlainCallable,
    LateLoweredPlainLocalEffectControl, LateLoweredProgram, LateLoweredResumeInterface,
    LateLoweredResumeMethod, LateLoweredResumePayloadBinding,
    LateLoweredSourceStatementClassification, LateLoweredStateGraph, LateLoweredStateTerminator,
    LateLoweredStepType, LateLoweredSurfaceResumeDispatchPublication, StateId, SystemSlotKind,
};
use crate::effect_lowered::{
    LateLoweredOptOptions, LateLoweredProgramBuilder, optimize_program_with_options,
};
use crate::llvm::codegen::effect_lowered::types::RefactorCallTargetQuery;
use crate::llvm::codegen::{
    CompilationUnitCodegenCx, CompilationUnitCodegenInputs, EffectOpTagState, MainCodegen,
};
use crate::llvm::target;
use crate::mir::{LoweredMir, MirLoweringFacts, SiteId, lower_hir_file_for_dump_with_facts};
use crate::pipeline::{
    MirStageOutput, build_effect_facts_stage_output, build_effect_lowered_stage_output,
    load_typed_hir_stage_output_for_dump,
};
use crate::program_facts::ProgramFacts;
use crate::session::{Session, SessionOptions};
use crate::source::{SourceFile, SourceMap};
use crate::ty::{TypeParamType, TypeStore};
use inkwell::context::Context;

struct FixtureAbiInputs {
    source_map: SourceMap,
    entry_source_id: crate::source::SourceId,
    hir_compat_scaffold: crate::hir::LoweredHir,
    effect_lowered_stage_output: crate::pipeline::EffectLoweredStageOutput,
    abi_visibility_program: LateLoweredProgram,
}

fn refactor_session() -> Session {
    Session::with_options(SessionOptions::new()).unwrap()
}

fn load_fixture(phase: &str, name: &str) -> SourceFile {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(phase)
        .join(name);
    SourceFile::load(&path).expect("fixture 应可加载")
}

fn load_build_fixture(name: &str) -> SourceFile {
    load_fixture("build", name)
}

fn build_fixture_inputs_from_source(source: SourceFile) -> FixtureAbiInputs {
    let session = refactor_session();
    let typed_hir_output =
        load_typed_hir_stage_output_for_dump(&session, &source).expect("typed HIR stage 应成功");
    let hir_compat_scaffold = typed_hir_output
        .lowered_hir()
        .clone_hir_compat_scaffold_without_materialized_mir();
    let facts = MirLoweringFacts::from_refactor_typed_handoff(
        typed_hir_output.lowered_hir(),
        typed_hir_output.effect_contracts(),
    );
    let effect_contracts = typed_hir_output.effect_contracts().clone();
    let mut lowered_hir = typed_hir_output.into_lowered_hir();
    let builtins = lowered_hir.types.intern_builtins();
    let file = lower_hir_file_for_dump_with_facts(
        builtins,
        &mut lowered_hir.types,
        &lowered_hir.file,
        &lowered_hir.member_funs,
        &facts,
    );
    let types = std::mem::replace(&mut lowered_hir.types, TypeStore::new());
    let materialized_mir = lowered_hir.into_materialized_mir();
    let mir_stage_output = MirStageOutput::new(
        LoweredMir { file, types },
        effect_contracts,
        materialized_mir,
    );
    let effect_facts_stage_output =
        build_effect_facts_stage_output(&session, &source, mir_stage_output)
            .expect("effect facts stage 应成功");
    let effect_lowered_stage_output =
        build_effect_lowered_stage_output(&session, effect_facts_stage_output)
            .expect("effect lowered stage 应成功");
    // ABI materializer 必须消费与真实 refactor LLVM stage 相同的 shell-preserving handoff，
    // 不能误用会裁剪 published resume methods 的 authoritative reachable-body program。
    let abi_visibility_program = optimize_program_with_options(
        LateLoweredProgramBuilder::from_canonical_inputs(
            effect_lowered_stage_output.materialized_pass_view(),
            effect_lowered_stage_output.effect_facts(),
            effect_lowered_stage_output.types(),
        )
        .build()
        .expect("ABI visibility late-lowered program 应成功"),
        LateLoweredOptOptions::preserve_published_resume_shells(),
    );
    let input_sources = vec![source.clone()];
    let (source_map, entry_source_id) =
        crate::llvm::frontend::build_source_map_with_extra_sources(&session, &input_sources, 0);
    FixtureAbiInputs {
        source_map,
        entry_source_id,
        hir_compat_scaffold,
        effect_lowered_stage_output,
        abi_visibility_program,
    }
}

fn build_fixture_inputs(name: &str) -> FixtureAbiInputs {
    build_fixture_inputs_from_source(load_build_fixture(name))
}

fn with_inputs_query_result(
    inputs: FixtureAbiInputs,
    rewrite_program: impl FnOnce(&FixtureAbiInputs) -> LateLoweredProgram,
    check: impl for<'ctx> FnOnce(
        &FixtureAbiInputs,
        Result<ProgramAbiQuery<'ctx>, LlvmEmitError>,
        &inkwell::module::Module<'ctx>,
    ),
) {
    let program = rewrite_program(&inputs);
    let context = Context::create();
    let module = context.create_module("refactor_abi_test");
    let builder = context.create_builder();
    let target_info = target::configure_module_for_host(&module).expect("host target 应可配置");
    let target_data = inkwell::targets::TargetData::create(&target_info.data_layout);
    let lowered = &inputs.hir_compat_scaffold;
    let fun_index: HashMap<String, &crate::hir::FunDecl> = lowered
        .file
        .items
        .iter()
        .filter_map(|item| match item {
            crate::hir::Item::Fun(fun) => Some(fun),
            _ => None,
        })
        .chain(lowered.member_funs.iter())
        .map(|fun| (fun.fqn.clone(), fun))
        .collect();
    let program_facts = Rc::new(ProgramFacts::from_lowered(lowered));
    let effect_op_tags = Rc::new(RefCell::new(EffectOpTagState::new()));
    let unit_codegen = CompilationUnitCodegenCx::new(CompilationUnitCodegenInputs {
        context: &context,
        module: &module,
        builder: &builder,
        target_data: &target_data,
        host: &target_info,
        source_map: &inputs.source_map,
        entry_source_id: inputs.entry_source_id,
        stable_cone_key: &lowered.stable_cone_key,
        stable_type_param_keys: &lowered.stable_type_param_keys,
        types: &lowered.types,
        struct_layouts: &lowered.struct_layouts,
        enum_layouts: &lowered.enum_layouts,
        top_level_vars: &lowered.top_level_vars,
        top_level_consts: &lowered.top_level_consts,
        top_level_immutable_values: &lowered.top_level_immutable_values,
        top_level_fun_call_sites: &lowered.top_level_fun_call_sites,
        extern_globals: &lowered.extern_globals,
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
        fun_index: &fun_index,
        materialized_pass_view: Some(inputs.effect_lowered_stage_output.materialized_pass_view()),
        published_late_lowered_program: Some(&program),
        program_facts,
        effect_op_tags,
    });
    let mut codegen = unit_codegen.fresh_main_codegen();
    let result = codegen.materialize_program_abi(
        &program,
        inputs.effect_lowered_stage_output.types(),
        &inputs.effect_lowered_stage_output.materialized_pass_view(),
        inputs.effect_lowered_stage_output.effect_facts(),
    );
    check(&inputs, result, &module);
}

fn with_inputs_query_result_for_source_types(
    inputs: FixtureAbiInputs,
    rewrite_program: impl FnOnce(&FixtureAbiInputs) -> LateLoweredProgram,
    rewrite_source_types: impl FnOnce(&FixtureAbiInputs) -> TypeStore,
    check: impl for<'ctx> FnOnce(
        &FixtureAbiInputs,
        Result<ProgramAbiQuery<'ctx>, LlvmEmitError>,
        &inkwell::module::Module<'ctx>,
    ),
) {
    let program = rewrite_program(&inputs);
    let source_types = rewrite_source_types(&inputs);
    let context = Context::create();
    let module = context.create_module("refactor_abi_test");
    let builder = context.create_builder();
    let target_info = target::configure_module_for_host(&module).expect("host target 应可配置");
    let target_data = inkwell::targets::TargetData::create(&target_info.data_layout);
    let lowered = &inputs.hir_compat_scaffold;
    let fun_index: HashMap<String, &crate::hir::FunDecl> = lowered
        .file
        .items
        .iter()
        .filter_map(|item| match item {
            crate::hir::Item::Fun(fun) => Some(fun),
            _ => None,
        })
        .chain(lowered.member_funs.iter())
        .map(|fun| (fun.fqn.clone(), fun))
        .collect();
    let program_facts = Rc::new(ProgramFacts::from_lowered(lowered));
    let effect_op_tags = Rc::new(RefCell::new(EffectOpTagState::new()));
    let unit_codegen = CompilationUnitCodegenCx::new(CompilationUnitCodegenInputs {
        context: &context,
        module: &module,
        builder: &builder,
        target_data: &target_data,
        host: &target_info,
        source_map: &inputs.source_map,
        entry_source_id: inputs.entry_source_id,
        stable_cone_key: &lowered.stable_cone_key,
        stable_type_param_keys: &lowered.stable_type_param_keys,
        types: &lowered.types,
        struct_layouts: &lowered.struct_layouts,
        enum_layouts: &lowered.enum_layouts,
        top_level_vars: &lowered.top_level_vars,
        top_level_consts: &lowered.top_level_consts,
        top_level_immutable_values: &lowered.top_level_immutable_values,
        top_level_fun_call_sites: &lowered.top_level_fun_call_sites,
        extern_globals: &lowered.extern_globals,
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
        fun_index: &fun_index,
        materialized_pass_view: Some(inputs.effect_lowered_stage_output.materialized_pass_view()),
        published_late_lowered_program: Some(&program),
        program_facts,
        effect_op_tags,
    });
    let mut codegen = unit_codegen.fresh_main_codegen();
    let result = codegen.materialize_program_abi(
        &program,
        &source_types,
        &inputs.effect_lowered_stage_output.materialized_pass_view(),
        inputs.effect_lowered_stage_output.effect_facts(),
    );
    check(&inputs, result, &module);
}

fn with_inputs_query_result_and_codegen(
    inputs: FixtureAbiInputs,
    rewrite_program: impl FnOnce(&FixtureAbiInputs) -> LateLoweredProgram,
    check: impl for<'ctx> FnOnce(
        &FixtureAbiInputs,
        &mut MainCodegen<'_, 'ctx>,
        Result<ProgramAbiQuery<'ctx>, LlvmEmitError>,
        &inkwell::module::Module<'ctx>,
    ),
) {
    let program = rewrite_program(&inputs);
    let context = Context::create();
    let module = context.create_module("refactor_abi_test");
    let builder = context.create_builder();
    let target_info = target::configure_module_for_host(&module).expect("host target 应可配置");
    let target_data = inkwell::targets::TargetData::create(&target_info.data_layout);
    let lowered = &inputs.hir_compat_scaffold;
    let fun_index: HashMap<String, &crate::hir::FunDecl> = lowered
        .file
        .items
        .iter()
        .filter_map(|item| match item {
            crate::hir::Item::Fun(fun) => Some(fun),
            _ => None,
        })
        .chain(lowered.member_funs.iter())
        .map(|fun| (fun.fqn.clone(), fun))
        .collect();
    let program_facts = Rc::new(ProgramFacts::from_lowered(lowered));
    let effect_op_tags = Rc::new(RefCell::new(EffectOpTagState::new()));
    let unit_codegen = CompilationUnitCodegenCx::new(CompilationUnitCodegenInputs {
        context: &context,
        module: &module,
        builder: &builder,
        target_data: &target_data,
        host: &target_info,
        source_map: &inputs.source_map,
        entry_source_id: inputs.entry_source_id,
        stable_cone_key: &lowered.stable_cone_key,
        stable_type_param_keys: &lowered.stable_type_param_keys,
        types: &lowered.types,
        struct_layouts: &lowered.struct_layouts,
        enum_layouts: &lowered.enum_layouts,
        top_level_vars: &lowered.top_level_vars,
        top_level_consts: &lowered.top_level_consts,
        top_level_immutable_values: &lowered.top_level_immutable_values,
        top_level_fun_call_sites: &lowered.top_level_fun_call_sites,
        extern_globals: &lowered.extern_globals,
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
        fun_index: &fun_index,
        materialized_pass_view: Some(inputs.effect_lowered_stage_output.materialized_pass_view()),
        published_late_lowered_program: Some(&program),
        program_facts,
        effect_op_tags,
    });
    let mut codegen = unit_codegen.fresh_main_codegen();
    let pass_view = inputs.effect_lowered_stage_output.materialized_pass_view();
    let result = codegen.materialize_program_abi(
        &program,
        inputs.effect_lowered_stage_output.types(),
        &pass_view,
        inputs.effect_lowered_stage_output.effect_facts(),
    );
    check(&inputs, &mut codegen, result, &module);
}

fn with_fixture_query_result(
    name: &str,
    rewrite_program: impl FnOnce(&FixtureAbiInputs) -> LateLoweredProgram,
    check: impl for<'ctx> FnOnce(
        &FixtureAbiInputs,
        Result<ProgramAbiQuery<'ctx>, LlvmEmitError>,
        &inkwell::module::Module<'ctx>,
    ),
) {
    with_inputs_query_result(build_fixture_inputs(name), rewrite_program, check);
}

fn with_phase_fixture_query_result(
    phase: &str,
    name: &str,
    rewrite_program: impl FnOnce(&FixtureAbiInputs) -> LateLoweredProgram,
    check: impl for<'ctx> FnOnce(
        &FixtureAbiInputs,
        Result<ProgramAbiQuery<'ctx>, LlvmEmitError>,
        &inkwell::module::Module<'ctx>,
    ),
) {
    with_inputs_query_result(
        build_fixture_inputs_from_source(load_fixture(phase, name)),
        rewrite_program,
        check,
    );
}

fn with_fixture_query(
    name: &str,
    check: impl for<'ctx> FnOnce(
        &FixtureAbiInputs,
        &ProgramAbiQuery<'ctx>,
        &inkwell::module::Module<'ctx>,
    ),
) {
    with_fixture_query_result(
        name,
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, module| {
            let query = result.expect("refactor ABI materialization 应成功");
            check(inputs, &query, module);
        },
    );
}

fn clone_callable_with_interfaces(
    callable: &LateLoweredCallable,
    resume_interfaces: Vec<ResumeInterfaceId>,
) -> LateLoweredCallable {
    LateLoweredCallable::new(
        callable.root_fqn().to_string(),
        callable.stable_instance_key().clone(),
        callable.body_version_key().clone(),
        callable.step_schema(),
        callable.resolved_outward_cases().to_vec(),
        callable.dynamic_invoke_entry().clone(),
        callable.state_graph().clone(),
        callable.frame_schema().clone(),
        callable.boundary_map().clone(),
        callable.resume_state_map().clone(),
        callable.continuation_object(),
        resume_interfaces,
    )
    .with_source_statement_classifications(callable.source_statement_classifications().to_vec())
}

fn clone_continuation_object_with_interfaces(
    object: &LateLoweredContinuationObject,
    implemented_interfaces: Vec<ResumeInterfaceId>,
) -> LateLoweredContinuationObject {
    LateLoweredContinuationObject::new(
        object.object_id(),
        object.owner_version_key().clone(),
        object.continuation_obj_ty(),
        implemented_interfaces,
        object.captures().to_vec(),
        object.surface_resumes().to_vec(),
        object.methods().to_vec(),
    )
}

fn clone_continuation_object_with_surface_resumes(
    object: &LateLoweredContinuationObject,
    surface_resumes: Vec<LateLoweredContinuationSurfaceResume>,
) -> LateLoweredContinuationObject {
    LateLoweredContinuationObject::new(
        object.object_id(),
        object.owner_version_key().clone(),
        object.continuation_obj_ty(),
        object.implemented_packings().to_vec(),
        object.captures().to_vec(),
        surface_resumes,
        object.methods().to_vec(),
    )
}

fn clone_continuation_object_with_methods(
    object: &LateLoweredContinuationObject,
    methods: Vec<crate::effect_lowered::ir::LateLoweredContinuationMethod>,
) -> LateLoweredContinuationObject {
    LateLoweredContinuationObject::new(
        object.object_id(),
        object.owner_version_key().clone(),
        object.continuation_obj_ty(),
        object.implemented_packings().to_vec(),
        object.captures().to_vec(),
        object.surface_resumes().to_vec(),
        methods,
    )
}

fn clone_continuation_object_with_id(
    object: &LateLoweredContinuationObject,
    object_id: ContinuationObjectId,
) -> LateLoweredContinuationObject {
    LateLoweredContinuationObject::new(
        object_id,
        object.owner_version_key().clone(),
        object.continuation_obj_ty(),
        object.implemented_packings().to_vec(),
        object.captures().to_vec(),
        object.surface_resumes().to_vec(),
        object.methods().to_vec(),
    )
}

fn clone_callable_with_boundary_map(
    callable: &LateLoweredCallable,
    boundary_map: LateLoweredBoundaryMap,
) -> LateLoweredCallable {
    if let Some(plain) = callable.plain_abi() {
        let local = callable
            .plain_local_effect_control()
            .expect("plain callable 应发布 local effect/control 才能替换 boundary map");
        return clone_plain_callable_with_local_control(
            callable,
            plain,
            LateLoweredPlainLocalEffectControl::new(
                local.step_schema(),
                local.state_graph().clone(),
                local.frame_schema().clone(),
                boundary_map,
                local.resume_state_map().clone(),
                local.source_statement_classifications().to_vec(),
                local.continuation_object(),
                local.resume_packings().to_vec(),
            ),
        );
    }
    LateLoweredCallable::new(
        callable.root_fqn().to_string(),
        callable.stable_instance_key().clone(),
        callable.body_version_key().clone(),
        callable.step_schema(),
        callable.resolved_outward_cases().to_vec(),
        callable.dynamic_invoke_entry().clone(),
        callable.state_graph().clone(),
        callable.frame_schema().clone(),
        boundary_map,
        callable.resume_state_map().clone(),
        callable.continuation_object(),
        callable.resume_packings().to_vec(),
    )
    .with_source_statement_classifications(callable.source_statement_classifications().to_vec())
}

fn clone_callable_with_state_graph(
    callable: &LateLoweredCallable,
    state_graph: LateLoweredStateGraph,
) -> LateLoweredCallable {
    if let Some(plain) = callable.plain_abi() {
        let local = callable
            .plain_local_effect_control()
            .expect("plain callable 应发布 local effect/control 才能替换 state graph");
        return clone_plain_callable_with_local_control(
            callable,
            plain,
            LateLoweredPlainLocalEffectControl::new(
                local.step_schema(),
                state_graph,
                local.frame_schema().clone(),
                local.boundary_map().clone(),
                local.resume_state_map().clone(),
                local.source_statement_classifications().to_vec(),
                local.continuation_object(),
                local.resume_packings().to_vec(),
            ),
        );
    }
    LateLoweredCallable::new(
        callable.root_fqn().to_string(),
        callable.stable_instance_key().clone(),
        callable.body_version_key().clone(),
        callable.step_schema(),
        callable.resolved_outward_cases().to_vec(),
        callable.dynamic_invoke_entry().clone(),
        state_graph,
        callable.frame_schema().clone(),
        callable.boundary_map().clone(),
        callable.resume_state_map().clone(),
        callable.continuation_object(),
        callable.resume_packings().to_vec(),
    )
    .with_source_statement_classifications(callable.source_statement_classifications().to_vec())
}

fn clone_callable_with_frame_schema(
    callable: &LateLoweredCallable,
    frame_schema: LateLoweredFrameSchema,
) -> LateLoweredCallable {
    if let Some(plain) = callable.plain_abi() {
        let local = callable
            .plain_local_effect_control()
            .expect("plain callable 应发布 local effect/control 才能替换 frame schema");
        return clone_plain_callable_with_local_control(
            callable,
            plain,
            LateLoweredPlainLocalEffectControl::new(
                local.step_schema(),
                local.state_graph().clone(),
                frame_schema,
                local.boundary_map().clone(),
                local.resume_state_map().clone(),
                local.source_statement_classifications().to_vec(),
                local.continuation_object(),
                local.resume_packings().to_vec(),
            ),
        );
    }
    LateLoweredCallable::new(
        callable.root_fqn().to_string(),
        callable.stable_instance_key().clone(),
        callable.body_version_key().clone(),
        callable.step_schema(),
        callable.resolved_outward_cases().to_vec(),
        callable.dynamic_invoke_entry().clone(),
        callable.state_graph().clone(),
        frame_schema,
        callable.boundary_map().clone(),
        callable.resume_state_map().clone(),
        callable.continuation_object(),
        callable.resume_packings().to_vec(),
    )
    .with_source_statement_classifications(callable.source_statement_classifications().to_vec())
}

fn clone_plain_callable_with_local_control(
    callable: &LateLoweredCallable,
    plain: &LateLoweredPlainCallable,
    local: LateLoweredPlainLocalEffectControl,
) -> LateLoweredCallable {
    LateLoweredCallable::new_plain(
        callable.root_fqn().to_string(),
        callable.stable_instance_key().clone(),
        callable.body_version_key().clone(),
        callable.resolved_outward_cases().to_vec(),
        LateLoweredPlainCallable::new(
            plain.function_ty(),
            plain.param_tys().to_vec(),
            plain.return_ty(),
            plain.body_slices().to_vec(),
            plain.call_sites().to_vec(),
            Some(local),
        ),
    )
}

fn clone_callable_with_source_statement_classifications(
    callable: &LateLoweredCallable,
    classifications: Vec<LateLoweredSourceStatementClassification>,
) -> LateLoweredCallable {
    if let Some(plain) = callable.plain_abi() {
        let local = callable
            .plain_local_effect_control()
            .expect("plain callable 应发布 local effect/control 才能替换 classifications");
        return clone_plain_callable_with_local_control(
            callable,
            plain,
            LateLoweredPlainLocalEffectControl::new(
                local.step_schema(),
                local.state_graph().clone(),
                local.frame_schema().clone(),
                local.boundary_map().clone(),
                local.resume_state_map().clone(),
                classifications,
                local.continuation_object(),
                local.resume_packings().to_vec(),
            ),
        );
    }
    LateLoweredCallable::new(
        callable.root_fqn().to_string(),
        callable.stable_instance_key().clone(),
        callable.body_version_key().clone(),
        callable.step_schema(),
        callable.resolved_outward_cases().to_vec(),
        callable.dynamic_invoke_entry().clone(),
        callable.state_graph().clone(),
        callable.frame_schema().clone(),
        callable.boundary_map().clone(),
        callable.resume_state_map().clone(),
        callable.continuation_object(),
        callable.resume_packings().to_vec(),
    )
    .with_source_statement_classifications(classifications)
}

fn clone_state_graph_with_handle_contract(
    state_graph: &crate::effect_lowered::ir::LateLoweredStateGraph,
    site_id: SiteId,
    new_contract: LateLoweredHandleDispatchContract,
) -> crate::effect_lowered::ir::LateLoweredStateGraph {
    let states = state_graph
        .states()
        .iter()
        .map(|state| match state.terminator() {
            crate::effect_lowered::ir::LateLoweredStateTerminator::HandleDispatch {
                site_id: state_site,
                body_state,
                arm_states,
                finally_state,
                exit_state,
                boundary_ids,
                drop_state,
                ..
            } if *state_site == site_id => crate::effect_lowered::ir::LateLoweredState::new(
                state.state_id(),
                state.role(),
                state.source_slices().to_vec(),
                crate::effect_lowered::ir::LateLoweredStateTerminator::HandleDispatch {
                    site_id: *state_site,
                    body_state: *body_state,
                    arm_states: arm_states.clone(),
                    finally_state: *finally_state,
                    exit_state: *exit_state,
                    contract: new_contract.clone(),
                    boundary_ids: boundary_ids.clone(),
                    drop_state: *drop_state,
                },
            ),
            _ => state.clone(),
        })
        .collect();
    crate::effect_lowered::ir::LateLoweredStateGraph::new(
        state_graph.entry_state(),
        state_graph.complete_state(),
        state_graph.cleanup_state(),
        state_graph.drop_state(),
        states,
    )
}

fn handle_dispatch_contract(
    callable: &LateLoweredCallable,
    site_id: SiteId,
) -> &LateLoweredHandleDispatchContract {
    callable
        .state_graph()
        .states()
        .iter()
        .find_map(|state| match state.terminator() {
            crate::effect_lowered::ir::LateLoweredStateTerminator::HandleDispatch {
                site_id: state_site,
                contract,
                ..
            } if *state_site == site_id => Some(contract),
            _ => None,
        })
        .expect("应找到指定 site 的 HandleDispatch contract")
}

fn first_handle_dispatch(
    callable: &LateLoweredCallable,
) -> (SiteId, &LateLoweredHandleDispatchContract) {
    callable
        .state_graph()
        .states()
        .iter()
        .find_map(|state| match state.terminator() {
            crate::effect_lowered::ir::LateLoweredStateTerminator::HandleDispatch {
                site_id,
                contract,
                ..
            } => Some((*site_id, contract)),
            _ => None,
        })
        .expect("应找到至少一个 HandleDispatch contract")
}

fn handle_dispatch_with_pending_outward(
    callable: &LateLoweredCallable,
) -> (SiteId, &LateLoweredHandleDispatchContract) {
    callable
        .state_graph()
        .states()
        .iter()
        .find_map(|state| match state.terminator() {
            crate::effect_lowered::ir::LateLoweredStateTerminator::HandleDispatch {
                site_id,
                contract,
                ..
            } if contract.pending_completions().iter().any(|completion| {
                matches!(
                    completion,
                    LateLoweredHandlePendingCompletion::PropagateOutward(_)
                )
            }) =>
            {
                Some((*site_id, contract))
            }
            _ => None,
        })
        .expect("应找到带 pending outward completion 的 HandleDispatch contract")
}

fn clone_handle_dispatch_contract_with_handled_arms(
    contract: &LateLoweredHandleDispatchContract,
    handled_arms: Vec<crate::effect_lowered::ir::LateLoweredHandleArmDispatch>,
) -> LateLoweredHandleDispatchContract {
    LateLoweredHandleDispatchContract::new(
        contract.carrier(),
        contract.body_complete_target(),
        contract.arm_complete_target(),
        contract.finally_complete_target(),
        contract.body_completion_payload_source().cloned(),
        handled_arms,
        contract.body_outward_cases().to_vec(),
        contract.finally_outward_cases().to_vec(),
        contract.outward_emissions().to_vec(),
        contract.pending_completions().to_vec(),
        contract.pending_completion_origins().to_vec(),
        contract.pending_payload_transports().to_vec(),
        contract.state_regions().to_vec(),
        contract.boundary_routings().to_vec(),
        contract.abandon_target(),
    )
}

fn clone_handle_dispatch_contract_with_regions_and_routes(
    contract: &LateLoweredHandleDispatchContract,
    state_regions: Vec<crate::effect_lowered::ir::LateLoweredHandleStateRegionEntry>,
    boundary_routings: Vec<crate::effect_lowered::ir::LateLoweredHandleBoundaryRouting>,
) -> LateLoweredHandleDispatchContract {
    LateLoweredHandleDispatchContract::new(
        contract.carrier(),
        contract.body_complete_target(),
        contract.arm_complete_target(),
        contract.finally_complete_target(),
        contract.body_completion_payload_source().cloned(),
        contract.handled_arms().to_vec(),
        contract.body_outward_cases().to_vec(),
        contract.finally_outward_cases().to_vec(),
        contract.outward_emissions().to_vec(),
        contract.pending_completions().to_vec(),
        contract.pending_completion_origins().to_vec(),
        contract.pending_payload_transports().to_vec(),
        state_regions,
        boundary_routings,
        contract.abandon_target(),
    )
}

fn clone_callable_with_dynamic_invoke_entry(
    callable: &LateLoweredCallable,
    dynamic_invoke_entry: LateLoweredDynamicInvokeEntry,
) -> LateLoweredCallable {
    LateLoweredCallable::new(
        callable.root_fqn().to_string(),
        callable.stable_instance_key().clone(),
        callable.body_version_key().clone(),
        callable.step_schema(),
        callable.resolved_outward_cases().to_vec(),
        dynamic_invoke_entry,
        callable.state_graph().clone(),
        callable.frame_schema().clone(),
        callable.boundary_map().clone(),
        callable.resume_state_map().clone(),
        callable.continuation_object(),
        callable.resume_packings().to_vec(),
    )
    .with_source_statement_classifications(callable.source_statement_classifications().to_vec())
}

fn duplicate_no_outward_callable_version(
    program: &LateLoweredProgram,
    root_fqn: &str,
) -> LateLoweredProgram {
    let callable = program
        .callables()
        .iter()
        .find(|callable| callable.root_fqn() == root_fqn)
        .unwrap_or_else(|| panic!("应存在 callable `{root_fqn}`"));
    assert_eq!(
        callable.impl_plan(),
        ImplPlan::NoOutward,
        "当前 helper 只支持 NoOutward callable version"
    );
    let plain = callable
        .plain_abi()
        .expect("NoOutward callable 应保持 plain ABI")
        .clone();
    let cloned_version_key = LateLoweredBodyVersionKey::new(
        callable.instance_key().clone(),
        callable.allowed_row().clone(),
        callable.impl_plan(),
        !callable.needs_reentry(),
    );

    let mut callables = program.callables().to_vec();
    callables.push(LateLoweredCallable::new_plain(
        callable.root_fqn().to_string(),
        callable.stable_instance_key().clone(),
        cloned_version_key,
        callable.resolved_outward_cases().to_vec(),
        plain,
    ));

    LateLoweredProgram::new(
        program.step_types().to_vec(),
        program.resume_packings().to_vec(),
        program.continuation_objects().to_vec(),
        callables,
    )
    .with_stable_instance_keys(program.stable_instance_keys().clone())
}

fn site_boundary(callable: &LateLoweredCallable, kind: BoundarySiteKind) -> &LateLoweredBoundary {
    callable
        .boundary_map()
        .entries()
        .iter()
        .find(|boundary| {
            matches!(
                boundary.source(),
                LateLoweredBoundarySource::Site {
                    kind: boundary_kind,
                    ..
                } if boundary_kind == kind
            )
        })
        .expect("应找到指定 kind 的 boundary")
}

fn call_boundary_lowering(boundary: &LateLoweredBoundary) -> &LateLoweredCallBoundaryLowering {
    let Some(LateLoweredBoundaryLowering::Call(lowering)) = boundary.lowering() else {
        panic!("boundary 应物化成 Call lowering");
    };
    lowering
}

fn perform_boundary_lowering(
    boundary: &LateLoweredBoundary,
) -> &crate::effect_lowered::ir::LateLoweredPerformBoundaryLowering {
    let Some(LateLoweredBoundaryLowering::Perform(lowering)) = boundary.lowering() else {
        panic!("boundary 应物化成 Perform lowering");
    };
    lowering
}

fn resume_boundary_lowering(
    boundary: &LateLoweredBoundary,
) -> &crate::effect_lowered::ir::LateLoweredResumeBoundaryLowering {
    let Some(LateLoweredBoundaryLowering::Resume(lowering)) = boundary.lowering() else {
        panic!("boundary 应物化成 Resume lowering");
    };
    lowering
}

fn boundary_site_id(boundary: &LateLoweredBoundary) -> crate::mir::SiteId {
    let LateLoweredBoundarySource::Site { site_id, .. } = boundary.source() else {
        panic!("boundary 应带 site source");
    };
    site_id
}

fn handle_dispatch_state(
    callable: &LateLoweredCallable,
    site_id: SiteId,
) -> &crate::effect_lowered::ir::LateLoweredState {
    callable
        .state_graph()
        .states()
        .iter()
        .find(|state| {
            matches!(
                state.terminator(),
                LateLoweredStateTerminator::HandleDispatch { site_id: state_site, .. }
                    if *state_site == site_id
            )
        })
        .expect("应找到指定 site 的 HandleDispatch state")
}

fn source_slice_non_boundary_dynamic_call_site(
    inputs: &FixtureAbiInputs,
    callable: &LateLoweredCallable,
) -> (crate::mir::SiteId, CallSiteEffectFacts) {
    if let Some(plain) = callable.plain_abi() {
        return plain
            .call_sites()
            .iter()
            .find_map(|site| {
                (site.facts().target_mode() != CallTargetMode::KnownInstance)
                    .then(|| (site.site_id(), site.facts().clone()))
            })
            .expect("plain callable 应发布一个 non-boundary source-slice dynamic call site");
    }

    let body = inputs
        .effect_lowered_stage_output
        .materialized_pass_view()
        .callable(callable.root_fqn())
        .expect("callable 的 canonical MIR body 应存在")
        .body
        .as_ref()
        .expect("callable 的 canonical MIR body 内容应存在");
    let body_facts = inputs
        .effect_lowered_stage_output
        .effect_facts()
        .body(callable.instance_key())
        .expect("callable 的 BodyEffectFacts 应存在");
    let boundary_call_sites = callable
        .boundary_map()
        .entries()
        .iter()
        .filter_map(|boundary| match boundary.source() {
            LateLoweredBoundarySource::Site {
                site_id,
                kind: BoundarySiteKind::Call,
            } => Some(site_id),
            LateLoweredBoundarySource::RuntimeError { .. }
            | LateLoweredBoundarySource::Site { .. } => None,
        })
        .collect::<BTreeSet<_>>();

    for state in callable.state_graph().states() {
        for slice in state.source_slices() {
            let block = &body.blocks[slice.block_id().as_u32() as usize];
            let start = slice.start_statement_index() as usize;
            let end = slice.end_statement_index() as usize;
            for stmt in &block.stmts[start..end] {
                let MirStatementKind::Assign {
                    value: MirRvalue::Call { site_id, kind, .. },
                    ..
                } = &stmt.kind
                else {
                    continue;
                };
                if boundary_call_sites.contains(site_id)
                    || !matches!(
                        kind,
                        MirCallKind::FunValue { .. }
                            | MirCallKind::FunPtr { .. }
                            | MirCallKind::Closure { .. }
                            | MirCallKind::Virtual { .. }
                            | MirCallKind::Interface { .. }
                    )
                {
                    continue;
                }
                let SiteEffectFacts::Call(facts) = body_facts
                    .site(*site_id)
                    .expect("source-slice dynamic call site 应带 published Call facts")
                else {
                    panic!("source-slice dynamic call site 必须对应 Call facts");
                };
                if facts.target_mode() == CallTargetMode::KnownInstance {
                    continue;
                }
                return (*site_id, facts.clone());
            }
        }
    }

    panic!("应找到一个 non-boundary source-slice dynamic call site");
}

fn clone_resume_interface_with_methods(
    interface: &LateLoweredResumeInterface,
    methods: Vec<LateLoweredResumeMethod>,
) -> LateLoweredResumeInterface {
    LateLoweredResumeInterface::new(
        interface.interface_id(),
        interface.effect_family().clone(),
        interface.return_step_schema(),
        methods,
    )
}

fn single_case_worker_program_with_ping_method_order(
    inputs: &FixtureAbiInputs,
    method_case_order: &[CaseTag],
) -> LateLoweredProgram {
    let program = &inputs.abi_visibility_program;
    let callable = program
        .callable("fixtures.build.singleCaseWorker")
        .expect("callable 应存在");
    let step_type = program
        .step_type(callable.step_schema())
        .expect("step type 应存在");
    let ping_interface = program
        .resume_packings()
        .iter()
        .find(|interface| interface.effect_family().effect_fqn() == "fixtures.build.Ping")
        .expect("应存在 Ping resume packing");
    let methods = method_case_order
        .iter()
        .map(|case_tag| {
            ping_interface
                .methods()
                .iter()
                .find(|method| method.case_tag() == *case_tag)
                .cloned()
                .unwrap_or_else(|| {
                    let step_case = step_type
                        .case(*case_tag)
                        .expect("method case 应可回查 step shell");
                    LateLoweredResumeMethod::new(
                        step_case.case_tag(),
                        step_case.concrete_op_key().clone(),
                        step_case.continuation_contract(),
                    )
                })
        })
        .collect::<Vec<_>>();
    let resume_interfaces = program
        .resume_packings()
        .iter()
        .map(|candidate| {
            if candidate.interface_id() == ping_interface.interface_id() {
                clone_resume_interface_with_methods(candidate, methods.clone())
            } else {
                candidate.clone()
            }
        })
        .collect();

    LateLoweredProgram::new(
        program.step_types().to_vec(),
        resume_interfaces,
        program.continuation_objects().to_vec(),
        program.callables().to_vec(),
    )
    .with_stable_instance_keys(program.stable_instance_keys().clone())
}

fn resume_method_for_case(
    step_type: &LateLoweredStepType,
    case_tag: CaseTag,
) -> LateLoweredResumeMethod {
    let step_case = step_type
        .case(case_tag)
        .expect("method case 应可回查 step shell");
    LateLoweredResumeMethod::new(
        step_case.case_tag(),
        step_case.concrete_op_key().clone(),
        step_case.continuation_contract(),
    )
}

fn next_resume_interface_id(program: &LateLoweredProgram) -> ResumeInterfaceId {
    let next = program
        .resume_packings()
        .iter()
        .map(|interface| interface.interface_id().as_u32())
        .max()
        .map(|raw| raw.saturating_add(1))
        .unwrap_or(0);
    ResumeInterfaceId::new(next)
}

fn unit_worker_program_with_ping_interface(inputs: &FixtureAbiInputs) -> LateLoweredProgram {
    let program = &inputs.abi_visibility_program;
    let callable = program
        .callable("fixtures.build.unitWorker")
        .expect("callable 应存在");
    let step_type = program
        .step_type(callable.step_schema())
        .expect("step type 应存在");
    let ping_method = resume_method_for_case(step_type, CaseTag::new(0));
    let ping_interface_id = program
        .resume_packings()
        .iter()
        .find(|interface| interface.effect_family().effect_fqn() == "fixtures.build.Ping")
        .map(LateLoweredResumeInterface::interface_id)
        .unwrap_or_else(|| next_resume_interface_id(program));
    let ping_interface = LateLoweredResumeInterface::new(
        ping_interface_id,
        ping_method.concrete_op_key().effect_family().clone(),
        callable.step_schema(),
        vec![ping_method],
    );

    let resume_interfaces = program
        .resume_packings()
        .iter()
        .filter(|interface| interface.interface_id() != ping_interface_id)
        .cloned()
        .chain(std::iter::once(ping_interface))
        .collect();
    let callables = program
        .callables()
        .iter()
        .map(|candidate| {
            if candidate.body_step_schema() == Some(callable.step_schema()) {
                clone_callable_with_interfaces(candidate, vec![ping_interface_id])
            } else {
                candidate.clone()
            }
        })
        .collect();
    let continuation_objects = program
        .continuation_objects()
        .iter()
        .map(|candidate| {
            if candidate.object_id() == callable.continuation_object() {
                clone_continuation_object_with_interfaces(candidate, vec![ping_interface_id])
            } else {
                candidate.clone()
            }
        })
        .collect();

    LateLoweredProgram::new(
        program.step_types().to_vec(),
        resume_interfaces,
        continuation_objects,
        callables,
    )
    .with_stable_instance_keys(program.stable_instance_keys().clone())
}

#[test]
fn refactor_llvm_source_slice_classification_rejects_missing_handoff() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let mut removed = false;
            let callables = program
                .callables()
                .iter()
                .map(|callable| {
                    if !removed && !callable.source_statement_classifications().is_empty() {
                        removed = true;
                        clone_callable_with_source_statement_classifications(callable, Vec::new())
                    } else {
                        callable.clone()
                    }
                })
                .collect();
            assert!(
                removed,
                "fixture 应发布至少一个 source statement classification"
            );
            LateLoweredProgram::new(
                program.step_types().to_vec(),
                program.resume_packings().to_vec(),
                program.continuation_objects().to_vec(),
                callables,
            )
            .with_stable_instance_keys(program.stable_instance_keys().clone())
            .with_stable_instance_keys(program.stable_instance_keys().clone())
        },
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => panic!("缺失 classification handoff 必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(message.contains("source-slice statement"));
            assert!(message.contains("classification"));
        },
    );
}

#[test]
fn refactor_llvm_no_outward_plain_abi_layout_has_no_step_shell() {
    with_fixture_query(
        "effect_refactor_step_enum_no_outward.scoop",
        |inputs, query, module| {
            for fqn in ["fixtures.build.helper", "fixtures.build.main"] {
                let callable = inputs
                    .abi_visibility_program
                    .callable(fqn)
                    .expect("plain callable 应存在");
                assert!(callable.plain_abi().is_some());
                assert!(callable.body_step_schema().is_none());

                let layout = query
                    .plain_callable_layout_by_version_key(callable.body_version_key())
                    .expect("plain callable layout 应可查询");
                assert_eq!(layout.root_fqn(), fqn);
                let direct_symbol = layout.direct_entry().symbol_name();
                let expected_prefix = if fqn == "fixtures.build.main" {
                    "__scoop_abi0_fun__fixtures_build_main__h"
                } else {
                    "__scoop_abi0_fun__fixtures_build_helper__h"
                };
                assert!(
                    direct_symbol.starts_with(expected_prefix),
                    "source-level plain callable direct entry 应切到 AbiMangler namespace: {direct_symbol}"
                );
                assert!(module.get_function(direct_symbol).is_some());
                assert!(
                    query
                        .callable_layout_by_version_key(callable.body_version_key())
                        .is_err(),
                    "plain callable 不应发布 effect-step callable layout"
                );
            }

            let ir = module.print_to_string().to_string();
            assert!(
                !ir.contains("__scoop_priv0__refactor_step_case_tag_complete__h")
                    && !ir.contains("__scoop_priv0__refactor_direct_invoke__h")
                    && !ir.contains("__scoop_priv0__refactor_dynamic_invoke__h")
                    && !ir.contains("%scoop.refactor.Step__h"),
                "plain callable 不应发布 effect-step type/case-tag/dynamic-entry 家族：\n{ir}"
            );
        },
    );
}

#[test]
fn refactor_llvm_step_layout_keeps_canonical_case_set_for_single_case_callable() {
    with_fixture_query(
        "effect_refactor_step_enum_single_case.scoop",
        |inputs, query, module| {
            let callable = inputs
                .effect_lowered_stage_output
                .program()
                .callable("fixtures.build.singleCaseWorker")
                .expect("callable 应存在");
            assert_eq!(callable.impl_plan(), ImplPlan::SingleCase(CaseTag::new(0)));

            let step_layout = query
                .step_layout(callable.step_schema())
                .expect("step layout 应可查询");
            assert_eq!(step_layout.complete_variant().tag_value(), 0);
            assert_eq!(step_layout.cases().len(), 3);
            assert_eq!(
                step_layout
                    .case_layout(CaseTag::new(0))
                    .expect("case0 应存在")
                    .variant()
                    .tag_value(),
                1
            );
            assert_eq!(
                step_layout
                    .case_layout(CaseTag::new(1))
                    .expect("case1 应存在")
                    .variant()
                    .tag_value(),
                2
            );
            assert_eq!(
                step_layout
                    .case_layout(CaseTag::new(2))
                    .expect("runtime-error case 应存在")
                    .variant()
                    .tag_value(),
                3
            );
            assert!(
                module
                    .get_global(step_layout.complete_tag_constant_name())
                    .is_some()
            );
            assert!(
                module
                    .get_global(
                        step_layout
                            .case_layout(CaseTag::new(1))
                            .expect("case1 应存在")
                            .tag_constant_name(),
                    )
                    .is_some()
            );
            assert!(
                module
                    .get_global(
                        step_layout
                            .case_layout(CaseTag::new(2))
                            .expect("runtime-error case 应存在")
                            .tag_constant_name(),
                    )
                    .is_some()
            );
        },
    );
}

#[test]
fn refactor_llvm_frame_layout_preserves_slot_indices_and_system_fields() {
    with_fixture_query(
        "effect_refactor_step_enum_single_case.scoop",
        |inputs, query, _module| {
            let callable = inputs
                .effect_lowered_stage_output
                .program()
                .callable("fixtures.build.singleCaseWorker")
                .expect("callable 应存在");
            let frame_layout = query
                .frame_layout(callable.step_schema())
                .expect("frame layout 应可查询");

            assert_eq!(
                frame_layout.fields()[0].kind(),
                RefactorFrameFieldKind::Header
            );
            for (ordinal, slot) in callable.frame_schema().slots().iter().enumerate() {
                let expected_field_index = ordinal as u32 + 1;
                assert_eq!(
                    frame_layout.field_index_for_slot(slot.slot_id()),
                    Some(expected_field_index)
                );
                if let LateLoweredFrameSlotKind::System(kind) = slot.kind() {
                    assert_eq!(
                        frame_layout.field_index_for_system(kind),
                        Some(expected_field_index)
                    );
                }
            }
            for required in [
                SystemSlotKind::StateTag,
                SystemSlotKind::ResumePayloadCarrier,
                SystemSlotKind::CleanupFlag,
                SystemSlotKind::OneShotFlag,
                SystemSlotKind::CompletionTag,
                SystemSlotKind::CurrentEffectCtx,
            ] {
                assert!(
                    frame_layout.field_index_for_system(required).is_some(),
                    "frame layout 缺少系统字段 {required:?}"
                );
            }
        },
    );
}

#[test]
fn refactor_llvm_continuation_layout_keeps_full_method_set() {
    with_fixture_query_result(
        "effect_refactor_step_enum_single_case.scoop",
        |inputs| {
            single_case_worker_program_with_ping_method_order(
                inputs,
                &[CaseTag::new(0), CaseTag::new(1)],
            )
        },
        |inputs, result, module| {
            let query = result.expect("published full method set 应可物化 ABI");
            let callable = inputs
                .effect_lowered_stage_output
                .program()
                .callable("fixtures.build.singleCaseWorker")
                .expect("callable 应存在");
            let continuation_layout = query
                .continuation_layout(callable.continuation_object())
                .expect("continuation layout 应可查询");
            let callable_layout = query
                .callable_layout(callable.step_schema())
                .expect("callable layout 应可查询");
            let interface_id = *callable_layout
                .resume_packings()
                .iter()
                .find(|interface_id| {
                    query
                        .resume_packing_layout(**interface_id)
                        .is_some_and(|interface| {
                            interface.packing_family_fqn() == "fixtures.build.Ping"
                        })
                })
                .expect("应存在 Ping resume packing");
            let interface_layout = query
                .resume_packing_layout(interface_id)
                .expect("resume packing layout 应可查询");

            assert_eq!(interface_layout.methods().len(), 2);
            assert_eq!(
                interface_layout
                    .method(CaseTag::new(0))
                    .expect("case0 method 应存在")
                    .vtable_index(),
                0
            );
            assert_eq!(
                interface_layout
                    .method(CaseTag::new(1))
                    .expect("case1 method 应存在")
                    .vtable_index(),
                1
            );
            assert!(
                continuation_layout
                    .field_index_for_packing(interface_id)
                    .is_some()
            );
            assert!(
                module
                    .get_function(
                        interface_layout
                            .method(CaseTag::new(1))
                            .expect("case1 method 应存在")
                            .symbol_name(),
                    )
                    .is_some()
            );
        },
    );
}

#[test]
fn refactor_llvm_continuation_layout_uses_codegen_owned_fields() {
    with_fixture_query(
        "effect_refactor_step_enum_single_case.scoop",
        |inputs, query, _module| {
            let callable = inputs
                .effect_lowered_stage_output
                .program()
                .callable("fixtures.build.singleCaseWorker")
                .expect("callable 应存在");
            let continuation_layout = query
                .continuation_layout(callable.continuation_object())
                .expect("continuation layout 应可查询");
            let field_kinds = continuation_layout
                .fields()
                .iter()
                .take(9)
                .map(|field| field.kind())
                .collect::<Vec<_>>();

            assert_eq!(
                field_kinds,
                vec![
                    RefactorContinuationFieldKind::Header,
                    RefactorContinuationFieldKind::ResumedFlag,
                    RefactorContinuationFieldKind::ResumeStateTag,
                    RefactorContinuationFieldKind::CapturedEffectCtxRef,
                    RefactorContinuationFieldKind::StateRef,
                    RefactorContinuationFieldKind::StepFn,
                    RefactorContinuationFieldKind::ResumeWord,
                    RefactorContinuationFieldKind::ResumeGcRef,
                    RefactorContinuationFieldKind::CapturedCalleeSuspendStateRef,
                ]
            );
        },
    );
}

#[test]
fn refactor_llvm_continuation_layout_preserves_published_packing_order() {
    with_fixture_query_result(
        "effect_refactor_step_enum_single_case.scoop",
        |inputs| {
            let program = single_case_worker_program_with_ping_method_order(
                inputs,
                &[CaseTag::new(0), CaseTag::new(1)],
            );
            let callable = program
                .callable("fixtures.build.singleCaseWorker")
                .expect("callable 应存在");
            let continuation_object = program
                .continuation_object(callable.continuation_object())
                .expect("continuation object 应存在");
            let step_type = program
                .step_type(callable.step_schema())
                .expect("step type 应存在");
            let ping_interface = program
                .resume_packings()
                .iter()
                .find(|interface| interface.effect_family().effect_fqn() == "fixtures.build.Ping")
                .expect("应存在 Ping resume packing");
            let raise_interface_id = next_resume_interface_id(&program);
            let raise_method = resume_method_for_case(step_type, CaseTag::new(2));
            let raise_interface = LateLoweredResumeInterface::new(
                raise_interface_id,
                raise_method.concrete_op_key().effect_family().clone(),
                callable.step_schema(),
                vec![raise_method],
            );
            let reversed_interfaces = vec![raise_interface_id, ping_interface.interface_id()];
            let resume_interfaces = program
                .resume_packings()
                .iter()
                .cloned()
                .chain(std::iter::once(raise_interface))
                .collect();

            let callables = program
                .callables()
                .iter()
                .map(|candidate| {
                    if candidate.body_step_schema() == Some(callable.step_schema()) {
                        clone_callable_with_interfaces(candidate, reversed_interfaces.clone())
                    } else {
                        candidate.clone()
                    }
                })
                .collect();
            let continuation_objects = program
                .continuation_objects()
                .iter()
                .map(|candidate| {
                    if candidate.object_id() == continuation_object.object_id() {
                        clone_continuation_object_with_interfaces(
                            candidate,
                            reversed_interfaces.clone(),
                        )
                    } else {
                        candidate.clone()
                    }
                })
                .collect();

            LateLoweredProgram::new(
                program.step_types().to_vec(),
                resume_interfaces,
                continuation_objects,
                callables,
            )
            .with_stable_instance_keys(program.stable_instance_keys().clone())
        },
        |inputs, result, _module| {
            let query = result.expect("reordered published resume packings 应仍可物化 ABI");
            let callable = inputs
                .abi_visibility_program
                .callable("fixtures.build.singleCaseWorker")
                .expect("singleCaseWorker callable 应存在");
            let callable_layout = query
                .callable_layout_by_version_key(callable.body_version_key())
                .expect("callable layout 应可查询");
            let ping_interface_id = callable_layout
                .resume_packings()
                .iter()
                .find(|interface_id| {
                    query
                        .resume_packing_layout(**interface_id)
                        .is_some_and(|interface| {
                            interface.packing_family_fqn() == "fixtures.build.Ping"
                        })
                })
                .copied()
                .expect("应存在 Ping resume packing");
            let raise_interface_id = callable_layout
                .resume_packings()
                .iter()
                .find(|interface_id| {
                    query
                        .resume_packing_layout(**interface_id)
                        .is_some_and(|interface| {
                            interface.packing_family_fqn() == "scoop.core.Raise"
                        })
                })
                .copied()
                .expect("应存在 Raise resume packing");
            let expected_order = vec![raise_interface_id, ping_interface_id];

            assert_eq!(callable_layout.resume_packings(), expected_order.as_slice());

            let continuation_layout = query
                .continuation_layout(callable_layout.continuation_object())
                .expect("continuation layout 应可查询");
            let first_index = continuation_layout
                .field_index_for_packing(expected_order[0])
                .expect("首个 published packing 应有 field");
            let second_index = continuation_layout
                .field_index_for_packing(expected_order[1])
                .expect("次个 published packing 应有 field");
            assert!(
                first_index < second_index,
                "continuation field 顺序必须跟随 published packing 顺序"
            );
        },
    );
}

#[test]
fn refactor_llvm_continuation_layout_preserves_authoritative_method_order() {
    with_fixture_query_result(
        "effect_refactor_step_enum_single_case.scoop",
        |inputs| {
            single_case_worker_program_with_ping_method_order(
                inputs,
                &[CaseTag::new(1), CaseTag::new(0)],
            )
        },
        |inputs, result, _module| {
            let query = result.expect("reordered authoritative methods 应仍可物化 ABI");
            let callable = inputs
                .effect_lowered_stage_output
                .program()
                .callable("fixtures.build.singleCaseWorker")
                .expect("callable 应存在");
            let interface_id = query
                .callable_layout(callable.step_schema())
                .expect("callable layout 应可查询")
                .resume_packings()
                .iter()
                .find(|interface_id| {
                    query
                        .resume_packing_layout(**interface_id)
                        .is_some_and(|interface| {
                            interface.packing_family_fqn() == "fixtures.build.Ping"
                        })
                })
                .copied()
                .expect("应存在 Ping resume packing");
            let interface_layout = query
                .resume_packing_layout(interface_id)
                .expect("resume packing layout 应可查询");

            assert_eq!(
                interface_layout
                    .method(CaseTag::new(1))
                    .expect("case1 method 应存在")
                    .vtable_index(),
                0
            );
            assert_eq!(
                interface_layout
                    .method(CaseTag::new(0))
                    .expect("case0 method 应存在")
                    .vtable_index(),
                1
            );
        },
    );
}

#[test]
fn refactor_llvm_continuation_layout_rejects_missing_published_packing() {
    with_fixture_query_result(
        "effect_refactor_step_enum_single_case.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let callable = program
                .callable("fixtures.build.singleCaseWorker")
                .expect("callable 应存在");
            let dropped_interface = callable
                .resume_packings()
                .first()
                .copied()
                .expect("fixture 应至少发布一个 packing");
            let resume_interfaces = program
                .resume_packings()
                .iter()
                .filter(|interface| interface.interface_id() != dropped_interface)
                .cloned()
                .collect();

            LateLoweredProgram::new(
                program.step_types().to_vec(),
                resume_interfaces,
                program.continuation_objects().to_vec(),
                program.callables().to_vec(),
            )
            .with_stable_instance_keys(program.stable_instance_keys().clone())
        },
        |inputs, result, _module| {
            let callable = inputs
                .effect_lowered_stage_output
                .program()
                .callable("fixtures.build.singleCaseWorker")
                .expect("callable 应存在");
            let dropped_interface = callable
                .resume_packings()
                .first()
                .copied()
                .expect("fixture 应至少发布一个 packing");
            let err = match result {
                Ok(_) => panic!("缺失 published packing 时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains(&format!("resume packing {}", dropped_interface.as_u32())),
                "错误消息应指出缺失的 published packing: {message}"
            );
        },
    );
}

#[test]
fn refactor_llvm_continuation_layout_rejects_missing_authoritative_method() {
    with_fixture_query_result(
        "effect_refactor_step_enum_single_case.scoop",
        |inputs| single_case_worker_program_with_ping_method_order(inputs, &[CaseTag::new(0)]),
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => panic!("缺失 authoritative method 时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("authoritative method cases [1]"),
                "错误消息应指出缺失的 authoritative case tag: {message}"
            );
            assert!(
                message.contains("effect family `fixtures.build.Ping`"),
                "错误消息应指出缺失方法所属的 interface family: {message}"
            );
            assert!(
                message.contains("step schema"),
                "错误消息应指出缺失方法对应的 step schema: {message}"
            );
        },
    );
}

#[test]
fn refactor_llvm_call_target_query_preserves_known_instance_direct_entries() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_handle_hidden_suspend_virtual_helper_basic.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query = result.expect("known-instance direct call 应可回查 callable entry");
            let program = inputs.effect_lowered_stage_output.program();
            let main = program.callable("main").expect("main callable 应存在");
            let helper = program.callable("helper").expect("helper callable 应存在");
            let main_plain = main.plain_abi().expect("main 应保持 plain callable ABI");
            let call_facts = main_plain
                .call_sites()
                .iter()
                .map(|site| site.facts())
                .find(|facts| matches!(facts.target(), CallSiteTarget::KnownInstance(target) if target.template.fqn == "helper"))
                .expect("main plain source slice 应发布 helper known-instance call facts");

            assert_eq!(call_facts.target_mode(), CallTargetMode::KnownInstance);
            if helper.effect_step_abi().is_some() {
                let target = query
                    .callable_layout_by_version_key(helper.body_version_key())
                    .expect("effect-step helper 应可按 body version key 回查 callable entry");
                assert_eq!(target.root_fqn(), "helper");
                assert_eq!(target.body_version_key(), helper.body_version_key());
            } else {
                let target = query
                    .plain_callable_layout_by_version_key(helper.body_version_key())
                    .expect("NoOutward helper 应可按 body version key 回查 plain entry");
                assert_eq!(target.root_fqn(), "helper");
                assert_eq!(target.body_version_key(), helper.body_version_key());
            }
        },
    );
}

#[test]
fn refactor_llvm_callable_version_query_resolves_layout_by_body_version_key() {
    with_fixture_query(
        "effect_refactor_dynamic_entry_publication_emit_llvm.scoop",
        |inputs, query, _module| {
            for callable in inputs.abi_visibility_program.callables() {
                if callable.effect_step_abi().is_some() {
                    let layout = query
                        .callable_layout_by_version_key(callable.body_version_key())
                        .expect("effect-step callable version 应可按 body version key 回查");
                    assert_eq!(layout.root_fqn(), callable.root_fqn());
                    assert_eq!(layout.step_schema(), callable.step_schema());
                    assert_eq!(layout.continuation_object(), callable.continuation_object());
                } else {
                    let layout = query
                        .plain_callable_layout_by_version_key(callable.body_version_key())
                        .expect("plain callable version 应可按 body version key 回查");
                    assert_eq!(layout.root_fqn(), callable.root_fqn());
                }
            }
        },
    );
}

#[test]
fn refactor_llvm_known_instance_version_selection_resolves_generic_instance_keys() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, module| {
            let query = result.expect("generic known-instance callable 应可回查 callable version");
            let println_int = inputs
                .abi_visibility_program
                .callables()
                .iter()
                .find(|callable| callable.root_fqn() == "scoop.core.println::<Int>")
                .expect("fixture 应发布 println::<Int> callable shell");
            let target = query
                .plain_callable_layout_by_version_key(println_int.body_version_key())
                .expect("NoOutward generic callable 应发布 plain version layout");

            assert_eq!(target.root_fqn(), println_int.root_fqn());
            assert_eq!(target.body_version_key(), println_int.body_version_key());
            assert_eq!(target.surface_instance(), println_int.instance_key());
            assert!(
                query
                    .callable_layout_by_version_key(println_int.body_version_key())
                    .is_err(),
                "NoOutward generic callable 不应发布 effect-step callable layout"
            );
            let direct_symbol = target.direct_entry().symbol_name();
            assert!(
                direct_symbol.starts_with("__scoop_abi0_fun__scoop_core_println__h"),
                "materialized generic plain callable 应切到 AbiMangler namespace: {direct_symbol}"
            );
            assert!(module.get_function(direct_symbol).is_some());
        },
    );
}

#[test]
fn refactor_llvm_boundary_operand_contract_resolves_direct_call_anchor_and_args() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_resume_if_else_branch_single_perform.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query = result.expect("direct call boundary operand contract 应成功发布");
            let main = inputs
                .abi_visibility_program
                .callable("main")
                .expect("main callable 应存在");
            let boundary = site_boundary(main, BoundarySiteKind::Call);
            let lowering = call_boundary_lowering(boundary);
            let site_id = boundary_site_id(boundary);
            let layout = query
                .call_boundary_operand_layout(
                    main.step_schema(),
                    site_id,
                    lowering.operand_contract(),
                )
                .expect("direct call boundary 应可回查 published operand contract");
            let RefactorCallTargetQuery::KnownInstance(_) = query
                .call_target_layout(main.step_schema(), site_id, lowering.facts())
                .expect("direct call target contract 应成功")
            else {
                panic!("known-instance direct call 不应走 dynamic invoke contract");
            };

            assert_eq!(layout.owner_step_schema(), main.step_schema());
            assert_eq!(layout.site_id(), site_id);
            assert!(matches!(
                layout.contract().source_consumption(),
                LateLoweredBoundarySourceConsumption::Statement {
                    consumes_last_statement: true,
                    ..
                }
            ));
            assert!(layout.contract().carrier_source().is_none());
            assert_eq!(layout.contract().arg_sources().len(), 1);
            assert_eq!(
                inputs
                    .effect_lowered_stage_output
                    .types()
                    .display(layout.contract().arg_sources()[0].source_ty())
                    .to_string(),
                "Bool"
            );
            assert!(matches!(
                layout.contract().arg_sources()[0].value(),
                LateLoweredOperandValueSource::Local(_)
                    | LateLoweredOperandValueSource::Const(crate::mir::ConstValue::Bool(_))
            ));
            assert!(layout.contract().arg_sources()[0].span().is_some());
        },
    );
}

#[test]
fn refactor_llvm_boundary_operand_contract_resolves_dynamic_call_carrier() {
    with_phase_fixture_query_result(
        "effect_facts",
        "dynamic_fallback_widening.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query = result.expect("dynamic call boundary operand contract 应成功发布");
            let call_value = inputs
                .abi_visibility_program
                .callable("sample.callValue")
                .expect("sample.callValue callable 应存在");
            let boundary = site_boundary(call_value, BoundarySiteKind::Call);
            let lowering = call_boundary_lowering(boundary);
            let site_id = boundary_site_id(boundary);
            let layout = query
                .call_boundary_operand_layout(
                    call_value.step_schema(),
                    site_id,
                    lowering.operand_contract(),
                )
                .expect("dynamic call boundary 应可回查 published operand contract");
            let RefactorCallTargetQuery::DynamicInvoke(_) = query
                .call_target_layout(call_value.step_schema(), site_id, lowering.facts())
                .expect("dynamic call target contract 应成功")
            else {
                panic!("non-KnownInstance call 应走 dynamic invoke contract");
            };

            assert!(matches!(
                layout.contract().source_consumption(),
                LateLoweredBoundarySourceConsumption::Statement { .. }
            ));
            assert_eq!(layout.contract().arg_sources().len(), 0);
            assert!(matches!(
                layout
                    .contract()
                    .carrier_source()
                    .expect("dynamic call 应发布 carrier source")
                    .value(),
                LateLoweredOperandValueSource::Local(_)
            ));
        },
    );
}

#[test]
fn refactor_llvm_boundary_operand_contract_resolves_perform_and_resume_sources() {
    with_phase_fixture_query_result(
        "effect_facts",
        "handle_perform.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query = result.expect("perform boundary operand contract 应成功发布");
            let main = inputs
                .abi_visibility_program
                .callable("a.main")
                .expect("a.main callable 应存在");
            let boundary = site_boundary(main, BoundarySiteKind::Perform);
            let lowering = perform_boundary_lowering(boundary);
            let site_id = boundary_site_id(boundary);
            let layout = query
                .perform_boundary_operand_layout(
                    main.step_schema(),
                    site_id,
                    lowering.operand_contract(),
                )
                .expect("perform boundary 应可回查 published operand contract");

            assert!(matches!(
                layout.contract().source_consumption(),
                LateLoweredBoundarySourceConsumption::Terminator { .. }
            ));
            assert_eq!(layout.contract().payload_sources().len(), 1);
            assert!(matches!(
                layout.contract().payload_sources()[0].value(),
                LateLoweredOperandValueSource::Local(_)
                    | LateLoweredOperandValueSource::Const(crate::mir::ConstValue::Int)
            ));
            assert!(layout.contract().payload_sources()[0].span().is_some());
        },
    );

    with_phase_fixture_query_result(
        "effect_facts",
        "dispatch_and_resume_call.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query = result.expect("resume boundary operand contract 应成功发布");
            let callable = inputs
                .abi_visibility_program
                .callable("fixtures.mir.resumeBoom")
                .expect("fixtures.mir.resumeBoom callable 应存在");
            let boundary = site_boundary(callable, BoundarySiteKind::Resume);
            let lowering = resume_boundary_lowering(boundary);
            let site_id = boundary_site_id(boundary);
            let layout = query
                .resume_boundary_operand_layout(
                    callable.step_schema(),
                    site_id,
                    lowering.operand_contract(),
                )
                .expect("resume boundary 应可回查 published operand contract");

            assert!(matches!(
                layout.contract().source_consumption(),
                LateLoweredBoundarySourceConsumption::Statement {
                    consumes_last_statement: true,
                    ..
                }
            ));
            assert!(matches!(
                layout.contract().continuation_source().value(),
                LateLoweredOperandValueSource::Local(_)
            ));
            assert_eq!(layout.contract().arg_sources().len(), 1);
            assert!(matches!(
                layout.contract().arg_sources()[0].value(),
                LateLoweredOperandValueSource::Local(_)
                    | LateLoweredOperandValueSource::Const(crate::mir::ConstValue::Int)
            ));
            assert!(layout.contract().arg_sources()[0].span().is_some());
            let route = layout.contract().underlying_continuation_route();
            assert_eq!(
                route.continuation_schema(),
                lowering.facts().continuation_schema()
            );
            assert!(matches!(
                route.publication(),
                LateLoweredSurfaceResumeDispatchPublication::ResumeBoundary {
                    owner_version_key,
                    owner_continuation_object,
                    site_id: route_site_id,
                } if owner_version_key == callable.body_version_key()
                    && *owner_continuation_object == callable.continuation_object()
                    && *route_site_id == site_id
            ));
        },
    );

    with_phase_fixture_query_result(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query =
                result.expect("readback resume boundary provenance 应成功发布到 LLVM query");
            let callable = inputs
                .abi_visibility_program
                .callable("main")
                .expect("main callable 应存在");
            let handle_state = handle_dispatch_state(callable, SiteId::from_raw(1));
            let LateLoweredStateTerminator::HandleDispatch { contract, .. } =
                handle_state.terminator()
            else {
                panic!("main 顶层 handle 应保持 HandleDispatch terminator");
            };
            let binder = contract.handled_arms()[0]
                .continuation_binder()
                .expect("Ask handle arm 应发布 continuation binder");

            let routes = callable
                .boundary_map()
                .entries()
                .iter()
                .filter_map(|boundary| match boundary.lowering() {
                    Some(LateLoweredBoundaryLowering::Resume(lowering)) => {
                        Some((boundary_site_id(boundary), lowering))
                    }
                    _ => None,
                })
                .map(|(site_id, lowering)| {
                    let layout = query
                        .resume_boundary_operand_layout(
                            callable.step_schema(),
                            site_id,
                            lowering.operand_contract(),
                        )
                        .unwrap_or_else(|err| {
                            panic!(
                                "resume site{} 应可回查 boundary operand contract: {err}",
                                site_id.as_u32()
                            )
                        });
                    let route = layout.contract().underlying_continuation_route();
                    (site_id, route)
                })
                .collect::<Vec<_>>();

            assert_eq!(
                routes
                    .iter()
                    .map(|(site_id, _)| site_id.as_u32())
                    .collect::<Vec<_>>(),
                vec![26, 31, 36, 41]
            );
            for (_site_id, route) in routes {
                assert_eq!(route.continuation_schema(), binder.continuation_schema());
                assert!(matches!(
                    route.publication(),
                    LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
                        owner_continuation_object,
                        site_id,
                        arm_ordinal,
                        handled_case,
                        ..
                    } if *owner_continuation_object == callable.continuation_object()
                        && site_id.as_u32() == 1
                        && *arm_ordinal == 0
                        && *handled_case == contract.handled_arms()[0].handled_case()
                ));
            }
        },
    );
}

#[test]
fn refactor_llvm_resume_payload_binding_resolves_boundary_and_state_queries() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_resume_if_else_branch_single_perform.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query =
                result.expect("call/resume boundary 的 resumed local/home contract 应成功发布");

            let main = inputs
                .abi_visibility_program
                .callable("main")
                .expect("main callable 应存在");
            let call_boundary = site_boundary(main, BoundarySiteKind::Call);
            let call_binding = main
                .frame_schema()
                .resume_payload_binding(call_boundary.boundary_id())
                .expect("call boundary 应发布 resumed local/home binding");
            let call_layout = query
                .resume_payload_binding_layout(main.step_schema(), call_binding)
                .expect("call boundary 应可回查 resumed local/home contract");
            let call_frame_layout = query
                .frame_layout(main.step_schema())
                .expect("callable frame layout 应可查询");
            let call_home_slot = call_binding
                .consumer_frame_slot()
                .expect("call boundary 应发布 frame home slot");

            assert_eq!(call_layout.boundary_id(), call_boundary.boundary_id());
            assert_eq!(call_layout.resume_state(), call_boundary.resume_state());
            assert_eq!(call_layout.consumer_local(), call_binding.consumer_local());
            assert_eq!(
                call_layout.frame_field_index(),
                call_frame_layout.field_index_for_slot(call_home_slot),
            );

            let run = inputs
                .abi_visibility_program
                .callable("run")
                .expect("run callable 应存在");
            let resume_boundary = site_boundary(run, BoundarySiteKind::Resume);
            let resume_binding = run
                .frame_schema()
                .resume_payload_binding(resume_boundary.boundary_id())
                .expect("resume boundary 应发布 resumed local/home binding");
            let resume_layout = query
                .resume_payload_binding_layout(run.step_schema(), resume_binding)
                .expect("resume boundary 应可回查 resumed local/home contract");
            let state_layout = query
                .resume_payload_binding_for_state(run.step_schema(), resume_boundary.resume_state())
                .expect("resume state 应可直接回查 resumed local/home contract");

            assert_eq!(
                resume_layout.consumer_local(),
                resume_binding.consumer_local()
            );
            assert_eq!(
                state_layout.consumer_frame_slot(),
                resume_binding.consumer_frame_slot(),
            );
        },
    );

    with_phase_fixture_query_result(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query =
                result.expect("perform/runtime-error 的 resumed local/home contract 应成功发布");

            let fetch = inputs
                .abi_visibility_program
                .callable("fetch")
                .expect("fetch callable 应存在");
            let perform_boundary = site_boundary(fetch, BoundarySiteKind::Perform);
            let perform_binding = fetch
                .frame_schema()
                .resume_payload_binding(perform_boundary.boundary_id())
                .expect("perform boundary 应发布 resumed local/home binding");
            let perform_layout = query
                .resume_payload_binding_layout(fetch.step_schema(), perform_binding)
                .expect("perform boundary 应可回查 resumed local/home contract");
            let fetch_frame_layout = query
                .frame_layout(fetch.step_schema())
                .expect("fetch frame layout 应可查询");
            let perform_home_slot = perform_binding
                .consumer_frame_slot()
                .expect("perform boundary 应发布 frame home slot");

            assert_eq!(perform_layout.boundary_id(), perform_boundary.boundary_id());
            assert_eq!(
                perform_layout.resume_state(),
                perform_boundary.resume_state()
            );
            assert_eq!(
                perform_layout.frame_field_index(),
                fetch_frame_layout.field_index_for_slot(perform_home_slot),
            );

            let main = inputs
                .abi_visibility_program
                .callable("main")
                .expect("main callable 应存在");
            let runtime_error_boundary = main
                .boundary_map()
                .entries()
                .iter()
                .find(|boundary| {
                    matches!(
                        boundary.source(),
                        LateLoweredBoundarySource::RuntimeError { .. }
                    )
                })
                .expect("main 应存在 runtime-error boundary");
            let runtime_error_binding = main
                .frame_schema()
                .resume_payload_binding(runtime_error_boundary.boundary_id())
                .expect("runtime-error boundary 应发布 resumed local/home binding");
            let runtime_error_layout = query
                .resume_payload_binding_layout(main.step_schema(), runtime_error_binding)
                .expect("runtime-error boundary 应可回查 resumed local/home contract");
            let state_layout = query
                .resume_payload_binding_for_state(
                    main.step_schema(),
                    runtime_error_boundary.resume_state(),
                )
                .expect("runtime-error resume state 应可直接回查 resumed local/home contract");

            assert_eq!(
                runtime_error_layout.consumer_local(),
                runtime_error_binding.consumer_local(),
            );
            assert_eq!(
                state_layout.consumer_frame_slot(),
                runtime_error_binding.consumer_frame_slot(),
            );
        },
    );
}

#[test]
fn refactor_llvm_resume_payload_binding_rejects_missing_contract() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let fetch = program.callable("fetch").expect("fetch callable 应存在");
            let frame_schema = LateLoweredFrameSchema::new(fetch.frame_schema().slots().to_vec())
                .with_completion_payload_bindings(
                    fetch.frame_schema().completion_payload_bindings().to_vec(),
                );
            let callables = program
                .callables()
                .iter()
                .map(|candidate| {
                    if candidate.body_step_schema() == Some(fetch.step_schema()) {
                        clone_callable_with_frame_schema(candidate, frame_schema.clone())
                    } else {
                        candidate.clone()
                    }
                })
                .collect();
            LateLoweredProgram::new(
                program.step_types().to_vec(),
                program.resume_packings().to_vec(),
                program.continuation_objects().to_vec(),
                callables,
            )
            .with_stable_instance_keys(program.stable_instance_keys().clone())
            .with_stable_instance_keys(program.stable_instance_keys().clone())
        },
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => panic!("缺失 resumed local/home contract 时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("resumed local/home contract"),
                "错误消息应指出缺失的是 resumed local/home contract: {message}"
            );
        },
    );
}

#[test]
fn refactor_llvm_resume_payload_binding_rejects_runtime_error_binding_drift() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let main = program.callable("main").expect("main callable 应存在");
            let runtime_error_boundary = main
                .boundary_map()
                .entries()
                .iter()
                .find(|boundary| {
                    matches!(
                        boundary.source(),
                        LateLoweredBoundarySource::RuntimeError { .. }
                    )
                })
                .expect("main 应存在 runtime-error boundary");
            let replacement = main
                .frame_schema()
                .resume_payload_bindings()
                .iter()
                .find(|binding| {
                    binding.boundary_id() != runtime_error_boundary.boundary_id()
                        && binding.resume_state() != runtime_error_boundary.resume_state()
                })
                .copied()
                .expect("应存在可用于构造 drift 的其它 resumed local/home binding");
            let bindings = main
                .frame_schema()
                .resume_payload_bindings()
                .iter()
                .copied()
                .map(|binding| {
                    if binding.boundary_id() == runtime_error_boundary.boundary_id() {
                        LateLoweredResumePayloadBinding::new(
                            binding.boundary_id(),
                            binding.resume_state(),
                            replacement.consumer_local(),
                            replacement.consumer_frame_slot(),
                        )
                    } else {
                        binding
                    }
                })
                .collect();
            let frame_schema = LateLoweredFrameSchema::new(main.frame_schema().slots().to_vec())
                .with_resume_payload_bindings(bindings)
                .with_completion_payload_bindings(
                    main.frame_schema().completion_payload_bindings().to_vec(),
                );
            let callables = program
                .callables()
                .iter()
                .map(|candidate| {
                    if candidate.body_step_schema() == Some(main.step_schema()) {
                        clone_callable_with_frame_schema(candidate, frame_schema.clone())
                    } else {
                        candidate.clone()
                    }
                })
                .collect();
            LateLoweredProgram::new(
                program.step_types().to_vec(),
                program.resume_packings().to_vec(),
                program.continuation_objects().to_vec(),
                callables,
            )
            .with_stable_instance_keys(program.stable_instance_keys().clone())
        },
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => panic!("runtime-error binding 漂移时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("resumed local/home contract")
                    && (message.contains("runtime-error boundary")
                        || message.contains("漂移")
                        || message.contains("冲突")),
                "错误消息应指出 runtime-error resumed local/home contract 漂移: {message}"
            );
        },
    );
}

#[test]
fn refactor_llvm_completion_payload_contract_resolves_return_state_query() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_resume_if_else_branch_single_perform.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query = result.expect("completion payload contract 应成功发布到 LLVM query");
            let run = inputs
                .abi_visibility_program
                .callable("run")
                .expect("run callable 应存在");
            let binding = run
                .frame_schema()
                .completion_payload_bindings()
                .iter()
                .find(|binding| !binding.payload_source().is_unit())
                .expect("run(): Int 应发布 non-Unit completion payload binding");
            let layout = query
                .completion_payload_binding_layout(run.step_schema(), binding)
                .expect("return state 应可回查 completion payload contract");
            let state_layout = query
                .completion_payload_binding_for_state(run.step_schema(), binding.return_state())
                .expect("return state 应可直接回查 completion payload contract");
            let frame_layout = query
                .frame_layout(run.step_schema())
                .expect("run frame layout 应可查询");

            assert_eq!(layout.owner_step_schema(), run.step_schema());
            assert_eq!(layout.return_state(), binding.return_state());
            assert_eq!(layout.complete_state(), run.state_graph().complete_state());
            assert_eq!(state_layout.binding(), binding);
            assert_eq!(layout.payload_source(), binding.payload_source());
            assert_eq!(
                inputs
                    .effect_lowered_stage_output
                    .types()
                    .display(layout.payload_source().source_ty())
                    .to_string(),
                "Int"
            );
            assert!(
                !layout.payload_abi().is_elided(),
                "Int completion payload 不应在 ABI 中被 elide"
            );
            if let Some(slot_id) = binding.payload_frame_slot() {
                assert_eq!(
                    layout.frame_field_index(),
                    frame_layout.field_index_for_slot(slot_id),
                );
            }
        },
    );
}

#[test]
fn refactor_llvm_completion_payload_contract_rejects_missing_contract() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_resume_if_else_branch_single_perform.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let run = program.callable("run").expect("run callable 应存在");
            let frame_schema = LateLoweredFrameSchema::new(run.frame_schema().slots().to_vec())
                .with_resume_payload_bindings(
                    run.frame_schema().resume_payload_bindings().to_vec(),
                );
            let callables = program
                .callables()
                .iter()
                .map(|candidate| {
                    if candidate.body_step_schema() == Some(run.step_schema()) {
                        clone_callable_with_frame_schema(candidate, frame_schema.clone())
                    } else {
                        candidate.clone()
                    }
                })
                .collect();
            LateLoweredProgram::new(
                program.step_types().to_vec(),
                program.resume_packings().to_vec(),
                program.continuation_objects().to_vec(),
                callables,
            )
            .with_stable_instance_keys(program.stable_instance_keys().clone())
        },
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => panic!("缺失 completion payload contract 时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("completion payload contract"),
                "错误消息应指出缺失的是 completion payload contract: {message}"
            );
        },
    );
}

#[test]
fn refactor_llvm_completion_payload_contract_rejects_source_drift() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_resume_if_else_branch_single_perform.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let run = program.callable("run").expect("run callable 应存在");
            let drifted_bindings = run
                .frame_schema()
                .completion_payload_bindings()
                .iter()
                .map(|binding| {
                    if binding.payload_source().is_unit() {
                        binding.clone()
                    } else {
                        LateLoweredCompletionPayloadBinding::new(
                            binding.return_state(),
                            binding.complete_state(),
                            LateLoweredCompletionPayloadSource::unit(
                                binding.payload_source().source_ty(),
                            ),
                            binding.payload_frame_slot(),
                        )
                    }
                })
                .collect();
            let frame_schema = LateLoweredFrameSchema::new(run.frame_schema().slots().to_vec())
                .with_resume_payload_bindings(run.frame_schema().resume_payload_bindings().to_vec())
                .with_completion_payload_bindings(drifted_bindings);
            let callables = program
                .callables()
                .iter()
                .map(|candidate| {
                    if candidate.body_step_schema() == Some(run.step_schema()) {
                        clone_callable_with_frame_schema(candidate, frame_schema.clone())
                    } else {
                        candidate.clone()
                    }
                })
                .collect();
            LateLoweredProgram::new(
                program.step_types().to_vec(),
                program.resume_packings().to_vec(),
                program.continuation_objects().to_vec(),
                callables,
            )
            .with_stable_instance_keys(program.stable_instance_keys().clone())
        },
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => panic!("completion payload source 漂移时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("completion payload source")
                    || message.contains("completion payload frame home"),
                "错误消息应指出 completion payload contract 漂移: {message}"
            );
        },
    );
}

#[test]
fn refactor_llvm_boundary_operand_contract_rejects_ordered_arg_drift() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_resume_if_else_branch_single_perform.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let main = program.callable("main").expect("main callable 应存在");
            let boundary_map = LateLoweredBoundaryMap::new(
                main.boundary_map()
                    .entries()
                    .iter()
                    .map(|boundary| {
                        let lowering = match boundary
                            .lowering()
                            .cloned()
                            .expect("boundary 应带 lowering")
                        {
                            LateLoweredBoundaryLowering::Call(lowering) => {
                                LateLoweredBoundaryLowering::Call(
                                    LateLoweredCallBoundaryLowering::new(
                                        lowering.facts().clone(),
                                        lowering.result_local(),
                                        LateLoweredCallBoundaryOperandContract::new(
                                            lowering.operand_contract().source_consumption(),
                                            None,
                                            Vec::new(),
                                        ),
                                        lowering.dispatch().clone(),
                                        lowering.continuation_compositions().to_vec(),
                                        lowering.consumed_runtime_error_case().cloned(),
                                    ),
                                )
                            }
                            other => other,
                        };
                        LateLoweredBoundary::new(
                            boundary.boundary_id(),
                            boundary.source(),
                            boundary.owner_state(),
                            boundary.resume_state(),
                        )
                        .with_lowering(lowering)
                    })
                    .collect(),
            );
            let callables = program
                .callables()
                .iter()
                .map(|candidate| {
                    if candidate.body_step_schema() == Some(main.step_schema()) {
                        clone_callable_with_boundary_map(candidate, boundary_map.clone())
                    } else {
                        candidate.clone()
                    }
                })
                .collect();
            LateLoweredProgram::new(
                program.step_types().to_vec(),
                program.resume_packings().to_vec(),
                program.continuation_objects().to_vec(),
                callables,
            )
            .with_stable_instance_keys(program.stable_instance_keys().clone())
        },
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => panic!("ordered arg drift 时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("ordered args")
                    && (message.contains("contract 非法")
                        || message.contains("单一 source")
                        || message.contains("component")),
                "错误消息应指出 ordered args contract 漂移: {message}"
            );
        },
    );
}

#[test]
fn refactor_llvm_boundary_operand_contract_rejects_missing_dynamic_carrier_source() {
    with_phase_fixture_query_result(
        "effect_facts",
        "dynamic_fallback_widening.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let call_value = program
                .callable("sample.callValue")
                .expect("sample.callValue callable 应存在");
            let boundary_map = LateLoweredBoundaryMap::new(
                call_value
                    .boundary_map()
                    .entries()
                    .iter()
                    .map(|boundary| {
                        let lowering = match boundary
                            .lowering()
                            .cloned()
                            .expect("boundary 应带 lowering")
                        {
                            LateLoweredBoundaryLowering::Call(lowering) => {
                                LateLoweredBoundaryLowering::Call(
                                    LateLoweredCallBoundaryLowering::new(
                                        lowering.facts().clone(),
                                        lowering.result_local(),
                                        LateLoweredCallBoundaryOperandContract::new(
                                            lowering.operand_contract().source_consumption(),
                                            None,
                                            lowering.operand_contract().arg_sources().to_vec(),
                                        ),
                                        lowering.dispatch().clone(),
                                        lowering.continuation_compositions().to_vec(),
                                        lowering.consumed_runtime_error_case().cloned(),
                                    ),
                                )
                            }
                            other => other,
                        };
                        LateLoweredBoundary::new(
                            boundary.boundary_id(),
                            boundary.source(),
                            boundary.owner_state(),
                            boundary.resume_state(),
                        )
                        .with_lowering(lowering)
                    })
                    .collect(),
            );
            let callables = program
                .callables()
                .iter()
                .map(|candidate| {
                    if candidate.body_step_schema() == Some(call_value.step_schema()) {
                        clone_callable_with_boundary_map(candidate, boundary_map.clone())
                    } else {
                        candidate.clone()
                    }
                })
                .collect();
            LateLoweredProgram::new(
                program.step_types().to_vec(),
                program.resume_packings().to_vec(),
                program.continuation_objects().to_vec(),
                callables,
            )
            .with_stable_instance_keys(program.stable_instance_keys().clone())
        },
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => panic!("缺失 dynamic carrier source 时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("carrier source contract"),
                "错误消息应指出缺失的是 dynamic carrier source contract: {message}"
            );
        },
    );
}

#[test]
fn refactor_llvm_boundary_operand_contract_rejects_missing_underlying_continuation_route_publication()
 {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let main = program.callable("main").expect("main callable 应存在");
            let boundary_map = LateLoweredBoundaryMap::new(
                main.boundary_map()
                    .entries()
                    .iter()
                    .map(|boundary| {
                        let lowering = match boundary
                            .lowering()
                            .cloned()
                            .expect("boundary 应带 lowering")
                        {
                            LateLoweredBoundaryLowering::Resume(lowering) => {
                                let route = lowering
                                    .operand_contract()
                                    .underlying_continuation_route();
                                let broken_contract =
                                    crate::effect_lowered::ir::LateLoweredResumeBoundaryOperandContract::new(
                                        lowering.operand_contract().source_consumption(),
                                        lowering.operand_contract().continuation_source().clone(),
                                        lowering.operand_contract().arg_sources().to_vec(),
                                        crate::effect_lowered::ir::LateLoweredContinuationRoute::new(
                                            route.continuation_schema(),
                                            LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
                                                owner_version_key: main.body_version_key().clone(),
                                                owner_continuation_object: main.continuation_object(),
                                                site_id: SiteId::from_raw(999),
                                                arm_ordinal: 0,
                                                handled_case: CaseTag::new(1),
                                            },
                                        ),
                                        lowering
                                            .operand_contract()
                                            .underlying_route_is_compatible_set(),
                                    );
                                LateLoweredBoundaryLowering::Resume(
                                    crate::effect_lowered::ir::LateLoweredResumeBoundaryLowering::new(
                                        lowering.facts().clone(),
                                        lowering.result_local(),
                                        lowering.runtime_error_boundary(),
                                        broken_contract,
                                        lowering.dispatch().clone(),
                                        lowering.continuation_compositions().to_vec(),
                                    ),
                                )
                            }
                            other => other,
                        };
                        LateLoweredBoundary::new(
                            boundary.boundary_id(),
                            boundary.source(),
                            boundary.owner_state(),
                            boundary.resume_state(),
                        )
                        .with_lowering(lowering)
                    })
                    .collect(),
            );
            let callables = program
                .callables()
                .iter()
                .map(|candidate| {
                    if candidate.body_step_schema() == Some(main.step_schema()) {
                        clone_callable_with_boundary_map(candidate, boundary_map.clone())
                    } else {
                        candidate.clone()
                    }
                })
                .collect();
            LateLoweredProgram::new(
                program.step_types().to_vec(),
                program.resume_packings().to_vec(),
                program.continuation_objects().to_vec(),
                callables,
            )
            .with_stable_instance_keys(program.stable_instance_keys().clone())
        },
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => {
                    panic!("缺失 underlying continuation route publication 时必须 fail fast")
                }
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("underlying continuation route")
                    || message.contains("wrapper complete projection")
                    || message.contains("handle binder"),
                "错误消息应指出 underlying continuation route publication 漂移: {message}"
            );
        },
    );
}

#[test]
fn refactor_llvm_dynamic_invoke_query_resolves_fun_value_unit_contract() {
    with_phase_fixture_query_result(
        "effect_facts",
        "dynamic_fallback_widening.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query = result.expect("fun-value DynamicFallback 应可物化 dynamic invoke contract");
            let callable = inputs
                .effect_lowered_stage_output
                .program()
                .callable("sample.callValue")
                .expect("sample.callValue callable 应存在");
            let boundary = site_boundary(callable, BoundarySiteKind::Call);
            let lowering = call_boundary_lowering(boundary);

            assert_eq!(
                lowering.facts().target_mode(),
                CallTargetMode::DynamicFallback
            );
            let site_id = boundary_site_id(boundary);
            let RefactorCallTargetQuery::DynamicInvoke(layout) = query
                .call_target_layout(callable.step_schema(), site_id, lowering.facts())
                .expect("fun-value boundary 应可回查 dynamic invoke contract")
            else {
                panic!("DynamicFallback fun-value call 应走 dynamic invoke contract");
            };
            assert_eq!(layout.owner_step_schema(), callable.step_schema());
            assert_eq!(layout.site_id(), site_id);
            assert_eq!(layout.target_mode(), CallTargetMode::DynamicFallback);
            assert_eq!(
                layout.return_step_schema(),
                lowering.facts().callee_schema()
            );
            assert_eq!(
                layout.invoke_args_tuple_ty(),
                lowering.facts().invoke_args_tuple_ty()
            );
            assert!(layout.args_abi().is_elided());
            assert_eq!(layout.param_count(), 1);
            match layout.carrier() {
                RefactorDynamicInvokeCarrierLayout::ClosureObject(carrier) => {
                    assert_eq!(carrier.object_ty().count_fields(), 3);
                    assert_eq!(carrier.env_field_index(), 1);
                    assert_eq!(carrier.fn_field_index(), 2);
                    assert!(!carrier.receiver_abi().is_elided());
                }
                other => {
                    panic!("fun-value dynamic invoke 应发布 closure carrier，而不是 {other:?}")
                }
            }
        },
    );
}

#[test]
fn refactor_llvm_callable_carrier_layout_resolves_virtual_candidate_set_contracts() {
    with_fixture_query_result(
        "effect_refactor_dynamic_invoke_candidate_set_emit_llvm.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query =
                result.expect("candidate-set virtual helper 应可物化 dynamic invoke contract");
            let callable = inputs
                .abi_visibility_program
                .callable("fixtures.build.helper")
                .expect("fixtures.build.helper callable 应存在");
            let boundary = site_boundary(callable, BoundarySiteKind::Call);
            let lowering = call_boundary_lowering(boundary);

            assert_eq!(lowering.facts().target_mode(), CallTargetMode::CandidateSet);
            let site_id = boundary_site_id(boundary);
            let RefactorCallTargetQuery::DynamicInvoke(layout) = query
                .call_target_layout(callable.step_schema(), site_id, lowering.facts())
                .expect("candidate-set virtual boundary 应可回查 dynamic invoke contract")
            else {
                panic!("CandidateSet virtual call 应走 dynamic invoke contract");
            };
            assert_eq!(layout.target_mode(), CallTargetMode::CandidateSet);
            assert_eq!(layout.param_count(), 1);
            assert!(layout.args_abi().is_elided());
            assert_eq!(
                layout.return_step_schema(),
                lowering.facts().callee_schema()
            );
            match layout.carrier() {
                RefactorDynamicInvokeCarrierLayout::VirtualReceiver(dispatch) => {
                    assert_eq!(
                        inputs
                            .effect_lowered_stage_output
                            .types()
                            .display(dispatch.receiver_ty())
                            .to_string(),
                        "fixtures.build.Base"
                    );
                    assert_eq!(dispatch.owner_fqn(), "fixtures.build.Base");
                    assert_eq!(dispatch.member_name(), "ping");
                    assert!(!dispatch.receiver_abi().is_elided());
                }
                other => panic!(
                    "virtual CandidateSet 应发布 receiver-dispatch carrier，而不是 {other:?}"
                ),
            }
        },
    );
}

#[test]
fn refactor_llvm_dynamic_invoke_query_resolves_non_boundary_virtual_contract() {
    with_fixture_query(
        "effect_refactor_non_boundary_dynamic_call_emit_llvm.scoop",
        |inputs, query, _module| {
            let helper = inputs
                .abi_visibility_program
                .callable("fixtures.build.helper")
                .expect("fixtures.build.helper callable 应存在");
            let plain = helper
                .plain_abi()
                .expect("NoOutward helper 应保持 plain callable ABI");
            assert!(plain.local_effect_control().is_none());

            let (site_id, facts) = source_slice_non_boundary_dynamic_call_site(inputs, helper);
            assert!(
                facts.resolved_cases().is_empty(),
                "non-boundary dynamic call 的 resolved cases 应为空"
            );
            assert_eq!(facts.target_mode(), CallTargetMode::CandidateSet);
            assert!(
                plain
                    .call_sites()
                    .iter()
                    .any(|site| site.site_id() == site_id)
            );
            let layout = query
                .plain_callable_layout_by_version_key(helper.body_version_key())
                .expect("NoOutward helper 应发布 plain callable layout");
            assert_eq!(layout.root_fqn(), helper.root_fqn());
            assert!(
                query
                    .callable_layout_by_version_key(helper.body_version_key())
                    .is_err(),
                "NoOutward helper 不应发布 effect-step callable layout"
            );
        },
    );
}

#[test]
fn refactor_llvm_callable_carrier_layout_resolves_non_boundary_virtual_contracts() {
    with_fixture_query(
        "effect_refactor_non_boundary_dynamic_call_emit_llvm.scoop",
        |inputs, query, _module| {
            let helper = inputs
                .abi_visibility_program
                .callable("fixtures.build.helper")
                .expect("fixtures.build.helper callable 应存在");
            let (_site_id, facts) = source_slice_non_boundary_dynamic_call_site(inputs, helper);

            assert_eq!(facts.target_mode(), CallTargetMode::CandidateSet);
            let CallSiteTarget::CandidateSet(targets) = facts.target() else {
                panic!("non-boundary virtual call 应保留 CandidateSet target");
            };
            assert!(
                targets
                    .iter()
                    .any(|target| target.template.fqn == "fixtures.build.Base.ping")
            );
            assert!(
                query
                    .plain_callable_layout_by_version_key(helper.body_version_key())
                    .is_ok(),
                "NoOutward non-boundary dynamic call owner 应保持 plain callable layout"
            );
            for target in targets {
                assert!(
                    query
                        .plain_callable_layout_by_root_fqn(&target.template.fqn)
                        .is_ok(),
                    "NoOutward virtual target `{}` 应发布 plain callable layout",
                    target.template.fqn
                );
            }
        },
    );
}

#[test]
fn refactor_llvm_dynamic_invoke_query_rejects_missing_published_contract() {
    with_fixture_query_result(
        "effect_refactor_dynamic_invoke_candidate_set_emit_llvm.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let helper = program
                .callable("fixtures.build.helper")
                .expect("fixtures.build.helper callable 应存在");
            let bogus_site = crate::mir::SiteId::from_raw(999);
            let rewritten_boundary_map = LateLoweredBoundaryMap::new(
                helper
                    .boundary_map()
                    .entries()
                    .iter()
                    .map(|boundary| {
                        let source = match boundary.source() {
                            LateLoweredBoundarySource::Site {
                                kind: BoundarySiteKind::Call,
                                ..
                            } => LateLoweredBoundarySource::Site {
                                site_id: bogus_site,
                                kind: BoundarySiteKind::Call,
                            },
                            other => other,
                        };
                        let lowered = boundary
                            .lowering()
                            .cloned()
                            .expect("candidate-set helper 的 boundary 应带 lowering");
                        LateLoweredBoundary::new(
                            boundary.boundary_id(),
                            source,
                            boundary.owner_state(),
                            boundary.resume_state(),
                        )
                        .with_lowering(lowered)
                    })
                    .collect(),
            );
            let callables = program
                .callables()
                .iter()
                .map(|candidate| {
                    if candidate.body_step_schema() == Some(helper.step_schema()) {
                        clone_callable_with_boundary_map(candidate, rewritten_boundary_map.clone())
                    } else {
                        candidate.clone()
                    }
                })
                .collect();
            LateLoweredProgram::new(
                program.step_types().to_vec(),
                program.resume_packings().to_vec(),
                program.continuation_objects().to_vec(),
                callables,
            )
            .with_stable_instance_keys(program.stable_instance_keys().clone())
        },
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => panic!("缺失 dynamic-invoke contract 时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("canonical MIR call metadata"),
                "错误消息应指出缺失的是 call-site authoritative metadata: {message}"
            );
            assert!(
                message.contains("dynamic-invoke contract"),
                "错误消息应指出缺失的是 dynamic-invoke contract: {message}"
            );
            assert!(
                message.contains("fixtures.build.helper") && message.contains("999"),
                "错误消息应指出缺失 contract 所属的 callable 和 site id: {message}"
            );
        },
    );
}

#[test]
fn refactor_llvm_call_boundary_continuation_composition() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query = result.expect("refactor ABI materialization 应成功");
            let main = inputs
                .abi_visibility_program
                .callable("main")
                .expect("main callable 应存在");
            let composition = main
                .boundary_map()
                .entries()
                .iter()
                .find_map(|boundary| {
                    let Some(LateLoweredBoundaryLowering::Call(lowering)) = boundary.lowering()
                    else {
                        return None;
                    };
                    lowering.continuation_compositions().first()
                })
                .expect("main 的 fetch call boundary 应发布 composition contract");
            let continuation_layout = query
                .continuation_layout(main.continuation_object())
                .expect("main continuation object layout 应存在");
            assert!(continuation_layout.fields().iter().any(|field| {
                field.kind() == RefactorContinuationFieldKind::CapturedCalleeSuspendStateRef
            }));
            let callee_surface = query
                .surface_resume_layout(composition.callee_continuation_schema())
                .expect("callee continuation surface resume ABI 应发布");
            assert_eq!(
                callee_surface.return_step_schema(),
                composition.input_step_schema()
            );
            assert_eq!(
                callee_surface.resume_tuple_ty(),
                composition.callee_continuation_contract().resume_tuple_ty()
            );
        },
    );

    with_phase_fixture_query_result(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let main = program.callable("main").expect("main callable 应存在");
            let boundary_map = LateLoweredBoundaryMap::new(
                main.boundary_map()
                    .entries()
                    .iter()
                    .map(|boundary| {
                        let lowering = match boundary
                            .lowering()
                            .cloned()
                            .expect("main boundary 应带 lowering")
                        {
                            LateLoweredBoundaryLowering::Call(lowering) => {
                                LateLoweredBoundaryLowering::Call(
                                    LateLoweredCallBoundaryLowering::new(
                                        lowering.facts().clone(),
                                        lowering.result_local(),
                                        lowering.operand_contract().clone(),
                                        lowering.dispatch().clone(),
                                        Vec::new(),
                                        lowering.consumed_runtime_error_case().cloned(),
                                    ),
                                )
                            }
                            other => other,
                        };
                        LateLoweredBoundary::new(
                            boundary.boundary_id(),
                            boundary.source(),
                            boundary.owner_state(),
                            boundary.resume_state(),
                        )
                        .with_lowering(lowering)
                    })
                    .collect(),
            );
            let callables = program
                .callables()
                .iter()
                .map(|candidate| {
                    if candidate.body_step_schema() == Some(main.step_schema()) {
                        clone_callable_with_boundary_map(candidate, boundary_map.clone())
                    } else {
                        candidate.clone()
                    }
                })
                .collect();
            LateLoweredProgram::new(
                program.step_types().to_vec(),
                program.resume_packings().to_vec(),
                program.continuation_objects().to_vec(),
                callables,
            )
            .with_stable_instance_keys(program.stable_instance_keys().clone())
        },
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => panic!("缺失 continuation composition 时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("continuation composition"),
                "错误消息应指出缺失 call-boundary continuation composition: {message}"
            );
        },
    );
}

#[test]
fn refactor_llvm_dynamic_entry_publication_declares_closure_vtable_and_itable_targets() {
    with_inputs_query_result_and_codegen(
        build_fixture_inputs("effect_refactor_dynamic_entry_publication_emit_llvm.scoop"),
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, codegen, result, module| {
            let query = result.expect("refactor ABI materialization 应成功");
            let make_closure_callable = inputs
                .abi_visibility_program
                .callable("fixtures.build.makeClosure")
                .expect("makeClosure callable 应存在");
            let base_ping_callable = inputs
                .abi_visibility_program
                .callable("fixtures.build.Base.ping")
                .expect("Base.ping callable 应存在");
            let derived_ping_callable = inputs
                .abi_visibility_program
                .callable("fixtures.build.Derived.ping")
                .expect("Derived.ping callable 应存在");

            let make_closure = query
                .plain_callable_layout_by_version_key(make_closure_callable.body_version_key())
                .expect("makeClosure plain callable target 应存在");
            let base_vtable = query
                .plain_callable_layout_by_version_key(base_ping_callable.body_version_key())
                .expect("Base.ping plain callable target 应存在");
            let derived_vtable = query
                .plain_callable_layout_by_version_key(derived_ping_callable.body_version_key())
                .expect("Derived.ping plain callable target 应存在");

            assert_eq!(
                make_closure.body_version_key(),
                make_closure_callable.body_version_key()
            );
            assert_eq!(
                base_vtable.body_version_key(),
                base_ping_callable.body_version_key()
            );
            assert_eq!(
                derived_vtable.body_version_key(),
                derived_ping_callable.body_version_key()
            );

            for (kind, fqn) in [
                (
                    CallableCarrierKind::ClosureObject,
                    "fixtures.build.makeClosure",
                ),
                (CallableCarrierKind::ClassVtable, "fixtures.build.Base.ping"),
                (
                    CallableCarrierKind::InterfaceItable,
                    "fixtures.build.Base.ping",
                ),
                (
                    CallableCarrierKind::ClassVtable,
                    "fixtures.build.Derived.ping",
                ),
                (
                    CallableCarrierKind::InterfaceItable,
                    "fixtures.build.Derived.ping",
                ),
            ] {
                assert!(
                    query.callable_carrier_target_layout(kind, fqn).is_err(),
                    "NoOutward carrier `{fqn}` 不应发布 effect-step dynamic entry target"
                );
                assert!(
                    codegen.plain_callable_carrier_fallback_allowed(kind, fqn),
                    "NoOutward carrier `{fqn}` 应发布 plain callable fallback"
                );
            }

            let _ = codegen
                .get_or_create_class_vtable_global(dummy_span(), "fixtures.build.Base")
                .expect("Base vtable 应可物化");
            let _ = codegen
                .get_or_create_class_vtable_global(dummy_span(), "fixtures.build.Derived")
                .expect("Derived vtable 应可物化");
            let _ = codegen
                .get_or_create_class_itable_global(dummy_span(), "fixtures.build.Base")
                .expect("Base itable 应可物化");
            let _ = codegen
                .get_or_create_class_itable_global(dummy_span(), "fixtures.build.Derived")
                .expect("Derived itable 应可物化");

            assert!(
                module
                    .get_function(make_closure.direct_entry().symbol_name())
                    .is_some()
            );
            assert!(
                module
                    .get_function(base_vtable.direct_entry().symbol_name())
                    .is_some()
            );
            assert!(
                module
                    .get_function(derived_vtable.direct_entry().symbol_name())
                    .is_some()
            );
        },
    );
}

#[test]
fn refactor_llvm_callable_carrier_version_selection_rejects_ambiguous_root_targets() {
    with_fixture_query_result(
        "effect_refactor_dynamic_entry_publication_emit_llvm.scoop",
        |inputs| {
            duplicate_no_outward_callable_version(
                &inputs.abi_visibility_program,
                "fixtures.build.makeClosure",
            )
        },
        |_inputs, result, _module| {
            let query = result.expect("duplicated plain versions 应允许物化到 version-key 查询面");
            let err = match query.plain_callable_layout_by_root_fqn("fixtures.build.makeClosure") {
                Ok(_) => panic!("歧义 root 查询必须要求调用方改用 body version key"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("fixtures.build.makeClosure"),
                "错误消息应指出歧义 callable: {message}"
            );
            assert!(
                message.contains("多个 published callable version"),
                "错误消息应指出存在多个 callable version: {message}"
            );
            assert!(
                message.contains("body version key"),
                "错误消息应指出歧义 version key: {message}"
            );
        },
    );
}

#[test]
fn refactor_llvm_dynamic_entry_publication_rejects_missing_dispatch_callable_shell() {
    with_inputs_query_result_and_codegen(
        build_fixture_inputs("effect_refactor_dynamic_entry_publication_emit_llvm.scoop"),
        |inputs| inputs.abi_visibility_program.clone(),
        |_inputs, codegen, result, _module| {
            let _ = result.expect("ABI materialization 应成功");
            let dummy_fn = codegen.declare_compiler_private_helper_function(
                "__scoop_refactor_missing_carrier_target_dummy",
                codegen.context.void_type().fn_type(&[], false),
                Linkage::External,
            );
            let err = match codegen.callable_carrier_target_fn_ptr(
                CallableCarrierKind::ClassVtable,
                "fixtures.build.Missing.ping",
                dummy_fn.as_global_value().as_pointer_value(),
            ) {
                Ok(_) => panic!("缺失 dispatch callable shell 时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("fixtures.build.Missing.ping"),
                "错误消息应指出缺失 shell 的 target callable: {message}"
            );
            assert!(
                message.contains("published target entry") || message.contains("class vtable slot"),
                "错误消息应指出问题出在 carrier target 发布: {message}"
            );
        },
    );
}

#[test]
fn refactor_llvm_local_runtime_error_contract_resolves_pure_call_boundary_targets() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_resume_if_else_branch_single_perform.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, module| {
            let query =
                result.expect("pure caller local runtime-error contract 应可发布到 ABI query");
            let main = inputs
                .effect_lowered_stage_output
                .program()
                .callable("main")
                .expect("main callable 应存在");
            let mut checked = 0usize;

            for boundary in main.boundary_map().entries() {
                let Some(LateLoweredBoundaryLowering::Call(lowering)) = boundary.lowering() else {
                    continue;
                };
                let Some(contract) = lowering.consumed_runtime_error_case() else {
                    continue;
                };
                let site_id = boundary_site_id(boundary);
                let published = query
                    .call_local_runtime_error_contract(main.step_schema(), site_id, contract)
                    .expect("call boundary 应可回查 published local runtime-error contract");

                assert_eq!(published.owner_step_schema(), main.step_schema());
                assert_eq!(published.site_id(), site_id);
                assert_eq!(published.input_case_tag(), contract.input_case_tag());
                assert_eq!(published.payload_tuple_ty(), contract.payload_tuple_ty());
                assert_eq!(
                    published.terminal_action().lowered_action(),
                    contract.terminal_action()
                );
                assert_eq!(published.target_state(), contract.target_state());
                assert!(
                    !published.payload_abi().is_elided(),
                    "RuntimeError payload 不应被零载荷退化"
                );
                let runtime_entry = published.terminal_action().runtime_entry();
                assert_eq!(
                    runtime_entry.kind(),
                    LateLoweredPublishedRuntimeEntry::RuntimeErrorFatal
                );
                assert_eq!(runtime_entry.symbol_name(), "scoop_runtime_error_fatal");
                assert_eq!(runtime_entry.param_count(), 1);
                assert!(
                    module.get_function(runtime_entry.symbol_name()).is_some(),
                    "published runtime fatal entry 应声明到 LLVM module 中"
                );
                checked += 1;
            }

            assert_eq!(
                checked, 2,
                "fixture 应包含两个 pure caller call boundary contract"
            );
        },
    );
}

#[test]
fn refactor_llvm_local_runtime_error_contract_rejects_missing_target_state() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_resume_if_else_branch_single_perform.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let main = program.callable("main").expect("main callable 应存在");
            let boundary_map = LateLoweredBoundaryMap::new(
                main.boundary_map()
                    .entries()
                    .iter()
                    .map(|boundary| {
                        let lowering = match boundary
                            .lowering()
                            .cloned()
                            .expect("main boundary 应带 lowering")
                        {
                            LateLoweredBoundaryLowering::Call(lowering) => {
                                let consumed_runtime_error_case = lowering
                                    .consumed_runtime_error_case()
                                    .cloned()
                                    .map(|contract| {
                                        LateLoweredConsumedRuntimeErrorCase::new(
                                            contract.input_case_tag(),
                                            contract.input_concrete_op_key().clone(),
                                            contract.payload_tuple_ty(),
                                            contract.terminal_action(),
                                            StateId::new(999),
                                        )
                                    });
                                LateLoweredBoundaryLowering::Call(
                                    LateLoweredCallBoundaryLowering::new(
                                        lowering.facts().clone(),
                                        lowering.result_local(),
                                        lowering.operand_contract().clone(),
                                        lowering.dispatch().clone(),
                                        lowering.continuation_compositions().to_vec(),
                                        consumed_runtime_error_case,
                                    ),
                                )
                            }
                            other => other,
                        };
                        LateLoweredBoundary::new(
                            boundary.boundary_id(),
                            boundary.source(),
                            boundary.owner_state(),
                            boundary.resume_state(),
                        )
                        .with_lowering(lowering)
                    })
                    .collect(),
            );
            let callables = program
                .callables()
                .iter()
                .map(|candidate| {
                    if candidate.body_step_schema() == Some(main.step_schema()) {
                        clone_callable_with_boundary_map(candidate, boundary_map.clone())
                    } else {
                        candidate.clone()
                    }
                })
                .collect();
            LateLoweredProgram::new(
                program.step_types().to_vec(),
                program.resume_packings().to_vec(),
                program.continuation_objects().to_vec(),
                callables,
            )
            .with_stable_instance_keys(program.stable_instance_keys().clone())
        },
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => panic!("缺失 local runtime-error target state 时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("local runtime-error target state"),
                "错误消息应指出缺失的是 local runtime-error target state: {message}"
            );
            assert!(
                message.contains("main") && message.contains("call site 1"),
                "错误消息应指出缺失 contract 所属的 callable 和 site id: {message}"
            );
        },
    );
}

#[test]
fn refactor_llvm_local_runtime_error_contract_rejects_non_local_runtime_error_terminator() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_resume_if_else_branch_single_perform.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let main = program.callable("main").expect("main callable 应存在");
            let local_runtime_error_states = main
                .boundary_map()
                .entries()
                .iter()
                .filter_map(|boundary| {
                    let Some(LateLoweredBoundaryLowering::Call(lowering)) = boundary.lowering()
                    else {
                        return None;
                    };
                    lowering
                        .consumed_runtime_error_case()
                        .map(|contract| contract.target_state())
                })
                .collect::<BTreeSet<_>>();
            let rewritten_states = main
                .state_graph()
                .states()
                .iter()
                .map(|state| {
                    if !local_runtime_error_states.contains(&state.state_id()) {
                        return state.clone();
                    }
                    crate::effect_lowered::ir::LateLoweredState::new(
                        state.state_id(),
                        state.role(),
                        state.source_slices().to_vec(),
                        crate::effect_lowered::ir::LateLoweredStateTerminator::Unreachable,
                    )
                })
                .collect::<Vec<_>>();
            let state_graph = crate::effect_lowered::ir::LateLoweredStateGraph::new(
                main.state_graph().entry_state(),
                main.state_graph().complete_state(),
                main.state_graph().cleanup_state(),
                main.state_graph().drop_state(),
                rewritten_states,
            );
            let callables = program
                .callables()
                .iter()
                .map(|candidate| {
                    if candidate.body_step_schema() == Some(main.step_schema()) {
                        clone_callable_with_state_graph(candidate, state_graph.clone())
                    } else {
                        candidate.clone()
                    }
                })
                .collect();
            LateLoweredProgram::new(
                program.step_types().to_vec(),
                program.resume_packings().to_vec(),
                program.continuation_objects().to_vec(),
                callables,
            )
            .with_stable_instance_keys(program.stable_instance_keys().clone())
        },
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => panic!("缺失 LocalRuntimeError terminal contract 时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("不是 LocalRuntimeError terminator"),
                "错误消息应指出 local runtime-error target state 缺少终止 contract: {message}"
            );
            assert!(
                message.contains("main") && message.contains("call site 1"),
                "错误消息应指出缺失 contract 所属的 callable 和 site id: {message}"
            );
        },
    );
}

#[test]
fn refactor_handle_dispatch_contract_publishes_llvm_query_layout() {
    with_phase_fixture_query_result(
        "effect_facts",
        "nested_handle_self_contained_vs_outward.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query = result.expect("HandleDispatch contract 应可发布到 LLVM ABI query");
            let callable = inputs
                .effect_lowered_stage_output
                .program()
                .callable("sample.nested_may_suspend_outward")
                .expect("callable 应存在");
            let site_id = SiteId::from_raw(1);
            let contract = handle_dispatch_contract(callable, site_id);
            let published = query
                .handle_dispatch_layout(callable.step_schema(), site_id, contract)
                .expect("query 应能稳定回查 HandleDispatch contract");
            let frame_layout = query
                .frame_layout(callable.step_schema())
                .expect("frame layout 应可查询");

            assert_eq!(published.owner_step_schema(), callable.step_schema());
            assert_eq!(published.site_id(), site_id);
            assert_eq!(published.lowered_contract(), contract);
            assert_eq!(
                published.state_tag_field_index(),
                frame_layout
                    .field_index_for_system(SystemSlotKind::StateTag)
                    .expect("frame 应保留 StateTag")
            );
            assert_eq!(
                published.completion_tag_field_index(),
                frame_layout
                    .field_index_for_system(SystemSlotKind::CompletionTag)
                    .expect("frame 应保留 CompletionTag")
            );
            assert_eq!(
                published.payload_carrier_field_index(),
                frame_layout
                    .field_index_for_system(SystemSlotKind::ResumePayloadCarrier)
                    .expect("frame 应保留 ResumePayloadCarrier")
            );
            assert!(
                published
                    .completion_tag_value(LateLoweredHandlePendingCompletion::ContinueToExit)
                    .is_some()
            );
            assert!(
                published
                    .completion_tag_value(LateLoweredHandlePendingCompletion::ReturnFromFunction)
                    .is_some()
            );
            assert!(
                published
                    .completion_tag_value(LateLoweredHandlePendingCompletion::PropagateOutward(
                        crate::effect_facts::CaseTag::new(1),
                    ))
                    .is_none()
            );
        },
    );
}

#[test]
fn refactor_llvm_handle_dispatch_publishes_pending_payload_transport_layout() {
    with_inputs_query_result(
        build_fixture_inputs_from_source(SourceFile::new_virtual(
            "<mem>/llvm_handle_pending_payload_transport.scoop",
            r#"
package sample

effect Inner {
fun go(): Int
}

effect Outer {
fun again(): Unit
}

fun cleanup() {}

fun propagate_before_finally(): Int {
return handle {
    val nested: Int = handle {
        Outer.again()
        0
    } with {
        Inner.go() -> 1
    } finally {
        cleanup()
    }
    nested + 10
} with {
    Outer.again() -> 99
}
}
"#,
        )),
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query =
                result.expect("pending payload transport 应可发布到 HandleDispatch LLVM query");
            let callable = inputs
                .effect_lowered_stage_output
                .program()
                .callable("sample.propagate_before_finally")
                .expect("sample.propagate_before_finally callable 应存在");
            let (site_id, contract) = handle_dispatch_with_pending_outward(callable);
            let published = query
                .handle_dispatch_layout(callable.step_schema(), site_id, contract)
                .expect("query 应能稳定回查 pending payload transport contract");
            let pending_case = *contract
                .body_outward_cases()
                .first()
                .expect("fixture 应发布 body outward case");
            let transport = published
                .pending_payload_transport_layout(
                    LateLoweredHandlePendingCompletion::PropagateOutward(pending_case),
                )
                .expect("pending outward case 应发布 typed payload transport layout");
            let frame_layout = query
                .frame_layout(callable.step_schema())
                .expect("frame layout 应可查询");
            let slot = callable
                .frame_schema()
                .slot_for_kind(LateLoweredFrameSlotKind::HandlePendingPayload {
                    site_id,
                    case_tag: pending_case,
                })
                .expect("frame schema 应保留 HandlePendingPayload slot");

            assert_eq!(
                transport.completion(),
                LateLoweredHandlePendingCompletion::PropagateOutward(pending_case)
            );
            assert_eq!(transport.frame_slot(), slot.slot_id());
            assert_eq!(
                transport.frame_field_index(),
                frame_layout
                    .field_index_for_slot(slot.slot_id())
                    .expect("frame layout 应可回查 pending payload field")
            );
            assert_eq!(
                transport.payload_tuple_ty(),
                contract
                    .outward_emission(pending_case)
                    .expect("pending outward case 应保留 outward emission")
                    .payload_tuple_ty()
            );
            assert!(
                published
                    .pending_payload_transport_layout(
                        LateLoweredHandlePendingCompletion::ContinueToExit,
                    )
                    .is_none()
            );
        },
    );
}

#[test]
fn refactor_llvm_handle_dispatch_rejects_missing_pending_payload_transport() {
    with_inputs_query_result(
        build_fixture_inputs_from_source(SourceFile::new_virtual(
            "<mem>/llvm_handle_pending_payload_transport_missing.scoop",
            r#"
package sample

effect Inner {
fun go(): Int
}

effect Outer {
fun again(): Unit
}

fun cleanup() {}

fun propagate_before_finally(): Int {
return handle {
    val nested: Int = handle {
        Outer.again()
        0
    } with {
        Inner.go() -> 1
    } finally {
        cleanup()
    }
    nested + 10
} with {
    Outer.again() -> 99
}
}
"#,
        )),
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let callable = program
                .callable("sample.propagate_before_finally")
                .expect("callable 应存在");
            let (site_id, contract) = handle_dispatch_with_pending_outward(callable);
            let broken_contract = LateLoweredHandleDispatchContract::new(
                contract.carrier(),
                contract.body_complete_target(),
                contract.arm_complete_target(),
                contract.finally_complete_target(),
                contract.body_completion_payload_source().cloned(),
                contract.handled_arms().to_vec(),
                contract.body_outward_cases().to_vec(),
                contract.finally_outward_cases().to_vec(),
                contract.outward_emissions().to_vec(),
                contract.pending_completions().to_vec(),
                contract.pending_completion_origins().to_vec(),
                Vec::new(),
                contract.state_regions().to_vec(),
                contract.boundary_routings().to_vec(),
                contract.abandon_target(),
            );
            let state_graph = clone_state_graph_with_handle_contract(
                callable.state_graph(),
                site_id,
                broken_contract,
            );
            let callables = program
                .callables()
                .iter()
                .map(|candidate| {
                    if candidate.body_step_schema() == Some(callable.step_schema()) {
                        clone_callable_with_state_graph(candidate, state_graph.clone())
                    } else {
                        candidate.clone()
                    }
                })
                .collect();
            LateLoweredProgram::new(
                program.step_types().to_vec(),
                program.resume_packings().to_vec(),
                program.continuation_objects().to_vec(),
                callables,
            )
            .with_stable_instance_keys(program.stable_instance_keys().clone())
        },
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => panic!("缺失 pending payload transport 时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("pending payload transport"),
                "错误消息应指出缺失的是 pending payload transport contract: {message}"
            );
            assert!(
                message.contains("sample.propagate_before_finally")
                    && message.contains("handle site"),
                "错误消息应指出出错 callable 和 site: {message}"
            );
        },
    );
}

#[test]
fn refactor_handle_dispatch_region_routing_publishes_query_lookup() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_resume_if_else_branch_single_perform.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query = result.expect("handle region routing contract 应可发布到 LLVM ABI query");
            let callable = inputs
                .effect_lowered_stage_output
                .program()
                .callable("run")
                .expect("run callable 应存在");
            let (site_id, contract) = first_handle_dispatch(callable);
            let published = query
                .handle_dispatch_layout(callable.step_schema(), site_id, contract)
                .expect("query 应能稳定回查 handle region routing contract");
            let perform_boundary = callable
                .boundary_map()
                .entries()
                .iter()
                .find(|boundary| {
                    matches!(
                        boundary.source(),
                        LateLoweredBoundarySource::Site {
                            kind: BoundarySiteKind::Perform,
                            ..
                        }
                    )
                })
                .expect("fixture 应发布 body perform boundary");
            let routing = published
                .boundary_routing(perform_boundary.boundary_id())
                .expect("perform boundary 应可通过 query 回查 routing contract");
            let handled_arm = contract
                .handled_arms()
                .first()
                .expect("fixture 应发布唯一 handled arm");
            let handled_route = routing
                .case_routing(handled_arm.handled_case())
                .expect("handled perform case 应发布 consume-to-arm routing");

            assert_eq!(
                routing.owner_region(),
                crate::effect_lowered::ir::LateLoweredHandleStateRegion::Body
            );
            assert_eq!(
                published.state_region(routing.owner_state()),
                crate::effect_lowered::ir::LateLoweredHandleStateRegion::Body
            );
            assert_eq!(
                published.state_region(routing.resume_state()),
                crate::effect_lowered::ir::LateLoweredHandleStateRegion::Body
            );
            assert!(matches!(
                handled_route.action(),
                crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::ConsumeToArm {
                    arm_state,
                    arm_ordinal,
                    continuation_resume_state,
                } if arm_state == handled_arm.arm_state()
                    && arm_ordinal == handled_arm.arm_ordinal()
                    && continuation_resume_state == routing.resume_state()
            ));
        },
    );
}

#[test]
fn refactor_handle_dispatch_region_routing_rejects_resume_state_drift() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_resume_if_else_branch_single_perform.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let callable = program.callable("run").expect("run callable 应存在");
            let (site_id, contract) = first_handle_dispatch(callable);
            let handled_case = contract
                .handled_arms()
                .first()
                .expect("fixture 应发布唯一 handled arm")
                .handled_case();
            let broken_routings = contract
                .boundary_routings()
                .iter()
                .map(|routing| {
                    let broken_case_routings = routing
                        .case_routings()
                        .iter()
                        .map(|route| {
                            if route.case_tag() != handled_case {
                                return *route;
                            }
                            let broken_action = match route.action() {
                                crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::ConsumeToArm {
                                    arm_state,
                                    arm_ordinal,
                                    ..
                                } => crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::ConsumeToArm {
                                    arm_state,
                                    arm_ordinal,
                                    continuation_resume_state: contract.body_complete_target(),
                                },
                                other => other,
                            };
                            crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRouting::new(
                                route.case_tag(),
                                broken_action,
                            )
                        })
                        .collect::<Vec<_>>();
                    crate::effect_lowered::ir::LateLoweredHandleBoundaryRouting::new(
                        routing.boundary_id(),
                        routing.owner_state(),
                        routing.owner_region(),
                        routing.resume_state(),
                        broken_case_routings,
                    )
                })
                .collect::<Vec<_>>();
            let broken_contract = clone_handle_dispatch_contract_with_regions_and_routes(
                contract,
                contract.state_regions().to_vec(),
                broken_routings,
            );
            let state_graph = clone_state_graph_with_handle_contract(
                callable.state_graph(),
                site_id,
                broken_contract,
            );
            let callables = program
                .callables()
                .iter()
                .map(|candidate| {
                    if candidate.body_step_schema() == Some(callable.step_schema()) {
                        clone_callable_with_state_graph(candidate, state_graph.clone())
                    } else {
                        candidate.clone()
                    }
                })
                .collect();
            LateLoweredProgram::new(
                program.step_types().to_vec(),
                program.resume_packings().to_vec(),
                program.continuation_objects().to_vec(),
                callables,
            )
            .with_stable_instance_keys(program.stable_instance_keys().clone())
        },
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => panic!("handle boundary routing resume_state 漂移时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("boundary-routing contract 漂移")
                    || message.contains("consume_to_arm")
                    || message.contains("resume=st"),
                "错误消息应指出 published routing 与 state graph/boundary map 不一致: {message}"
            );
        },
    );
}

#[test]
fn refactor_handle_arm_binding_contract_publishes_llvm_query_layout() {
    with_inputs_query_result(
        build_fixture_inputs_from_source(SourceFile::new_virtual(
            "<mem>/llvm_handle_arm_binding_single.scoop",
            r#"
package sample

import scoop.core.*

effect Edge {
fun visit(from: String, to: Int): Int
}

fun run(): Int {
return handle {
    Edge.visit("alpha", 1)
} with {
    Edge.visit(from, to), k -> {
        k.resume(to + 1)
    }
}
}

fun main(): Int {
return 0
}
"#,
        )),
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query = result.expect("handle arm binder contract 应可发布到 LLVM ABI query");
            let callable = inputs
                .effect_lowered_stage_output
                .program()
                .callable("sample.run")
                .expect("sample.run callable 应存在");
            let (site_id, contract) = first_handle_dispatch(callable);
            let published = query
                .handle_dispatch_layout(callable.step_schema(), site_id, contract)
                .expect("query 应能稳定回查 HandleDispatch arm binder contract");
            let arm = published
                .handled_arms()
                .first()
                .expect("单 arm fixture 应发布唯一 handled arm layout");

            assert_eq!(arm.arm_ordinal(), 0);
            assert_eq!(arm.payload_binders().len(), 2);
            assert_eq!(arm.payload_binders()[0].ordinal(), 0);
            assert_eq!(arm.payload_binders()[1].ordinal(), 1);
            let continuation_binder = arm
                .continuation_binder()
                .expect("escape continuation arm 应发布 continuation binder layout");
            assert_eq!(
                continuation_binder.continuation_object(),
                callable.continuation_object()
            );
            assert_eq!(
                continuation_binder.surface_resume_source_kind(),
                crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::HandleContinuationBinderOnly
            );
            assert_eq!(
                continuation_binder.surface_resume_return_step_schema(),
                callable.step_schema()
            );
        },
    );
}

#[test]
fn refactor_handle_arm_continuation_binding_publishes_mixed_multi_arm_query_layout() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query = result.expect("mixed multi-arm handle 应可发布 arm binder query");
            let callable = inputs
                .effect_lowered_stage_output
                .program()
                .callable("main")
                .expect("main callable 应存在");
            let (site_id, contract) = first_handle_dispatch(callable);
            let published = query
                .handle_dispatch_layout(callable.step_schema(), site_id, contract)
                .expect("query 应能稳定回查 mixed handle arm binder contract");

            assert_eq!(published.handled_arms().len(), 2);
            let escape_arm = published
                .handled_arms()
                .iter()
                .find(|arm| arm.continuation_binder().is_some())
                .expect("mixed fixture 应发布带 continuation binder 的 arm layout");
            let payload_only_arm = published
                .handled_arms()
                .iter()
                .find(|arm| arm.continuation_binder().is_none())
                .expect("mixed fixture 应发布纯 payload arm layout");

            assert_eq!(escape_arm.payload_binders().len(), 1);
            assert_eq!(payload_only_arm.payload_binders().len(), 1);
            let continuation_binder = escape_arm
                .continuation_binder()
                .expect("escape arm 应带 continuation binder layout");
            assert_eq!(
                continuation_binder.surface_resume_source_kind(),
                crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::HandleContinuationBinderOnly
            );
        },
    );
}

#[test]
fn refactor_completion_state_contract_rejects_missing_completion_tag_slot() {
    with_phase_fixture_query_result(
        "effect_facts",
        "nested_handle_self_contained_vs_outward.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let callable = program
                .callable("sample.nested_may_suspend_outward")
                .expect("callable 应存在");
            let frame_schema = LateLoweredFrameSchema::new(
                callable
                    .frame_schema()
                    .slots()
                    .iter()
                    .filter(|slot| {
                        slot.kind()
                            != LateLoweredFrameSlotKind::System(SystemSlotKind::CompletionTag)
                    })
                    .cloned()
                    .collect(),
            )
            .with_resume_payload_bindings(
                callable.frame_schema().resume_payload_bindings().to_vec(),
            )
            .with_completion_payload_bindings(
                callable
                    .frame_schema()
                    .completion_payload_bindings()
                    .to_vec(),
            );
            let callables = program
                .callables()
                .iter()
                .map(|candidate| {
                    if candidate.body_step_schema() == Some(callable.step_schema()) {
                        clone_callable_with_frame_schema(candidate, frame_schema.clone())
                    } else {
                        candidate.clone()
                    }
                })
                .collect();
            LateLoweredProgram::new(
                program.step_types().to_vec(),
                program.resume_packings().to_vec(),
                program.continuation_objects().to_vec(),
                callables,
            )
            .with_stable_instance_keys(program.stable_instance_keys().clone())
        },
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => panic!("缺失 CompletionTag system field 时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("缺少 CompletionTag system field"),
                "错误消息应指出缺失的是 CompletionTag 槽位: {message}"
            );
            assert!(
                message.contains("sample.nested_may_suspend_outward"),
                "错误消息应指出出错 callable: {message}"
            );
        },
    );
}

#[test]
fn refactor_handle_arm_binding_contract_rejects_payload_binder_order_drift() {
    with_inputs_query_result(
        build_fixture_inputs_from_source(SourceFile::new_virtual(
            "<mem>/llvm_handle_arm_binding_order_drift.scoop",
            r#"
package sample

import scoop.core.*

effect Edge {
fun visit(from: String, to: Int): Int
}

fun run(): Int {
return handle {
    Edge.visit("alpha", 1)
} with {
    Edge.visit(from, to), k -> {
        k.resume(to + 1)
    }
}
}

fun main(): Int {
return 0
}
"#,
        )),
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let callable = program
                .callable("sample.run")
                .expect("sample.run callable 应存在");
            let (site_id, contract) = first_handle_dispatch(callable);
            let original_arm = contract
                .handled_arms()
                .first()
                .expect("fixture 应发布唯一 handled arm");
            let mut swapped_binders = original_arm.payload_binders().to_vec();
            swapped_binders.swap(0, 1);
            let broken_arm = crate::effect_lowered::ir::LateLoweredHandleArmDispatch::new(
                original_arm.handled_case(),
                original_arm.arm_state(),
                original_arm.arm_ordinal(),
                original_arm.payload_tuple_ty(),
                original_arm.completion_payload_source().clone(),
                swapped_binders,
                original_arm.continuation_binder(),
                original_arm.arm_outward_cases().to_vec(),
            );
            let broken_contract =
                clone_handle_dispatch_contract_with_handled_arms(contract, vec![broken_arm]);
            let state_graph = clone_state_graph_with_handle_contract(
                callable.state_graph(),
                site_id,
                broken_contract,
            );
            let callables = program
                .callables()
                .iter()
                .map(|candidate| {
                    if candidate.body_step_schema() == Some(callable.step_schema()) {
                        clone_callable_with_state_graph(candidate, state_graph.clone())
                    } else {
                        candidate.clone()
                    }
                })
                .collect();
            LateLoweredProgram::new(
                program.step_types().to_vec(),
                program.resume_packings().to_vec(),
                program.continuation_objects().to_vec(),
                callables,
            )
            .with_stable_instance_keys(program.stable_instance_keys().clone())
        },
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => panic!("payload binder 次序漂移时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("payload binder ordinal 漂移")
                    || message.contains("payload binder #0 local 漂移"),
                "错误消息应指出 payload binder 顺序漂移: {message}"
            );
        },
    );
}

#[test]
fn refactor_handle_dispatch_contract_rejects_missing_handled_arm_mapping() {
    with_phase_fixture_query_result(
        "effect_facts",
        "nested_handle_self_contained_vs_outward.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let callable = program
                .callable("sample.nested_may_suspend_outward")
                .expect("callable 应存在");
            let site_id = SiteId::from_raw(1);
            let contract = handle_dispatch_contract(callable, site_id);
            let broken_contract = LateLoweredHandleDispatchContract::new(
                contract.carrier(),
                contract.body_complete_target(),
                contract.arm_complete_target(),
                contract.finally_complete_target(),
                contract.body_completion_payload_source().cloned(),
                Vec::new(),
                contract.body_outward_cases().to_vec(),
                contract.finally_outward_cases().to_vec(),
                contract.outward_emissions().to_vec(),
                contract.pending_completions().to_vec(),
                contract.pending_completion_origins().to_vec(),
                contract.pending_payload_transports().to_vec(),
                contract.state_regions().to_vec(),
                contract.boundary_routings().to_vec(),
                contract.abandon_target(),
            );
            let state_graph = clone_state_graph_with_handle_contract(
                callable.state_graph(),
                site_id,
                broken_contract,
            );
            let callables = program
                .callables()
                .iter()
                .map(|candidate| {
                    if candidate.body_step_schema() == Some(callable.step_schema()) {
                        clone_callable_with_state_graph(candidate, state_graph.clone())
                    } else {
                        candidate.clone()
                    }
                })
                .collect();
            LateLoweredProgram::new(
                program.step_types().to_vec(),
                program.resume_packings().to_vec(),
                program.continuation_objects().to_vec(),
                callables,
            )
        },
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => panic!("缺失 handled-arm 映射时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("handled-arm 数量"),
                "错误消息应指出缺失的是 handled-arm mapping: {message}"
            );
            assert!(
                message.contains("handle site 1") || message.contains("site 1"),
                "错误消息应指出出错 site: {message}"
            );
        },
    );
}

#[test]
fn refactor_handle_arm_continuation_binding_rejects_missing_published_continuation_binder() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let callable = program.callable("main").expect("main callable 应存在");
            let (site_id, contract) = first_handle_dispatch(callable);
            let broken_arms = contract
                .handled_arms()
                .iter()
                .map(|arm| {
                    crate::effect_lowered::ir::LateLoweredHandleArmDispatch::new(
                        arm.handled_case(),
                        arm.arm_state(),
                        arm.arm_ordinal(),
                        arm.payload_tuple_ty(),
                        arm.completion_payload_source().clone(),
                        arm.payload_binders().to_vec(),
                        None,
                        arm.arm_outward_cases().to_vec(),
                    )
                })
                .collect::<Vec<_>>();
            let broken_contract =
                clone_handle_dispatch_contract_with_handled_arms(contract, broken_arms);
            let state_graph = clone_state_graph_with_handle_contract(
                callable.state_graph(),
                site_id,
                broken_contract,
            );
            let callables = program
                .callables()
                .iter()
                .map(|candidate| {
                    if candidate.body_step_schema() == Some(callable.step_schema()) {
                        clone_callable_with_state_graph(candidate, state_graph.clone())
                    } else {
                        candidate.clone()
                    }
                })
                .collect();
            LateLoweredProgram::new(
                program.step_types().to_vec(),
                program.resume_packings().to_vec(),
                program.continuation_objects().to_vec(),
                callables,
            )
        },
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => panic!("缺失 published continuation binder 时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("underlying continuation route")
                    && message.contains("HandleContinuationBinder"),
                "错误消息应指出缺失的是 continuation binder contract: {message}"
            );
        },
    );
}

#[test]
fn refactor_llvm_surface_resume_layout_keeps_shared_schema_multi_case_object_publications() {
    with_fixture_query_result(
        "effect_refactor_step_enum_single_case.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query = result.expect("shared-schema fixture 应可物化 surface-resume ABI");
            let callable = inputs
                .abi_visibility_program
                .callable("fixtures.build.singleCaseWorker")
                .expect("singleCaseWorker callable 应存在");
            let step = inputs
                .abi_visibility_program
                .step_type(callable.step_schema())
                .expect("worker step shell 应存在");
            let shared_schema = step
                .case(CaseTag::new(0))
                .expect("worker c0 应存在")
                .continuation_schema();
            let continuation_layout = query
                .continuation_layout(callable.continuation_object())
                .expect("continuation layout 应可查询");
            let surface_layout = query
                .surface_resume_layout(shared_schema)
                .expect("shared schema surface-resume layout 应可查询");
            let bindings = continuation_layout
                .surface_resume_bindings(shared_schema)
                .expect("object-side shared schema surface publication 应可查询");

            assert_eq!(
                surface_layout.dispatch_source_kind(),
                crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::ContinuationObjectMethod
            );
            assert_eq!(bindings.len(), 2);
            assert!(bindings.iter().any(|binding| {
                binding.case_tag() == CaseTag::new(0)
                    && binding.reachability()
                        == crate::effect_lowered::ir::LateLoweredContinuationMethodReachability::Reachable
            }));
            assert!(bindings.iter().any(|binding| {
                binding.case_tag() == CaseTag::new(1)
                    && binding.reachability()
                        == crate::effect_lowered::ir::LateLoweredContinuationMethodReachability::Unreachable
            }));
        },
    );
}

#[test]
fn refactor_llvm_surface_resume_layout_resolves_resume_site_contracts() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_resume_if_else_branch_single_perform.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, module| {
            let query = result.expect("resume fixture 应可物化 surface-resume ABI");
            let mut checked_resume_site = false;
            for callable in inputs.effect_lowered_stage_output.program().callables() {
                if !callable.has_control_body() {
                    continue;
                }
                for boundary in callable.boundary_map().entries() {
                    let Some(LateLoweredBoundaryLowering::Resume(lowering)) = boundary.lowering()
                    else {
                        continue;
                    };
                    let facts = lowering.facts();
                    let surface_layout = query
                        .surface_resume_layout(facts.continuation_schema())
                        .expect("ResumeSiteEffectFacts 所需的 surface-resume layout 应已发布");

                    assert_eq!(
                        surface_layout.continuation_schema(),
                        facts.continuation_schema()
                    );
                    assert_eq!(surface_layout.resume_tuple_ty(), facts.resume_tuple_ty());
                    assert_eq!(surface_layout.answer_ty(), facts.answer_ty());
                    assert_eq!(surface_layout.return_step_schema(), facts.out_step_schema());
                    assert_eq!(surface_layout.param_count(), 2);
                    assert!(
                        !surface_layout.resume_payload_abi().is_elided(),
                        "Int resume payload 不应被零载荷退化"
                    );
                    assert!(
                        module.get_function(surface_layout.symbol_name()).is_some(),
                        "surface-resume symbol 应被声明到 module 中"
                    );
                    assert_eq!(
                        surface_layout.dispatch_source_kind(),
                        crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::OwnerTrampolineMixed
                    );
                    checked_resume_site = true;
                }
            }
            assert!(
                checked_resume_site,
                "fixture 应至少包含一个 resume boundary"
            );
        },
    );
}

#[test]
fn refactor_llvm_surface_resume_layout_rejects_missing_published_contract() {
    with_fixture_query_result(
        "effect_refactor_dynamic_invoke_unit_payload.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let callable = program
                .callable("fixtures.build.unitWorker")
                .expect("callable 应存在");
            let continuation_objects = program
                .continuation_objects()
                .iter()
                .map(|candidate| {
                    if candidate.object_id() == callable.continuation_object() {
                        clone_continuation_object_with_surface_resumes(candidate, Vec::new())
                    } else {
                        candidate.clone()
                    }
                })
                .collect::<Vec<_>>();

            LateLoweredProgram::new(
                program.step_types().to_vec(),
                program.resume_packings().to_vec(),
                continuation_objects,
                program.callables().to_vec(),
            )
            .with_stable_instance_keys(program.stable_instance_keys().clone())
            .with_stable_instance_keys(program.stable_instance_keys().clone())
        },
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => panic!("缺失 published surface-resume contract 时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("surface-resume 发布"),
                "错误消息应指出缺失的是 surface-resume contract: {message}"
            );
            assert!(
                message.contains("owner step schema"),
                "错误消息应指出缺失 contract 所属的 owner step schema: {message}"
            );
            assert!(
                message.contains("continuation schema k"),
                "错误消息应指出缺失的 continuation schema: {message}"
            );
        },
    );
}

#[test]
fn refactor_llvm_surface_resume_dispatch_layout_resolves_object_method_target() {
    with_fixture_query_result(
        "effect_refactor_step_enum_single_case.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query = result.expect("shared schema 应可发布 owner dispatch query");
            let callable = inputs
                .abi_visibility_program
                .callable("fixtures.build.singleCaseWorker")
                .expect("singleCaseWorker callable 应存在");
            let step = inputs
                .abi_visibility_program
                .step_type(callable.step_schema())
                .expect("worker step shell 应存在");
            let shared_schema = step
                .case(CaseTag::new(0))
                .expect("worker c0 应存在")
                .continuation_schema();
            let surface_layout = query
                .surface_resume_layout(shared_schema)
                .expect("surface-resume layout 应可查询");
            let dispatch = query
                .surface_resume_dispatch_layout(shared_schema)
                .expect("owner dispatch contract 应可查询");

            assert_eq!(dispatch.continuation_schema(), shared_schema);
            assert_eq!(
                dispatch.source_kind(),
                crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::ContinuationObjectMethod
            );
            assert_eq!(dispatch.method_targets().len(), 1);

            let lookup = dispatch.method_targets()[0];
            assert_eq!(lookup.continuation_object(), callable.continuation_object());
            let continuation_layout = query
                .continuation_layout(lookup.continuation_object())
                .expect("continuation layout 应可查询");
            assert_eq!(
                continuation_layout.field_index_for_packing(lookup.packing_interface_id()),
                Some(lookup.packing_field_index())
            );
            let method_layout = query
                .surface_resume_method_layout(lookup)
                .expect("surface-resume packing method layout 应可直接查询");
            assert_eq!(lookup.vtable_index(), method_layout.vtable_index());
            assert_eq!(
                method_layout.return_step_schema(),
                surface_layout.return_step_schema()
            );

            match dispatch.target() {
                RefactorContinuationSurfaceResumeDispatchTarget::OwnerTrampoline(trampoline) => {
                    assert_eq!(
                        trampoline.owner_root_fqn(),
                        "fixtures.build.singleCaseWorker"
                    );
                    assert_eq!(
                        trampoline.owner_continuation_object(),
                        callable.continuation_object()
                    );
                    assert!(trampoline.resume_boundary_sites().is_empty());
                    assert!(trampoline.handle_binder_routes().is_empty());
                }
                RefactorContinuationSurfaceResumeDispatchTarget::Unreachable => {
                    panic!("shared schema object-method fixture 不应是 unreachable dispatch")
                }
                RefactorContinuationSurfaceResumeDispatchTarget::OwnerTrampolines(_) => {
                    panic!("shared schema object-method fixture 不应发布 multi-owner dispatch")
                }
            }
        },
    );
}

#[test]
fn refactor_llvm_surface_resume_dispatch_layout_resolves_handle_binder_owner_trampoline() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_resume_if_else_branch_single_perform.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, module| {
            let query = result.expect("handle-binder schema 应可发布 owner trampoline query");
            let callable = inputs
                .abi_visibility_program
                .callable("run")
                .expect("run callable 应存在");
            let (site_id, contract) = first_handle_dispatch(callable);
            let binder = contract
                .handled_arms()
                .iter()
                .find_map(|arm| arm.continuation_binder())
                .expect("fixture 应至少包含一个 continuation binder");
            let dispatch = query
                .surface_resume_dispatch_layout(binder.continuation_schema())
                .expect("handle-binder schema 的 owner dispatch contract 应可查询");

            assert_eq!(
                dispatch.source_kind(),
                crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::HandleContinuationBinderOnly
            );
            assert!(dispatch.method_targets().is_empty());
            match dispatch.target() {
                RefactorContinuationSurfaceResumeDispatchTarget::OwnerTrampoline(trampoline) => {
                    assert_eq!(trampoline.owner_root_fqn(), "run");
                    assert_eq!(
                        trampoline.owner_continuation_object(),
                        callable.continuation_object()
                    );
                    assert!(trampoline.resume_boundary_sites().is_empty());
                    assert_eq!(trampoline.handle_binder_routes().len(), 1);
                    assert_eq!(trampoline.handle_binder_routes()[0].site_id(), site_id);
                    assert_eq!(trampoline.handle_binder_routes()[0].arm_ordinal(), 0);
                    assert_eq!(
                        trampoline.handle_binder_routes()[0].handled_case(),
                        CaseTag::new(0)
                    );
                    assert!(module.get_function(trampoline.symbol_name()).is_some());
                }
                RefactorContinuationSurfaceResumeDispatchTarget::Unreachable => {
                    panic!("handle-binder-only schema 不应是 unreachable dispatch")
                }
                RefactorContinuationSurfaceResumeDispatchTarget::OwnerTrampolines(_) => {
                    panic!("handle-binder-only schema 不应发布 multi-owner dispatch")
                }
            }
        },
    );
}

#[test]
fn refactor_llvm_surface_resume_dispatch_layout_resolves_multi_site_resume_owner_trampoline() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, module| {
            let query = result.expect("multi-resume-site schema 应可发布 owner trampoline query");
            let callable = inputs
                .abi_visibility_program
                .callable("main")
                .expect("main callable 应存在");
            let resume_lowering = callable
                .boundary_map()
                .entries()
                .iter()
                .find_map(|boundary| match boundary.lowering() {
                    Some(LateLoweredBoundaryLowering::Resume(lowering)) => Some(lowering),
                    _ => None,
                })
                .expect("fixture 应至少包含一个 resume boundary");
            let resume_schema = resume_lowering.facts().continuation_schema();
            let handle_state = handle_dispatch_state(callable, SiteId::from_raw(1));
            let LateLoweredStateTerminator::HandleDispatch { contract, .. } =
                handle_state.terminator()
            else {
                panic!("main 顶层 handle 应保持 HandleDispatch terminator");
            };
            let binder = contract.handled_arms()[0]
                .continuation_binder()
                .expect("Ask handle arm 应发布 continuation binder");
            let dispatch = query
                .surface_resume_dispatch_layout(resume_schema)
                .expect("resume schema 的 owner dispatch contract 应可查询");

            assert_eq!(
                dispatch.source_kind(),
                crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::OwnerTrampolineMixed
            );
            assert!(dispatch.method_targets().is_empty());
            match dispatch.target() {
                RefactorContinuationSurfaceResumeDispatchTarget::OwnerTrampoline(trampoline) => {
                    let sites = trampoline
                        .resume_boundary_sites()
                        .iter()
                        .map(|site_id| site_id.as_u32())
                        .collect::<Vec<_>>();
                    assert_eq!(trampoline.owner_root_fqn(), "main");
                    assert_eq!(
                        trampoline.owner_continuation_object(),
                        callable.continuation_object()
                    );
                    assert_eq!(sites, vec![26, 31, 36, 41]);
                    assert!(!trampoline.handle_binder_routes().is_empty());
                    let projection = trampoline.wrapper_projection().expect(
                        "shared wrapper schema 应发布 owner-step -> wrapper-step projection",
                    );
                    let outward = projection
                        .outward_cases()
                        .first()
                        .expect("shared wrapper projection 应至少包含一个 outward case");
                    assert_eq!(
                        projection.underlying_route().continuation_schema(),
                        binder.continuation_schema()
                    );
                    assert!(matches!(
                        projection.underlying_route().publication(),
                        LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
                            owner_continuation_object,
                            site_id,
                            arm_ordinal,
                            handled_case,
                            ..
                        } if *owner_continuation_object == callable.continuation_object()
                            && site_id.as_u32() == 1
                            && *arm_ordinal == 0
                            && *handled_case == contract.handled_arms()[0].handled_case()
                    ));
                    assert_eq!(projection.owner_step_schema(), callable.step_schema());
                    assert_eq!(
                        projection.wrapper_step_schema(),
                        resume_lowering.facts().out_step_schema()
                    );
                    assert_eq!(
                        outward.owner_case_tag().as_u32(),
                        2,
                        "fixture 应把 owner runtime-error case 投影回 wrapper c0"
                    );
                    assert_eq!(outward.wrapper_case_tag().as_u32(), 0);
                    assert!(module.get_function(trampoline.symbol_name()).is_some());
                }
                RefactorContinuationSurfaceResumeDispatchTarget::Unreachable => {
                    panic!("resume-boundary-only schema 不应是 unreachable dispatch")
                }
                RefactorContinuationSurfaceResumeDispatchTarget::OwnerTrampolines(_) => {
                    panic!("single-owner resume schema 不应发布 multi-owner dispatch")
                }
            }
        },
    );
}

#[test]
fn refactor_llvm_surface_resume_dispatch_layout_resolves_cross_owner_wrapper_trampoline() {
    with_phase_fixture_query_result(
        "run-pass",
        "continuation_escape_binder_resume_effect_row_runtime_basic.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, module| {
            let query = result.expect("cross-owner wrapper schema 应可发布 owner dispatch query");
            let entry = inputs
                .abi_visibility_program
                .surface_resume_dispatch_inventory()
                .iter()
                .find(|entry| {
                    let Some(projection) = entry.wrapper_projection() else {
                        return false;
                    };
                    let Some((underlying_owner, underlying_object)) =
                        surface_resume_publication_owner_identity(
                            projection.underlying_route().publication(),
                        )
                    else {
                        return false;
                    };
                    entry.publications().iter().any(|publication| {
                        matches!(
                            publication,
                            LateLoweredSurfaceResumeDispatchPublication::ResumeBoundary {
                                owner_version_key,
                                owner_continuation_object,
                                ..
                            } if owner_version_key != underlying_owner
                                || *owner_continuation_object != underlying_object
                        )
                    })
                })
                .expect("fixture 应发布跨 owner 的 wrapper surface-resume schema");
            let dispatch = query
                .surface_resume_dispatch_layout(entry.continuation_schema())
                .expect("cross-owner wrapper schema 的 owner dispatch contract 应可查询");

            assert_eq!(
                dispatch.source_kind(),
                crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::OwnerTrampolineMixed
            );
            let RefactorContinuationSurfaceResumeDispatchTarget::OwnerTrampoline(trampoline) =
                dispatch.target()
            else {
                panic!("cross-owner fixture 应发布单一 underlying owner trampoline")
            };
            assert_eq!(trampoline.owner_root_fqn(), "start");
            assert!(
                trampoline.resume_boundary_sites().is_empty(),
                "跨 owner wrapper trampoline 使用 underlying handle binder，不应要求 wrapper owner 的 resume site"
            );
            assert_eq!(trampoline.handle_binder_routes().len(), 1);
            assert!(
                trampoline.wrapper_projection().is_some(),
                "跨 owner wrapper trampoline 必须携带 owner-step -> wrapper-step 投影"
            );
            assert!(module.get_function(trampoline.symbol_name()).is_some());
        },
    );
}

#[test]
fn refactor_llvm_surface_resume_dispatch_layout_resolves_multi_owner_trampolines() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_multi_escape_custom_nonresuming_direct_indirect_multi.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, module| {
            let query = result.expect("multi-owner schema 应可发布 owner dispatch query");
            let entry = inputs
                .abi_visibility_program
                .surface_resume_dispatch_inventory()
                .iter()
                .find(|entry| entry.wrapper_projections().len() >= 2)
                .expect("fixture 应发布 owner-aware wrapper projections");
            let dispatch = query
                .surface_resume_dispatch_layout(entry.continuation_schema())
                .expect("multi-owner schema 的 owner dispatch contract 应可查询");

            assert_eq!(entry.wrapper_projections().len(), 2);
            let RefactorContinuationSurfaceResumeDispatchTarget::OwnerTrampolines(targets) =
                dispatch.target()
            else {
                panic!("multi-owner schema 应发布多个 owner trampoline target");
            };
            let roots = targets
                .iter()
                .map(|target| target.owner_root_fqn().to_string())
                .collect::<BTreeSet<_>>();
            assert_eq!(
                roots,
                [
                    "run_direct_indirect_direct".to_string(),
                    "run_indirect_direct".to_string(),
                ]
                .into_iter()
                .collect::<BTreeSet<_>>()
            );
            for target in targets {
                assert!(
                    target.wrapper_projection().is_some(),
                    "每个 owner trampoline 都必须携带 owner-specific wrapper projection: {}",
                    target.owner_root_fqn()
                );
                assert!(
                    module.get_function(target.symbol_name()).is_some(),
                    "owner trampoline symbol 应声明到 module: {}",
                    target.symbol_name()
                );
            }
        },
    );
}

#[test]
fn refactor_llvm_surface_resume_dispatch_layout_rejects_missing_wrapper_projection_contract() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let callable = program.callable("main").expect("main callable 应存在");
            let resume_schema = callable
                .boundary_map()
                .entries()
                .iter()
                .find_map(|boundary| match boundary.lowering() {
                    Some(LateLoweredBoundaryLowering::Resume(lowering)) => {
                        Some(lowering.facts().continuation_schema())
                    }
                    _ => None,
                })
                .expect("fixture 应至少包含一个 resume boundary schema");
            let inventory = program
                .surface_resume_dispatch_inventory()
                .iter()
                .map(|entry| {
                    LateLoweredSurfaceResumeDispatchInventoryEntry::new(
                        entry.continuation_schema(),
                        entry.contract(),
                        entry.source_kind(),
                        entry.publications().to_vec(),
                        if entry.continuation_schema() == resume_schema {
                            None
                        } else {
                            entry.wrapper_projection().cloned()
                        },
                    )
                })
                .collect::<Vec<_>>();
            program.with_surface_resume_dispatch_inventory(inventory)
        },
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => panic!("缺失 shared wrapper projection contract 时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("owner-step -> wrapper-step projection contract"),
                "错误消息应指出缺失的是 shared wrapper projection contract: {message}"
            );
            assert!(
                message.contains("underlying route k3"),
                "错误消息应指出缺失投影所依赖的 underlying route: {message}"
            );
        },
    );
}

#[test]
fn refactor_llvm_surface_resume_wrapper_completion_resolves_payload_source() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query = result.expect("refactor ABI materialization 应成功");
            let callable = inputs
                .effect_lowered_stage_output
                .program()
                .callable("main")
                .expect("main callable 应存在");
            let resume_schema = callable
                .boundary_map()
                .entries()
                .iter()
                .find_map(|boundary| match boundary.lowering() {
                    Some(LateLoweredBoundaryLowering::Resume(lowering)) => {
                        Some(lowering.facts().continuation_schema())
                    }
                    _ => None,
                })
                .expect("fixture 应包含 shared wrapper resume schema");
            let dispatch = query
                .surface_resume_dispatch_layout(resume_schema)
                .expect("shared wrapper dispatch 应可查询");

            let trampoline = match dispatch.target() {
                RefactorContinuationSurfaceResumeDispatchTarget::OwnerTrampoline(trampoline) => {
                    trampoline.as_ref()
                }
                RefactorContinuationSurfaceResumeDispatchTarget::OwnerTrampolines(targets)
                    if targets.len() == 1 =>
                {
                    &targets[0]
                }
                RefactorContinuationSurfaceResumeDispatchTarget::OwnerTrampolines(_) => {
                    panic!("该 fixture 应只有一个 owner trampoline")
                }
                RefactorContinuationSurfaceResumeDispatchTarget::Unreachable => {
                    panic!("shared wrapper schema 应发布 owner trampoline")
                }
            };
            let projection = trampoline
                .wrapper_projection()
                .expect("shared wrapper schema 应发布 wrapper projection");

            assert_eq!(projection.complete().owner_answer_ty().as_u32(), 2);
            assert_eq!(projection.complete().wrapper_answer_ty().as_u32(), 5);
            assert!(matches!(
                projection.complete().payload_source(),
                LateLoweredSurfaceResumeWrapperCompletePayloadSource::WrapperPayload(
                    LateLoweredCompletionPayloadSource::Operand(source)
                ) if source.source_ty() == projection.complete().wrapper_answer_ty()
                    && matches!(source.value(), LateLoweredOperandValueSource::Local(_))
            ));
        },
    );
}

#[test]
fn refactor_llvm_surface_resume_wrapper_completion_uses_owner_complete_for_matching_answer_type() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_resume_if_else_branch_single_perform.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query = result.expect("refactor ABI materialization 应成功");
            let resume_schema = inputs
                .abi_visibility_program
                .surface_resume_dispatch_inventory()
                .iter()
                .find_map(|entry| {
                    let projection = entry.wrapper_projection()?;
                    (projection.complete().owner_answer_ty()
                        == projection.complete().wrapper_answer_ty())
                    .then_some(entry.continuation_schema())
                })
                .expect("fixture 应包含 owner/wrapper answer type 相同的 wrapper projection");
            let dispatch = query
                .surface_resume_dispatch_layout(resume_schema)
                .expect("shared wrapper dispatch 应可查询");

            let trampoline = match dispatch.target() {
                RefactorContinuationSurfaceResumeDispatchTarget::OwnerTrampoline(trampoline) => {
                    trampoline.as_ref()
                }
                RefactorContinuationSurfaceResumeDispatchTarget::OwnerTrampolines(targets)
                    if targets.len() == 1 =>
                {
                    &targets[0]
                }
                RefactorContinuationSurfaceResumeDispatchTarget::OwnerTrampolines(_) => {
                    panic!("该 fixture 应只有一个 owner trampoline")
                }
                RefactorContinuationSurfaceResumeDispatchTarget::Unreachable => {
                    panic!("shared wrapper schema 应发布 owner trampoline")
                }
            };
            let projection = trampoline
                .wrapper_projection()
                .expect("shared wrapper schema 应发布 wrapper projection");

            assert_eq!(
                projection.complete().owner_answer_ty(),
                projection.complete().wrapper_answer_ty()
            );
            assert!(matches!(
                projection.complete().payload_source(),
                LateLoweredSurfaceResumeWrapperCompletePayloadSource::OwnerComplete { answer_ty }
                    if *answer_ty == projection.complete().wrapper_answer_ty()
            ));
        },
    );
}

#[test]
fn refactor_llvm_surface_resume_wrapper_completion_rejects_type_drift() {
    with_phase_fixture_query_result(
        "run-pass",
        "effect_multi_escape_indirect_direct_while.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let callable = program.callable("main").expect("main callable 应存在");
            let resume_schema = callable
                .boundary_map()
                .entries()
                .iter()
                .find_map(|boundary| match boundary.lowering() {
                    Some(LateLoweredBoundaryLowering::Resume(lowering)) => {
                        Some(lowering.facts().continuation_schema())
                    }
                    _ => None,
                })
                .expect("fixture 应至少包含一个 resume boundary schema");
            let inventory = program
                .surface_resume_dispatch_inventory()
                .iter()
                .map(|entry| {
                    let wrapper_projection = if entry.continuation_schema() == resume_schema {
                        entry.wrapper_projection().map(|projection| {
                            LateLoweredSurfaceResumeWrapperProjection::new(
                                projection.underlying_route().clone(),
                                projection.owner_step_schema(),
                                projection.wrapper_step_schema(),
                                LateLoweredSurfaceResumeWrapperCompleteProjection::new(
                                    projection.complete().owner_answer_ty(),
                                    projection.complete().wrapper_answer_ty(),
                                    LateLoweredSurfaceResumeWrapperCompletePayloadSource::wrapper_payload(
                                        LateLoweredCompletionPayloadSource::unit(
                                            projection.complete().wrapper_answer_ty(),
                                        ),
                                    ),
                                ),
                                projection.outward_cases().to_vec(),
                            )
                        })
                    } else {
                        entry.wrapper_projection().cloned()
                    };
                    LateLoweredSurfaceResumeDispatchInventoryEntry::new(
                        entry.continuation_schema(),
                        entry.contract(),
                        entry.source_kind(),
                        entry.publications().to_vec(),
                        wrapper_projection,
                    )
                })
                .collect::<Vec<_>>();
            program.with_surface_resume_dispatch_inventory(inventory)
        },
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => {
                    panic!("non-Unit wrapper answer 的 Unit payload source 必须 fail fast")
                }
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("wrapper complete payload")
                    || message.contains("wrapper-step projection contract 漂移"),
                "错误消息应指出 wrapper complete payload contract 漂移: {message}"
            );
        },
    );
}

#[test]
fn refactor_llvm_surface_resume_dispatch_layout_rejects_missing_internal_method_target() {
    with_fixture_query_result(
        "effect_refactor_step_enum_single_case.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let callable = program
                .callable("fixtures.build.singleCaseWorker")
                .expect("callable 应存在");
            let continuation_objects = program
                .continuation_objects()
                .iter()
                .map(|candidate| {
                    if candidate.object_id() == callable.continuation_object() {
                        clone_continuation_object_with_methods(candidate, Vec::new())
                    } else {
                        candidate.clone()
                    }
                })
                .collect();

            LateLoweredProgram::new(
                program.step_types().to_vec(),
                program.resume_packings().to_vec(),
                continuation_objects,
                program.callables().to_vec(),
            )
            .with_stable_instance_keys(program.stable_instance_keys().clone())
        },
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => panic!("缺失 internal method target 时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("ContinuationObjectMethod"),
                "错误消息应指出 source kind 与 method target 缺失的关系: {message}"
            );
            assert!(
                message.contains("reachable internal method target"),
                "错误消息应指出缺失的是 reachable internal method target: {message}"
            );
        },
    );
}

#[test]
fn refactor_llvm_surface_resume_dispatch_layout_keeps_multi_method_lookup_set() {
    with_phase_fixture_query_result(
        "effect_facts",
        "dynamic_fallback_widening.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, module| {
            let query = result.expect("多 method 共享 schema 应可发布 owner dispatch contract");
            let callable = inputs
                .abi_visibility_program
                .callable("sample.callValue")
                .expect("sample.callValue callable 应存在");
            let step = inputs
                .abi_visibility_program
                .step_type(callable.step_schema())
                .expect("callValue step shell 应存在");
            let shared_schema = step
                .case(CaseTag::new(0))
                .expect("c0 应存在")
                .continuation_schema();
            let dispatch = query
                .surface_resume_dispatch_layout(shared_schema)
                .expect("多 method 共享 schema 的 dispatch contract 应可查询");
            let method_keys = dispatch
                .method_targets()
                .iter()
                .map(|lookup| {
                    (
                        lookup.packing_interface_id().as_u32(),
                        lookup.case_tag().as_u32(),
                    )
                })
                .collect::<Vec<_>>();

            assert_eq!(
                dispatch.source_kind(),
                crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::ContinuationObjectMethod
            );
            assert_eq!(method_keys, vec![(0, 0), (1, 1)]);
            match dispatch.target() {
                RefactorContinuationSurfaceResumeDispatchTarget::OwnerTrampoline(trampoline) => {
                    assert_eq!(trampoline.owner_root_fqn(), "sample.callValue");
                    assert_eq!(
                        trampoline.owner_continuation_object(),
                        callable.continuation_object()
                    );
                    assert!(module.get_function(trampoline.symbol_name()).is_some());
                }
                RefactorContinuationSurfaceResumeDispatchTarget::Unreachable => {
                    panic!("多 method 共享 schema 不应是 unreachable dispatch")
                }
                RefactorContinuationSurfaceResumeDispatchTarget::OwnerTrampolines(_) => {
                    panic!("多 method 单 owner schema 不应发布 multi-owner dispatch")
                }
            }
        },
    );
}

#[test]
fn refactor_llvm_surface_resume_dispatch_layout_rejects_multi_object_publication() {
    with_fixture_query_result(
        "effect_refactor_step_enum_single_case.scoop",
        |inputs| {
            let program = &inputs.abi_visibility_program;
            let callable = program
                .callable("fixtures.build.singleCaseWorker")
                .expect("callable 应存在");
            let next_object_id = ContinuationObjectId::new(
                program
                    .continuation_objects()
                    .iter()
                    .map(|object| object.object_id().as_u32())
                    .max()
                    .map(|raw| raw.saturating_add(1))
                    .unwrap_or(0),
            );
            let duplicated_object = program
                .continuation_object(callable.continuation_object())
                .map(|object| clone_continuation_object_with_id(object, next_object_id))
                .expect("continuation object 应存在");
            let mut continuation_objects = program.continuation_objects().to_vec();
            continuation_objects.push(duplicated_object);

            LateLoweredProgram::new(
                program.step_types().to_vec(),
                program.resume_packings().to_vec(),
                continuation_objects,
                program.callables().to_vec(),
            )
        },
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => panic!("多 object 共享同一 schema 时必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("多个 continuation object 共享同一 schema"),
                "错误消息应指出 multi-object publication 歧义: {message}"
            );
        },
    );
}

#[test]
fn refactor_llvm_layout_binds_pure_direct_entries_without_hir_typestore_fallback() {
    with_fixture_query(
        "effect_refactor_dynamic_entry_publication_emit_llvm.scoop",
        |inputs, query, module| {
            let lambda_root = inputs
                .abi_visibility_program
                .callables()
                .iter()
                .find(|callable| callable.root_fqn().contains("$lambda"))
                .map(|callable| callable.root_fqn().to_string())
                .expect("fixture 应发布 lambda callable shell");
            let roots = vec![
                "fixtures.build.makeClosure".to_string(),
                "fixtures.build.Base.ping".to_string(),
                lambda_root,
            ];

            for root in roots {
                let callable = query
                    .plain_callable_layout_by_root_fqn(&root)
                    .expect("plain callable layout 应存在");
                assert_eq!(
                    callable.direct_entry().param_count(),
                    callable.direct_entry().param_tys().len(),
                    "plain direct entry 形参个数必须来自 P5 plain ABI handoff: {root}"
                );
                assert!(
                    module
                        .get_function(callable.direct_entry().symbol_name())
                        .is_some(),
                    "plain direct entry 应声明普通 LLVM callable symbol: {root}"
                );
                assert!(
                    query.callable_layout_by_root_fqn(&root).is_err(),
                    "NoOutward plain callable 不应发布 effect-step callable layout: {root}"
                );
            }
        },
    );
}

#[test]
fn refactor_llvm_layout_resolves_unit_case_payload_contract() {
    with_fixture_query(
        "effect_refactor_dynamic_invoke_unit_payload.scoop",
        |inputs, query, _module| {
            let callable = inputs
                .effect_lowered_stage_output
                .program()
                .callable("fixtures.build.unitWorker")
                .expect("unitWorker callable 应存在");
            let step_layout = query
                .step_layout(callable.step_schema())
                .expect("step layout 应存在");
            let case_variant = step_layout
                .case_layout(CaseTag::new(0))
                .expect("case0 layout 应存在")
                .variant();
            let case_payload_layout = query
                .source_value_layout(case_variant.payload_source_ty())
                .expect("case payload source type 应发布 source-type ABI contract");
            let complete_layout = query
                .source_value_layout(step_layout.complete_variant().payload_source_ty())
                .expect("complete payload source type 应发布 source-type ABI contract");

            assert_eq!(
                case_payload_layout.kind(),
                RefactorSourceAbiLayoutKind::Scalar
            );
            assert!(case_payload_layout.abi().is_elided());
            assert!(case_payload_layout.fields().is_empty());
            assert!(case_variant.payload_is_elided());
            assert_eq!(case_variant.payload_field_count(), 1);
            assert!(complete_layout.abi().is_elided());
            assert_eq!(step_layout.complete_variant().payload_field_count(), 0);
        },
    );
}

#[test]
fn refactor_llvm_layout_resolves_tuple_resume_payload_and_answer_contract() {
    with_phase_fixture_query_result(
        "run-pass",
        "continuation_resume_surface_named_tuple_and_unit_basic.scoop",
        |inputs| inputs.abi_visibility_program.clone(),
        |inputs, result, _module| {
            let query = result.expect("tuple resume fixture 应可发布 source-type ABI contract");
            let pair_surface = inputs
                .abi_visibility_program
                .continuation_objects()
                .iter()
                .flat_map(|object| object.surface_resumes().iter())
                .find(|surface| {
                    inputs
                        .effect_lowered_stage_output
                        .types()
                        .display(surface.resume_tuple_ty())
                        .to_string()
                        == "(Int, String)"
                })
                .expect("fixture 应包含 tuple resume surface");
            let surface_layout = query
                .surface_resume_layout(pair_surface.continuation_schema())
                .expect("surface-resume layout 应可查询");
            let resume_payload_layout = query
                .source_value_layout(surface_layout.resume_tuple_ty())
                .expect("resume tuple source type 应发布 source-type ABI contract");
            let answer_layout = query
                .source_value_layout(surface_layout.answer_ty())
                .expect("resume answer source type 应发布 source-type ABI contract");

            assert_eq!(
                resume_payload_layout.kind(),
                RefactorSourceAbiLayoutKind::Tuple
            );
            assert_eq!(resume_payload_layout.fields().len(), 2);
            assert_eq!(resume_payload_layout.abi_field_count(), 2);
            assert_eq!(resume_payload_layout.fields()[0].source_index(), 0);
            assert_eq!(resume_payload_layout.fields()[0].abi_field_index(), Some(0));
            assert_eq!(resume_payload_layout.fields()[1].source_index(), 1);
            assert_eq!(resume_payload_layout.fields()[1].abi_field_index(), Some(1));
            assert!(!resume_payload_layout.fields()[0].is_elided());
            assert!(!resume_payload_layout.fields()[1].is_elided());
            assert_eq!(answer_layout.kind(), RefactorSourceAbiLayoutKind::Scalar);
            assert!(answer_layout.abi().is_elided());
        },
    );
}

#[test]
fn refactor_llvm_layout_rejects_unlowerable_invoke_args_type() {
    let inputs = build_fixture_inputs("effect_refactor_step_enum_single_case.scoop");
    let mut source_types = inputs.effect_lowered_stage_output.types().clone();
    let param_ty = source_types.ty_param(TypeParamType {
        name: "SyntheticInvokeArgs".to_string(),
        decl_file: std::path::PathBuf::from("tests/p6_t02i.synthetic"),
        decl_span: dummy_span(),
    });

    with_inputs_query_result_for_source_types(
        inputs,
        move |inputs| {
            let program = &inputs.abi_visibility_program;
            let callables = program
                .callables()
                .iter()
                .map(|candidate| {
                    if candidate.root_fqn() == "fixtures.build.singleCaseWorker" {
                        clone_callable_with_dynamic_invoke_entry(
                            candidate,
                            LateLoweredDynamicInvokeEntry::new(
                                param_ty,
                                candidate.dynamic_invoke_entry().step_schema(),
                                candidate.dynamic_invoke_entry().entry_state(),
                                candidate.dynamic_invoke_entry().complete_state(),
                            ),
                        )
                    } else {
                        candidate.clone()
                    }
                })
                .collect();
            LateLoweredProgram::new(
                program.step_types().to_vec(),
                program.resume_packings().to_vec(),
                program.continuation_objects().to_vec(),
                callables,
            )
            .with_stable_instance_keys(program.stable_instance_keys().clone())
        },
        move |_inputs| source_types,
        |_inputs, result, _module| {
            let err = match result {
                Ok(_) => panic!("不可 lowering 的 synthetic invoke args type 必须 fail fast"),
                Err(err) => err,
            };
            let message = err.to_string();
            assert!(
                message.contains("source-type ABI value lowering"),
                "错误消息应指出缺失的是 source-type ABI lowering contract: {message}"
            );
            assert!(
                message.contains("SyntheticInvokeArgs"),
                "错误消息应指出不可 lowering 的 synthetic source type: {message}"
            );
            assert!(
                message.contains("尚未实例化的类型参数"),
                "错误消息应明确拒绝未实例化类型参数: {message}"
            );
        },
    );
}

#[test]
fn refactor_llvm_unit_abi_elides_zero_sized_args_and_resume_payloads() {
    with_fixture_query_result(
        "effect_refactor_dynamic_invoke_unit_payload.scoop",
        unit_worker_program_with_ping_interface,
        |inputs, result, module| {
            let query = result.expect("published unit resume packing 应可物化 ABI");
            let callable = inputs
                .effect_lowered_stage_output
                .program()
                .callable("fixtures.build.unitWorker")
                .expect("callable 应存在");
            let callable_layout = query
                .callable_layout(callable.step_schema())
                .expect("callable layout 应可查询");
            let step_layout = query
                .step_layout(callable.step_schema())
                .expect("step layout 应可查询");
            let continuation_object = inputs
                .effect_lowered_stage_output
                .program()
                .continuation_object(callable.continuation_object())
                .expect("continuation object 应存在");
            let interface_id = *query
                .callable_layout(callable.step_schema())
                .expect("callable layout 应可查询")
                .resume_packings()
                .iter()
                .find(|interface_id| {
                    query
                        .resume_packing_layout(**interface_id)
                        .is_some_and(|interface| {
                            interface.packing_family_fqn() == "fixtures.build.Ping"
                        })
                })
                .expect("应存在 Ping resume packing");
            let interface_layout = query
                .resume_packing_layout(interface_id)
                .expect("resume packing layout 应可查询");
            let method_layout = interface_layout
                .method(CaseTag::new(0))
                .expect("case0 method 应存在");
            let surface_resume_schema = continuation_object
                .surface_resumes()
                .iter()
                .find(|surface| surface.case_tag() == CaseTag::new(0))
                .expect("case0 surface resume 应存在")
                .continuation_schema();
            let surface_layout = query
                .surface_resume_layout(surface_resume_schema)
                .expect("surface-resume layout 应可查询");

            assert!(callable_layout.dynamic_entry().args_abi().is_elided());
            assert!(callable_layout.direct_entry().args_abi().is_elided());
            assert_eq!(callable_layout.dynamic_entry().param_count(), 0);
            assert_eq!(callable_layout.direct_entry().param_count(), 0);
            assert!(step_layout.complete_variant().payload_is_elided());
            assert_eq!(step_layout.complete_variant().payload_field_count(), 0);
            assert!(method_layout.resume_payload_abi().is_elided());
            assert_eq!(method_layout.param_count(), 1);
            assert!(surface_layout.resume_payload_abi().is_elided());
            assert_eq!(surface_layout.param_count(), 1);
            assert_eq!(
                step_layout
                    .case_layout(CaseTag::new(0))
                    .expect("case0 layout 应存在")
                    .variant()
                    .payload_field_count(),
                1
            );
            assert!(
                module
                    .get_function(callable_layout.dynamic_entry().symbol_name())
                    .is_some()
            );
            assert!(module.get_function(surface_layout.symbol_name()).is_some());
        },
    );
}
