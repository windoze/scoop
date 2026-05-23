use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use super::surface_resume::surface_resume_publication_owner_identity;
use super::*;
use crate::effect_facts::{CallSiteTarget, CallTargetMode, CaseTag, ImplPlan};
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
    LateLoweredOptOptions, LateLoweredProgramBuilder, run_lir_opt_pipeline,
};
use crate::llvm::codegen::effect_lowered::types::CallTargetQuery;
use crate::llvm::codegen::{
    CompilationUnitCodegenCx, CompilationUnitCodegenInputs, EffectOpTagState, MainCodegen,
};
use crate::llvm::target;
use crate::mir::{LoweredMir, MirLoweringFacts, SiteId, lower_hir_file_for_dump_with_facts};
use crate::pipeline::{
    DirectStyleMirStageOutput, LirStageOutput, LlvmStageBaseContext,
    build_effect_facts_stage_output, build_lir_stage_output_from_stage_outputs,
    load_hir_stage_output_for_dump,
};
use crate::session::{Session, SessionOptions};
use crate::source::{SourceFile, SourceMap};
use crate::ty::{TypeParamType, TypeStore};
use inkwell::context::Context;
use scoopc_lir_facts::{LirCallSiteContract, LirCallableContract};

struct FixtureAbiInputs {
    source_map: SourceMap,
    entry_source_id: crate::source::SourceId,
    base_context: LlvmStageBaseContext,
    lir_stage_output: LirStageOutput,
    abi_visibility_program: LateLoweredProgram,
    abi_visibility_lir_facts: scoopc_lir_facts::LirFacts,
}

impl FixtureAbiInputs {
    fn primary_types(&self) -> &TypeStore {
        self.base_context.types()
    }
}

fn session() -> Session {
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
    let session = session();
    let typed_hir_output =
        load_hir_stage_output_for_dump(&session, &source).expect("HIR stage 应成功");
    let hir_facts = typed_hir_output.hir_facts().clone();
    let source_side_tables = typed_hir_output.lowered_hir().clone();
    let facts = MirLoweringFacts::from_hir_facts(
        typed_hir_output.lowered_hir(),
        typed_hir_output.hir_facts(),
    );
    let mut lowered_hir = typed_hir_output.into_lowered_hir();
    let stable_cone_key = lowered_hir.stable_cone_key.clone();
    let builtins = lowered_hir.types.intern_builtins();
    let file = lower_hir_file_for_dump_with_facts(
        builtins,
        &mut lowered_hir.types,
        &lowered_hir.file,
        &lowered_hir.member_funs,
        &facts,
    );
    let types = std::mem::replace(&mut lowered_hir.types, TypeStore::new());
    let materialized_mir =
        crate::pipeline::materialize_direct_style_mir_for_dump(&session, &source)
            .expect("materialized MIR 应成功");
    let mir_stage_output = DirectStyleMirStageOutput::new(
        LoweredMir { file, types },
        stable_cone_key,
        &lowered_hir.source_cones,
        &lowered_hir.source_cone_order,
    )
    .with_materialized_mir(materialized_mir);
    let effect_facts_stage_output =
        build_effect_facts_stage_output(&session, &source, &mir_stage_output)
            .expect("effect facts stage 应成功");
    // ABI materializer 必须消费与真实 LLVM stage 相同的 shell-preserving handoff，
    // 不能误用会裁剪 published resume methods 的 authoritative reachable-body program。
    let (abi_visibility_program, abi_visibility_opt_pipeline) = run_lir_opt_pipeline(
        LateLoweredProgramBuilder::from_canonical_inputs(
            mir_stage_output.materialized_pass_view(),
            effect_facts_stage_output.effect_facts(),
            effect_facts_stage_output.effect_facts().types(),
            mir_stage_output.mir_facts(),
        )
        .build()
        .expect("ABI visibility late-lowered program 应成功"),
        LateLoweredOptOptions::preserve_published_resume_shells(),
    )
    .expect("ABI visibility LIR opt pipeline 应成功")
    .into_parts();
    let abi_visibility_lir_facts = crate::pipeline::lir_facts_builder::build_lir_facts(
        &abi_visibility_program,
        mir_stage_output.mir_facts(),
        mir_stage_output.materialized_mir(),
        effect_facts_stage_output.effect_facts(),
        mir_stage_output.materialized_mir().opt_level(),
        abi_visibility_opt_pipeline,
    )
    .expect("ABI visibility LIR facts 应成功");
    let lir_stage_output = build_lir_stage_output_from_stage_outputs(
        &mir_stage_output,
        &effect_facts_stage_output,
        LateLoweredOptOptions::default(),
    )
    .expect("effect lowered stage 应成功");
    let (_direct_style, materialized_mir) = mir_stage_output.into_parts();
    let base_context = LlvmStageBaseContext::from_lowered_hir(
        source_side_tables,
        hir_facts,
        materialized_mir,
        effect_facts_stage_output.into_effect_facts(),
    );
    let input_sources = vec![source.clone()];
    let (source_map, entry_source_id) =
        crate::llvm::frontend::build_source_map_with_extra_sources(&session, &input_sources, 0);
    FixtureAbiInputs {
        source_map,
        entry_source_id,
        base_context,
        lir_stage_output,
        abi_visibility_program,
        abi_visibility_lir_facts,
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
    let module = context.create_module("abi_test");
    let builder = context.create_builder();
    let target_info = target::configure_module_for_host(&module).expect("host target 应可配置");
    let target_data = inkwell::targets::TargetData::create(&target_info.data_layout);
    let base = &inputs.base_context;
    let fun_index = base.fun_index();
    let hir_facts = Rc::new(base.hir_facts().clone());
    let effect_op_tags = Rc::new(RefCell::new(EffectOpTagState::new()));
    let empty_enum_layouts = crate::hir::EnumLayoutIndex::default();
    let empty_class_inits = crate::hir::ClassInitIndex::default();
    let empty_class_vtables = crate::vtable::ClassVtableIndex::default();
    let empty_interfaces = crate::itable::InterfaceIndex::default();
    let empty_class_itables = crate::itable::ClassItableIndex::default();
    let unit_codegen = CompilationUnitCodegenCx::new(CompilationUnitCodegenInputs {
        context: &context,
        module: &module,
        builder: &builder,
        target_data: &target_data,
        host: &target_info,
        source_map: &inputs.source_map,
        entry_source_id: inputs.entry_source_id,
        stable_cone_key: base.stable_cone_key(),
        source_cones: base.source_cones(),
        stable_type_param_keys: base.stable_type_param_keys(),
        types: base.types(),
        struct_layouts: base.struct_layouts(),
        enum_layouts: &empty_enum_layouts,
        top_level_vars: base.top_level_vars(),
        top_level_immutable_values: base.top_level_immutable_values(),
        top_level_fun_call_sites: base.top_level_fun_call_sites(),
        object_inits: base.object_inits(),
        class_inits: &empty_class_inits,
        class_ctor_init_bodies: base.class_ctor_init_bodies(),
        class_vtables: &empty_class_vtables,
        interfaces: &empty_interfaces,
        class_itables: &empty_class_itables,
        ctor_call_sites: base.ctor_call_sites(),
        dispatch_call_sites: base.dispatch_call_sites(),
        effect_op_call_sites: base.effect_op_call_sites(),
        continuation_resume_call_sites: base.continuation_resume_call_sites(),
        when_pat_binding_tys: base.when_pat_binding_tys(),
        nominal_kinds: base.nominal_kinds(),
        direct_supertypes: base.direct_supertypes(),
        builtins: base.builtins(),
        callable_sources: base.callable_sources(),
        callable_signatures: base.callable_signatures(),
        extern_funs: base.extern_funs(),
        native_callable_funs: base.native_callable_funs(),
        fun_index: &fun_index,
        published_late_lowered_program: Some(&program),
        published_late_lowered_types: Some(inputs.primary_types()),
        published_lir_facts: inputs.lir_stage_output.lir_facts(),
        hir_facts,
        effect_op_tags,
    });
    let mut codegen = unit_codegen.fresh_main_codegen();
    let result = codegen.materialize_program_abi(
        &program,
        &inputs.abi_visibility_lir_facts,
        inputs.primary_types(),
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
    let module = context.create_module("abi_test");
    let builder = context.create_builder();
    let target_info = target::configure_module_for_host(&module).expect("host target 应可配置");
    let target_data = inkwell::targets::TargetData::create(&target_info.data_layout);
    let base = &inputs.base_context;
    let fun_index = base.fun_index();
    let hir_facts = Rc::new(base.hir_facts().clone());
    let effect_op_tags = Rc::new(RefCell::new(EffectOpTagState::new()));
    let empty_enum_layouts = crate::hir::EnumLayoutIndex::default();
    let empty_class_inits = crate::hir::ClassInitIndex::default();
    let empty_class_vtables = crate::vtable::ClassVtableIndex::default();
    let empty_interfaces = crate::itable::InterfaceIndex::default();
    let empty_class_itables = crate::itable::ClassItableIndex::default();
    let unit_codegen = CompilationUnitCodegenCx::new(CompilationUnitCodegenInputs {
        context: &context,
        module: &module,
        builder: &builder,
        target_data: &target_data,
        host: &target_info,
        source_map: &inputs.source_map,
        entry_source_id: inputs.entry_source_id,
        stable_cone_key: base.stable_cone_key(),
        source_cones: base.source_cones(),
        stable_type_param_keys: base.stable_type_param_keys(),
        types: base.types(),
        struct_layouts: base.struct_layouts(),
        enum_layouts: &empty_enum_layouts,
        top_level_vars: base.top_level_vars(),
        top_level_immutable_values: base.top_level_immutable_values(),
        top_level_fun_call_sites: base.top_level_fun_call_sites(),
        object_inits: base.object_inits(),
        class_inits: &empty_class_inits,
        class_ctor_init_bodies: base.class_ctor_init_bodies(),
        class_vtables: &empty_class_vtables,
        interfaces: &empty_interfaces,
        class_itables: &empty_class_itables,
        ctor_call_sites: base.ctor_call_sites(),
        dispatch_call_sites: base.dispatch_call_sites(),
        effect_op_call_sites: base.effect_op_call_sites(),
        continuation_resume_call_sites: base.continuation_resume_call_sites(),
        when_pat_binding_tys: base.when_pat_binding_tys(),
        nominal_kinds: base.nominal_kinds(),
        direct_supertypes: base.direct_supertypes(),
        builtins: base.builtins(),
        callable_sources: base.callable_sources(),
        callable_signatures: base.callable_signatures(),
        extern_funs: base.extern_funs(),
        native_callable_funs: base.native_callable_funs(),
        fun_index: &fun_index,
        published_late_lowered_program: Some(&program),
        published_late_lowered_types: Some(&source_types),
        published_lir_facts: inputs.lir_stage_output.lir_facts(),
        hir_facts,
        effect_op_tags,
    });
    let mut codegen = unit_codegen.fresh_main_codegen();
    let result =
        codegen.materialize_program_abi(&program, &inputs.abi_visibility_lir_facts, &source_types);
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
    let module = context.create_module("abi_test");
    let builder = context.create_builder();
    let target_info = target::configure_module_for_host(&module).expect("host target 应可配置");
    let target_data = inkwell::targets::TargetData::create(&target_info.data_layout);
    let base = &inputs.base_context;
    let fun_index = base.fun_index();
    let hir_facts = Rc::new(base.hir_facts().clone());
    let effect_op_tags = Rc::new(RefCell::new(EffectOpTagState::new()));
    let empty_enum_layouts = crate::hir::EnumLayoutIndex::default();
    let empty_class_inits = crate::hir::ClassInitIndex::default();
    let empty_class_vtables = crate::vtable::ClassVtableIndex::default();
    let empty_interfaces = crate::itable::InterfaceIndex::default();
    let empty_class_itables = crate::itable::ClassItableIndex::default();
    let unit_codegen = CompilationUnitCodegenCx::new(CompilationUnitCodegenInputs {
        context: &context,
        module: &module,
        builder: &builder,
        target_data: &target_data,
        host: &target_info,
        source_map: &inputs.source_map,
        entry_source_id: inputs.entry_source_id,
        stable_cone_key: base.stable_cone_key(),
        source_cones: base.source_cones(),
        stable_type_param_keys: base.stable_type_param_keys(),
        types: base.types(),
        struct_layouts: base.struct_layouts(),
        enum_layouts: &empty_enum_layouts,
        top_level_vars: base.top_level_vars(),
        top_level_immutable_values: base.top_level_immutable_values(),
        top_level_fun_call_sites: base.top_level_fun_call_sites(),
        object_inits: base.object_inits(),
        class_inits: &empty_class_inits,
        class_ctor_init_bodies: base.class_ctor_init_bodies(),
        class_vtables: &empty_class_vtables,
        interfaces: &empty_interfaces,
        class_itables: &empty_class_itables,
        ctor_call_sites: base.ctor_call_sites(),
        dispatch_call_sites: base.dispatch_call_sites(),
        effect_op_call_sites: base.effect_op_call_sites(),
        continuation_resume_call_sites: base.continuation_resume_call_sites(),
        when_pat_binding_tys: base.when_pat_binding_tys(),
        nominal_kinds: base.nominal_kinds(),
        direct_supertypes: base.direct_supertypes(),
        builtins: base.builtins(),
        callable_sources: base.callable_sources(),
        callable_signatures: base.callable_signatures(),
        extern_funs: base.extern_funs(),
        native_callable_funs: base.native_callable_funs(),
        fun_index: &fun_index,
        published_late_lowered_program: Some(&program),
        published_late_lowered_types: Some(inputs.primary_types()),
        published_lir_facts: inputs.lir_stage_output.lir_facts(),
        hir_facts,
        effect_op_tags,
    });
    let mut codegen = unit_codegen.fresh_main_codegen();
    let result = codegen.materialize_program_abi(
        &program,
        &inputs.abi_visibility_lir_facts,
        inputs.primary_types(),
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
            let query = result.expect("ABI materialization 应成功");
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
) -> (crate::mir::SiteId, LirCallSiteContract) {
    let owner = inputs
        .abi_visibility_lir_facts
        .callables
        .values()
        .find(|facts| facts.root_fqn() == callable.root_fqn())
        .expect("callable LIR facts 应存在");
    let LirCallableContract::Plain(plain) = &owner.contract else {
        panic!("non-boundary source-slice helper 只支持 plain callable facts");
    };
    plain
        .call_sites
        .iter()
        .find_map(|site| {
            (site.contract.target_mode != scoopc_lir_facts::LirCallTargetMode::KnownInstance)
                .then(|| (site.site_id, site.contract.clone()))
        })
        .expect("应找到一个 non-boundary source-slice dynamic call site")
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

mod abi_layout;
mod classification;
mod dispatch;
mod handle_dispatch;
mod surface_resume;

#[allow(unused_imports)]
use {abi_layout::*, classification::*, dispatch::*, handle_dispatch::*, surface_resume::*};
