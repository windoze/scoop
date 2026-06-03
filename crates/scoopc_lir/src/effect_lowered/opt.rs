use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::effect_facts::CaseTag;
use scoopc_lir_facts::{
    LIR_OPT_PIPELINE_REVISION, LirOptPassFacts, LirOptPassKind, LirOptPassStatus,
    LirOptPipelineFacts,
};

use super::ir::{
    BoundaryId, ContinuationObjectId, FrameSlotId, LateLoweredBoundary,
    LateLoweredBoundaryLowering, LateLoweredBoundaryMap, LateLoweredCallBoundaryLowering,
    LateLoweredCallable, LateLoweredCompleteStepDispatch, LateLoweredContinuationCapture,
    LateLoweredContinuationMethod, LateLoweredContinuationMethodReachability,
    LateLoweredContinuationObject, LateLoweredDynamicInvokeEntry, LateLoweredFrameSchema,
    LateLoweredFrameSlot, LateLoweredFrameSlotKind, LateLoweredHandleBoundaryLowering,
    LateLoweredPerformBoundaryLowering, LateLoweredPlainCallable,
    LateLoweredPlainLocalEffectControl, LateLoweredProgram, LateLoweredResumeBoundaryLowering,
    LateLoweredResumeInterface, LateLoweredResumePayloadBinding, LateLoweredResumeState,
    LateLoweredResumeStateMap, LateLoweredRuntimeErrorBoundaryLowering, LateLoweredState,
    LateLoweredStateGraph, LateLoweredStateRole, LateLoweredStateTerminator,
    LateLoweredStepDispatchPlan, ResumeInterfaceId, StateId,
};
use super::opt_verify::{LirOptVerifyError, verify_post_opt_program};

#[derive(Debug, Clone, Copy, Default)]
pub struct LateLoweredOptOptions {
    pub(crate) preserve_published_resume_shells: bool,
}

impl LateLoweredOptOptions {
    #[cfg_attr(not(feature = "llvm"), allow(dead_code))]
    pub const fn preserve_published_resume_shells() -> Self {
        Self {
            preserve_published_resume_shells: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LirOptPipelineOutput {
    program: LateLoweredProgram,
    opt_pipeline: LirOptPipelineFacts,
}

impl LirOptPipelineOutput {
    pub fn into_parts(self) -> (LateLoweredProgram, LirOptPipelineFacts) {
        (self.program, self.opt_pipeline)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct LirOptStats {
    redirected_states: usize,
    removed_states: usize,
    removed_boundaries: usize,
    removed_frame_slots: usize,
    rewritten_dynamic_entries: usize,
    pruned_resume_packings: usize,
    pruned_resume_methods: usize,
}

impl LirOptStats {
    fn local_state_machine_changed(self) -> bool {
        self.redirected_states > 0 || self.removed_states > 0
    }

    fn wrapper_state_folding_changed(self) -> bool {
        self.redirected_states > 0
    }

    fn dynamic_invoke_rewrite_changed(self) -> bool {
        self.rewritten_dynamic_entries > 0
    }

    fn dead_cleanup_changed(self) -> bool {
        self.removed_states > 0 || self.removed_boundaries > 0 || self.removed_frame_slots > 0
    }

    fn resume_packing_pruning_changed(self) -> bool {
        self.pruned_resume_packings > 0 || self.pruned_resume_methods > 0
    }
}

/// 在 late-lowered IR 上执行窄的 post-lowering 收缩。
///
/// 该 pass 只消费 `LateLoweredProgram`：
/// - 不重新读取 HIR/P3 MIR/P4 solver 结果；
/// - 不改动 `StepSchema` / `CaseTag` / `ImplPlan` / canonical dynamic invoke contract；
/// - 只做 wrapper state 折叠、internal resume interface 去虚化，以及死代码/死 slot 清理。
#[cfg_attr(any(not(test), feature = "standalone-stage-crate"), allow(dead_code))]
pub(crate) fn optimize_program(program: LateLoweredProgram) -> LateLoweredProgram {
    optimize_program_with_options(program, LateLoweredOptOptions::default())
}

#[cfg_attr(any(not(test), feature = "standalone-stage-crate"), allow(dead_code))]
pub(crate) fn optimize_program_with_options(
    program: LateLoweredProgram,
    options: LateLoweredOptOptions,
) -> LateLoweredProgram {
    run_lir_opt_pipeline(program, options)
        .expect("LIR opt verifier should accept internally rewritten LIR")
        .into_parts()
        .0
}

pub fn run_lir_opt_pipeline(
    program: LateLoweredProgram,
    options: LateLoweredOptOptions,
) -> Result<LirOptPipelineOutput, LirOptVerifyError> {
    let (program, stats) = optimize_program_body(program, options);
    verify_post_opt_program(&program)?;
    let opt_pipeline = build_opt_pipeline_facts(options, stats);
    Ok(LirOptPipelineOutput {
        program,
        opt_pipeline,
    })
}

fn optimize_program_body(
    program: LateLoweredProgram,
    options: LateLoweredOptOptions,
) -> (LateLoweredProgram, LirOptStats) {
    let mut stats = LirOptStats::default();
    let mut optimized_objects =
        BTreeMap::<ContinuationObjectId, LateLoweredContinuationObject>::new();
    let mut optimized_callables = Vec::with_capacity(program.len());

    for callable in program.callables() {
        if !callable.has_control_body() {
            optimized_callables.push(callable.clone());
            continue;
        }
        let continuation_object = program
            .continuation_object(callable.continuation_object())
            .expect("every callable should point at a published continuation object");
        let optimized = if callable.effect_step_abi().is_some() {
            optimize_callable(callable, continuation_object, options)
        } else {
            optimize_plain_callable(callable, continuation_object, options)
        };
        accumulate_callable_stats(
            callable,
            &optimized.callable,
            continuation_object,
            &optimized,
            &mut stats,
        );
        optimized_objects.insert(
            optimized.continuation_object.object_id(),
            optimized.continuation_object,
        );
        optimized_callables.push(optimized.callable);
    }

    if options.preserve_published_resume_shells {
        let continuation_objects = program
            .continuation_objects()
            .iter()
            .filter_map(|object| optimized_objects.remove(&object.object_id()))
            .collect::<Vec<_>>();
        let optimized_program = LateLoweredProgram::new(
            program.step_types().to_vec(),
            program.resume_packings().to_vec(),
            continuation_objects,
            optimized_callables,
        )
        .with_class_ctor_init_bodies(program.class_ctor_init_bodies().cloned().collect())
        .with_source_class_ctor_calls(program.source_class_ctor_calls().to_vec())
        .with_stable_instance_keys(program.stable_instance_keys().clone())
        .with_dump_metadata(
            program.dump_type_texts().clone(),
            program.dump_body_labels_map().clone(),
        );
        return (optimized_program, stats);
    }

    let live_methods_by_interface = collect_live_methods_by_interface(optimized_objects.values());
    let live_interface_ids = live_methods_by_interface
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();

    let resume_packings = program
        .resume_packings()
        .iter()
        .filter_map(|interface| {
            prune_resume_interface(
                interface,
                live_methods_by_interface.get(&interface.interface_id()),
            )
        })
        .collect::<Vec<_>>();
    stats.pruned_resume_packings = program
        .resume_packings()
        .len()
        .saturating_sub(resume_packings.len());

    let continuation_objects = program
        .continuation_objects()
        .iter()
        .filter_map(|object| {
            optimized_objects
                .remove(&object.object_id())
                .map(|optimized| prune_object_interfaces(optimized, &live_interface_ids))
        })
        .collect::<Vec<_>>();
    let implemented_packings_by_object = continuation_objects
        .iter()
        .map(|object| (object.object_id(), object.implemented_packings().to_vec()))
        .collect::<BTreeMap<_, _>>();

    let callables = optimized_callables
        .into_iter()
        .map(|callable| {
            if !callable.has_control_body() {
                return callable;
            }
            let resume_packings = implemented_packings_by_object
                .get(&callable.continuation_object())
                .cloned()
                .unwrap_or_default();
            with_callable_resume_packings(callable, resume_packings)
        })
        .collect::<Vec<_>>();

    let optimized_program = LateLoweredProgram::new(
        program.step_types().to_vec(),
        resume_packings,
        continuation_objects,
        callables,
    )
    .with_class_ctor_init_bodies(program.class_ctor_init_bodies().cloned().collect())
    .with_source_class_ctor_calls(program.source_class_ctor_calls().to_vec())
    .with_stable_instance_keys(program.stable_instance_keys().clone())
    .with_dump_metadata(
        program.dump_type_texts().clone(),
        program.dump_body_labels_map().clone(),
    );
    (optimized_program, stats)
}

fn accumulate_callable_stats(
    before: &LateLoweredCallable,
    after: &LateLoweredCallable,
    before_object: &LateLoweredContinuationObject,
    optimized: &OptimizedCallable,
    stats: &mut LirOptStats,
) {
    stats.redirected_states += collect_state_redirects(before.state_graph()).len();
    stats.removed_states += before
        .state_graph()
        .states()
        .len()
        .saturating_sub(after.state_graph().states().len());
    stats.removed_boundaries += before
        .boundary_map()
        .entries()
        .len()
        .saturating_sub(after.boundary_map().entries().len());
    stats.removed_frame_slots += before
        .frame_schema()
        .slots()
        .len()
        .saturating_sub(after.frame_schema().slots().len());
    if let (Some(before_effect), Some(after_effect)) =
        (before.effect_step_abi(), after.effect_step_abi())
        && before_effect.dynamic_invoke_entry() != after_effect.dynamic_invoke_entry()
    {
        stats.rewritten_dynamic_entries += 1;
    }
    stats.pruned_resume_methods += before_object
        .methods()
        .len()
        .saturating_sub(optimized.continuation_object.methods().len());
}

fn build_opt_pipeline_facts(
    options: LateLoweredOptOptions,
    stats: LirOptStats,
) -> LirOptPipelineFacts {
    let resume_pruning_status = if options.preserve_published_resume_shells {
        LirOptPassStatus::Skipped
    } else {
        LirOptPassStatus::Applied
    };
    LirOptPipelineFacts::new(
        LIR_OPT_PIPELINE_REVISION,
        options.preserve_published_resume_shells,
        vec![
            LirOptPassFacts::new(
                LirOptPassKind::LocalStateMachineElimination,
                LirOptPassStatus::Applied,
                stats.local_state_machine_changed(),
            ),
            LirOptPassFacts::new(
                LirOptPassKind::HigherOrderWrapperInlineDevirt,
                LirOptPassStatus::NoOp,
                false,
            ),
            LirOptPassFacts::new(
                LirOptPassKind::WrapperStateFolding,
                LirOptPassStatus::Applied,
                stats.wrapper_state_folding_changed(),
            ),
            LirOptPassFacts::new(
                LirOptPassKind::DynamicInvokeEntryRewrite,
                LirOptPassStatus::Applied,
                stats.dynamic_invoke_rewrite_changed(),
            ),
            LirOptPassFacts::new(
                LirOptPassKind::DeadStateSlotCleanup,
                LirOptPassStatus::Applied,
                stats.dead_cleanup_changed(),
            ),
            LirOptPassFacts::new(
                LirOptPassKind::ResumePackingPruning,
                resume_pruning_status,
                !options.preserve_published_resume_shells && stats.resume_packing_pruning_changed(),
            ),
            LirOptPassFacts::new(
                LirOptPassKind::PostOptVerifier,
                LirOptPassStatus::Applied,
                false,
            ),
        ],
    )
}

struct OptimizedCallable {
    callable: LateLoweredCallable,
    continuation_object: LateLoweredContinuationObject,
}

struct OptimizedControlBody {
    state_graph: LateLoweredStateGraph,
    frame_schema: LateLoweredFrameSchema,
    boundary_map: LateLoweredBoundaryMap,
    resume_state_map: LateLoweredResumeStateMap,
    continuation_object: LateLoweredContinuationObject,
    resume_packings: Vec<ResumeInterfaceId>,
}

fn preserve_source_callable(
    callable: LateLoweredCallable,
    original: &LateLoweredCallable,
) -> LateLoweredCallable {
    let callable = callable.with_source_kind(original.source_kind());
    if let Some(source_callable) = original.source_callable() {
        callable.with_source_callable(source_callable)
    } else {
        callable
    }
}

fn optimize_callable(
    callable: &LateLoweredCallable,
    continuation_object: &LateLoweredContinuationObject,
    options: LateLoweredOptOptions,
) -> OptimizedCallable {
    let redirects = collect_state_redirects(callable.state_graph());
    let optimized = optimize_control_body(callable, continuation_object, options, &redirects);
    let callable = preserve_source_callable(
        LateLoweredCallable::new(
            callable.root_fqn().to_string(),
            callable.stable_instance_key().clone(),
            callable.body_version_key().clone(),
            callable.step_schema(),
            callable.resolved_outward_cases().to_vec(),
            rewrite_dynamic_invoke_entry(callable.dynamic_invoke_entry(), &redirects),
            optimized.state_graph,
            optimized.frame_schema,
            optimized.boundary_map.clone(),
            optimized.resume_state_map,
            callable.continuation_object(),
            optimized.resume_packings,
        )
        .with_source_statement_classifications(
            callable.source_statement_classifications().to_vec(),
        ),
        callable,
    );

    OptimizedCallable {
        callable,
        continuation_object: optimized.continuation_object,
    }
}

fn optimize_plain_callable(
    callable: &LateLoweredCallable,
    continuation_object: &LateLoweredContinuationObject,
    options: LateLoweredOptOptions,
) -> OptimizedCallable {
    let redirects = collect_state_redirects(callable.state_graph());
    let optimized = optimize_control_body(callable, continuation_object, options, &redirects);
    let plain = callable
        .plain_abi()
        .expect("plain local control callable should publish a plain ABI");
    let local_control = LateLoweredPlainLocalEffectControl::new(
        callable.step_schema(),
        optimized.state_graph,
        optimized.frame_schema,
        optimized.boundary_map,
        optimized.resume_state_map,
        callable.source_statement_classifications().to_vec(),
        callable.continuation_object(),
        optimized.resume_packings,
    );
    let plain = LateLoweredPlainCallable::new(
        plain.function_ty(),
        plain.param_tys().to_vec(),
        plain.return_ty(),
        plain.body_slices().to_vec(),
        plain.call_sites().to_vec(),
        Some(local_control),
    );
    let callable = preserve_source_callable(
        LateLoweredCallable::new_plain(
            callable.root_fqn().to_string(),
            callable.stable_instance_key().clone(),
            callable.body_version_key().clone(),
            callable.resolved_outward_cases().to_vec(),
            plain,
        ),
        callable,
    );

    OptimizedCallable {
        callable,
        continuation_object: optimized.continuation_object,
    }
}

fn optimize_control_body(
    callable: &LateLoweredCallable,
    continuation_object: &LateLoweredContinuationObject,
    options: LateLoweredOptOptions,
    redirects: &BTreeMap<StateId, StateId>,
) -> OptimizedControlBody {
    let state_graph = rewrite_state_graph(callable.state_graph(), redirects);
    let live_states = state_graph
        .states()
        .iter()
        .map(LateLoweredState::state_id)
        .collect::<BTreeSet<_>>();
    let boundary_map = rewrite_boundary_map(callable.boundary_map(), redirects, &live_states);
    let live_boundaries = boundary_map
        .entries()
        .iter()
        .map(LateLoweredBoundary::boundary_id)
        .collect::<BTreeSet<_>>();
    let frame_schema = rewrite_frame_schema(
        callable.frame_schema(),
        redirects,
        &live_states,
        &live_boundaries,
    );
    let live_slots = frame_schema
        .slots()
        .iter()
        .map(LateLoweredFrameSlot::slot_id)
        .collect::<BTreeSet<_>>();
    let methods = continuation_object
        .methods()
        .iter()
        .filter(|method| {
            method.reachability() == LateLoweredContinuationMethodReachability::Reachable
        })
        .cloned()
        .collect::<Vec<_>>();
    let live_interfaces = methods
        .iter()
        .map(LateLoweredContinuationMethod::packing_interface_id)
        .collect::<BTreeSet<_>>();
    let implemented_packings = if options.preserve_published_resume_shells {
        continuation_object.implemented_packings().to_vec()
    } else {
        continuation_object
            .implemented_packings()
            .iter()
            .copied()
            .filter(|interface_id| live_interfaces.contains(interface_id))
            .collect::<Vec<_>>()
    };
    let captures = rewrite_captures(
        continuation_object.captures(),
        redirects,
        &live_states,
        &live_slots,
    );
    let continuation_object = LateLoweredContinuationObject::new(
        continuation_object.object_id(),
        continuation_object.owner_version_key().clone(),
        continuation_object.continuation_obj_ty(),
        implemented_packings.clone(),
        captures,
        continuation_object.surface_resumes().to_vec(),
        methods,
    );
    let resume_state_map = resume_state_map_from_boundaries(&boundary_map);
    let resume_packings = if options.preserve_published_resume_shells {
        callable.resume_packings().to_vec()
    } else {
        implemented_packings.clone()
    };
    OptimizedControlBody {
        state_graph,
        frame_schema,
        boundary_map,
        resume_state_map,
        continuation_object,
        resume_packings,
    }
}

fn with_callable_resume_packings(
    callable: LateLoweredCallable,
    resume_packings: Vec<ResumeInterfaceId>,
) -> LateLoweredCallable {
    if callable.effect_step_abi().is_some() {
        return preserve_source_callable(
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
                resume_packings,
            )
            .with_source_statement_classifications(
                callable.source_statement_classifications().to_vec(),
            ),
            &callable,
        );
    }

    let plain = callable
        .plain_abi()
        .expect("plain local control callable should publish a plain ABI");
    let local_control = LateLoweredPlainLocalEffectControl::new(
        callable.step_schema(),
        callable.state_graph().clone(),
        callable.frame_schema().clone(),
        callable.boundary_map().clone(),
        callable.resume_state_map().clone(),
        callable.source_statement_classifications().to_vec(),
        callable.continuation_object(),
        resume_packings,
    );
    let plain = LateLoweredPlainCallable::new(
        plain.function_ty(),
        plain.param_tys().to_vec(),
        plain.return_ty(),
        plain.body_slices().to_vec(),
        plain.call_sites().to_vec(),
        Some(local_control),
    );
    preserve_source_callable(
        LateLoweredCallable::new_plain(
            callable.root_fqn().to_string(),
            callable.stable_instance_key().clone(),
            callable.body_version_key().clone(),
            callable.resolved_outward_cases().to_vec(),
            plain,
        ),
        &callable,
    )
}

fn collect_live_methods_by_interface<'a>(
    continuation_objects: impl Iterator<Item = &'a LateLoweredContinuationObject>,
) -> BTreeMap<ResumeInterfaceId, BTreeSet<CaseTag>> {
    let mut live_methods = BTreeMap::<ResumeInterfaceId, BTreeSet<CaseTag>>::new();
    for continuation_object in continuation_objects {
        for method in continuation_object.methods() {
            live_methods
                .entry(method.packing_interface_id())
                .or_default()
                .insert(method.case_tag());
        }
    }
    live_methods
}

fn prune_resume_interface(
    interface: &LateLoweredResumeInterface,
    live_cases: Option<&BTreeSet<CaseTag>>,
) -> Option<LateLoweredResumeInterface> {
    let live_cases = live_cases?;
    let methods = interface
        .methods()
        .iter()
        .filter(|method| live_cases.contains(&method.case_tag()))
        .cloned()
        .collect::<Vec<_>>();
    if methods.is_empty() {
        return None;
    }

    Some(LateLoweredResumeInterface::new(
        interface.interface_id(),
        interface.effect_family().clone(),
        interface.return_step_schema(),
        methods,
    ))
}

fn prune_object_interfaces(
    continuation_object: LateLoweredContinuationObject,
    live_interface_ids: &BTreeSet<ResumeInterfaceId>,
) -> LateLoweredContinuationObject {
    let methods = continuation_object
        .methods()
        .iter()
        .filter(|method| live_interface_ids.contains(&method.packing_interface_id()))
        .cloned()
        .collect::<Vec<_>>();
    let used_interfaces = methods
        .iter()
        .map(LateLoweredContinuationMethod::packing_interface_id)
        .collect::<BTreeSet<_>>();
    let implemented_packings = continuation_object
        .implemented_packings()
        .iter()
        .copied()
        .filter(|interface_id| used_interfaces.contains(interface_id))
        .collect::<Vec<_>>();

    LateLoweredContinuationObject::new(
        continuation_object.object_id(),
        continuation_object.owner_version_key().clone(),
        continuation_object.continuation_obj_ty(),
        implemented_packings,
        continuation_object.captures().to_vec(),
        continuation_object.surface_resumes().to_vec(),
        methods,
    )
}

fn collect_state_redirects(state_graph: &LateLoweredStateGraph) -> BTreeMap<StateId, StateId> {
    let state_by_id = state_graph
        .states()
        .iter()
        .map(|state| (state.state_id(), state))
        .collect::<BTreeMap<_, _>>();

    state_graph
        .states()
        .iter()
        .filter_map(|state| {
            let redirected = resolve_redirect_target(state.state_id(), state_graph, &state_by_id);
            (redirected != state.state_id()).then_some((state.state_id(), redirected))
        })
        .collect()
}

fn resolve_redirect_target(
    state_id: StateId,
    state_graph: &LateLoweredStateGraph,
    state_by_id: &BTreeMap<StateId, &LateLoweredState>,
) -> StateId {
    let mut current = state_id;
    let mut seen = BTreeSet::new();
    while seen.insert(current) {
        let Some(state) = state_by_id.get(&current).copied() else {
            break;
        };
        let Some(target) = trivial_wrapper_target(state, state_graph) else {
            break;
        };
        current = target;
    }
    current
}

fn trivial_wrapper_target(
    state: &LateLoweredState,
    state_graph: &LateLoweredStateGraph,
) -> Option<StateId> {
    if state.state_id() == state_graph.entry_state()
        || state.state_id() == state_graph.complete_state()
        || state_graph.cleanup_state() == Some(state.state_id())
    {
        return None;
    }
    if state_graph.drop_state() == Some(state.state_id()) {
        return None;
    }
    if !matches!(
        state.role(),
        LateLoweredStateRole::Segment | LateLoweredStateRole::Resume
    ) {
        return None;
    }
    if !state.source_slices().is_empty() {
        return None;
    }
    match state.terminator() {
        LateLoweredStateTerminator::Goto { target } if *target != state.state_id() => Some(*target),
        _ => None,
    }
}

fn rewrite_state_graph(
    state_graph: &LateLoweredStateGraph,
    redirects: &BTreeMap<StateId, StateId>,
) -> LateLoweredStateGraph {
    let rewritten_states = state_graph
        .states()
        .iter()
        .filter(|state| !redirects.contains_key(&state.state_id()))
        .map(|state| rewrite_state(state, redirects))
        .collect::<Vec<_>>();
    let mut live_states = reachable_states(state_graph.entry_state(), &rewritten_states);
    // `drop_state` is entered via the continuation runtime contract rather than an ordinary CFG
    // successor edge, so DCE must seed reachability from it explicitly.
    if let Some(drop_state) = state_graph.drop_state() {
        live_states.extend(reachable_states(drop_state, &rewritten_states));
    }
    live_states.insert(state_graph.complete_state());

    let states = rewritten_states
        .into_iter()
        .filter(|state| live_states.contains(&state.state_id()))
        .collect::<Vec<_>>();
    let cleanup_state = state_graph
        .cleanup_state()
        .filter(|state_id| live_states.contains(state_id));
    let drop_state = state_graph
        .drop_state()
        .filter(|state_id| live_states.contains(state_id));

    LateLoweredStateGraph::new(
        state_graph.entry_state(),
        state_graph.complete_state(),
        cleanup_state,
        drop_state,
        states,
    )
}

fn reachable_states(entry_state: StateId, states: &[LateLoweredState]) -> BTreeSet<StateId> {
    let state_by_id = states
        .iter()
        .map(|state| (state.state_id(), state))
        .collect::<BTreeMap<_, _>>();
    let mut reachable = BTreeSet::new();
    let mut pending = VecDeque::from([entry_state]);
    while let Some(state_id) = pending.pop_front() {
        if !reachable.insert(state_id) {
            continue;
        }
        let Some(state) = state_by_id.get(&state_id) else {
            continue;
        };
        pending.extend(state.successors().iter().copied());
    }
    reachable
}

fn rewrite_state(
    state: &LateLoweredState,
    redirects: &BTreeMap<StateId, StateId>,
) -> LateLoweredState {
    LateLoweredState::new(
        state.state_id(),
        state.role(),
        state.source_slices().to_vec(),
        rewrite_terminator(state.terminator(), redirects),
    )
}

fn rewrite_terminator(
    terminator: &LateLoweredStateTerminator,
    redirects: &BTreeMap<StateId, StateId>,
) -> LateLoweredStateTerminator {
    match terminator.clone() {
        LateLoweredStateTerminator::Suspend {
            boundary_ids,
            resume_state,
            local_runtime_error_states,
            cleanup_state,
            drop_state,
        } => LateLoweredStateTerminator::Suspend {
            boundary_ids,
            resume_state: redirect_state_id(resume_state, redirects),
            local_runtime_error_states: local_runtime_error_states
                .into_iter()
                .map(|state_id| redirect_state_id(state_id, redirects))
                .collect(),
            cleanup_state: cleanup_state.map(|state_id| redirect_state_id(state_id, redirects)),
            drop_state: drop_state.map(|state_id| redirect_state_id(state_id, redirects)),
        },
        LateLoweredStateTerminator::Goto { target } => LateLoweredStateTerminator::Goto {
            target: redirect_state_id(target, redirects),
        },
        LateLoweredStateTerminator::Branch {
            cond_local,
            then_state,
            else_state,
        } => LateLoweredStateTerminator::Branch {
            cond_local,
            then_state: redirect_state_id(then_state, redirects),
            else_state: redirect_state_id(else_state, redirects),
        },
        LateLoweredStateTerminator::Return {
            payload_source,
            complete_state,
        } => LateLoweredStateTerminator::Return {
            payload_source,
            complete_state: redirect_state_id(complete_state, redirects),
        },
        LateLoweredStateTerminator::HandleDispatch {
            site_id,
            body_state,
            arm_states,
            finally_state,
            exit_state,
            contract,
            boundary_ids,
            drop_state,
        } => LateLoweredStateTerminator::HandleDispatch {
            site_id,
            body_state: redirect_state_id(body_state, redirects),
            arm_states: arm_states
                .into_iter()
                .map(|state_id| redirect_state_id(state_id, redirects))
                .collect(),
            finally_state: finally_state.map(|state_id| redirect_state_id(state_id, redirects)),
            exit_state: redirect_state_id(exit_state, redirects),
            contract: redirect_handle_dispatch_contract(contract, redirects),
            boundary_ids,
            drop_state: drop_state.map(|state_id| redirect_state_id(state_id, redirects)),
        },
        other => other,
    }
}

fn redirect_handle_dispatch_contract(
    contract: crate::effect_lowered::ir::LateLoweredHandleDispatchContract,
    redirects: &BTreeMap<StateId, StateId>,
) -> crate::effect_lowered::ir::LateLoweredHandleDispatchContract {
    crate::effect_lowered::ir::LateLoweredHandleDispatchContract::new(
        contract.carrier(),
        redirect_state_id(contract.body_complete_target(), redirects),
        redirect_state_id(contract.arm_complete_target(), redirects),
        contract
            .finally_complete_target()
            .map(|state_id| redirect_state_id(state_id, redirects)),
        contract.body_completion_payload_source().cloned(),
        contract
            .handled_arms()
            .iter()
            .map(|arm| {
                crate::effect_lowered::ir::LateLoweredHandleArmDispatch::new(
                    arm.handled_case(),
                    redirect_state_id(arm.arm_state(), redirects),
                    arm.arm_ordinal(),
                    arm.payload_tuple_ty(),
                    arm.completion_payload_source().clone(),
                    arm.payload_binders().to_vec(),
                    arm.continuation_binder(),
                    arm.arm_outward_cases().to_vec(),
                )
            })
            .collect(),
        contract.body_outward_cases().to_vec(),
        contract.finally_outward_cases().to_vec(),
        contract.outward_emissions().to_vec(),
        contract.pending_completions().to_vec(),
        contract
            .pending_completion_origins()
            .iter()
            .map(|origin| {
                crate::effect_lowered::ir::LateLoweredHandlePendingCompletionOrigin::new(
                    origin.completion(),
                    origin.boundary_id(),
                    redirect_state_id(origin.owner_state(), redirects),
                    redirect_state_id(origin.resume_state(), redirects),
                )
            })
            .collect(),
        contract.pending_payload_transports().to_vec(),
        contract
            .state_regions()
            .iter()
            .map(|entry| {
                crate::effect_lowered::ir::LateLoweredHandleStateRegionEntry::new(
                    redirect_state_id(entry.state_id(), redirects),
                    entry.region(),
                )
            })
            .collect(),
        contract
            .boundary_routings()
            .iter()
            .map(|routing| {
                let case_routings = routing
                    .case_routings()
                    .iter()
                    .map(|route| {
                        let action = match route.action() {
                            crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::ConsumeToArm {
                                arm_state,
                                arm_ordinal,
                                continuation_resume_state,
                            } => crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::ConsumeToArm {
                                arm_state: redirect_state_id(arm_state, redirects),
                                arm_ordinal,
                                continuation_resume_state: redirect_state_id(
                                    continuation_resume_state,
                                    redirects,
                                ),
                            },
                            crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::PendingCompletion {
                                completion,
                            } => crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::PendingCompletion {
                                completion,
                            },
                            crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::EmitOutward => crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::EmitOutward,
                        };
                        crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRouting::new(
                            route.case_tag(),
                            action,
                        )
                    })
                    .collect();
                crate::effect_lowered::ir::LateLoweredHandleBoundaryRouting::new(
                    routing.boundary_id(),
                    redirect_state_id(routing.owner_state(), redirects),
                    routing.owner_region(),
                    redirect_state_id(routing.resume_state(), redirects),
                    case_routings,
                )
            })
            .collect(),
        contract
            .abandon_target()
            .map(|state_id| redirect_state_id(state_id, redirects)),
    )
}

fn redirect_state_id(state_id: StateId, redirects: &BTreeMap<StateId, StateId>) -> StateId {
    let mut current = state_id;
    let mut seen = BTreeSet::new();
    while let Some(next) = redirects.get(&current).copied() {
        if !seen.insert(current) {
            break;
        }
        current = next;
    }
    current
}

fn rewrite_dynamic_invoke_entry(
    entry: &LateLoweredDynamicInvokeEntry,
    redirects: &BTreeMap<StateId, StateId>,
) -> LateLoweredDynamicInvokeEntry {
    LateLoweredDynamicInvokeEntry::new(
        entry.invoke_args_tuple_ty(),
        entry.step_schema(),
        redirect_state_id(entry.entry_state(), redirects),
        redirect_state_id(entry.complete_state(), redirects),
    )
}

fn rewrite_boundary_map(
    boundary_map: &LateLoweredBoundaryMap,
    redirects: &BTreeMap<StateId, StateId>,
    live_states: &BTreeSet<StateId>,
) -> LateLoweredBoundaryMap {
    let entries = boundary_map
        .entries()
        .iter()
        .filter_map(|boundary| {
            let owner_state = redirect_state_id(boundary.owner_state(), redirects);
            let resume_state = redirect_state_id(boundary.resume_state(), redirects);
            if !live_states.contains(&owner_state) || !live_states.contains(&resume_state) {
                return None;
            }
            let boundary = LateLoweredBoundary::new(
                boundary.boundary_id(),
                boundary.source(),
                owner_state,
                resume_state,
            );
            Some(
                match boundary_map
                    .boundary(boundary.boundary_id())
                    .and_then(|b| b.lowering())
                {
                    Some(lowering) => {
                        boundary.with_lowering(rewrite_boundary_lowering(lowering, redirects))
                    }
                    None => boundary,
                },
            )
        })
        .collect::<Vec<_>>();
    LateLoweredBoundaryMap::new(entries)
}

fn rewrite_boundary_lowering(
    lowering: &LateLoweredBoundaryLowering,
    redirects: &BTreeMap<StateId, StateId>,
) -> LateLoweredBoundaryLowering {
    match lowering {
        LateLoweredBoundaryLowering::Call(lowering) => {
            LateLoweredBoundaryLowering::Call(LateLoweredCallBoundaryLowering::new(
                lowering.facts().clone(),
                lowering.result_local(),
                lowering.metadata().clone(),
                lowering.operand_contract().clone(),
                rewrite_step_dispatch(lowering.dispatch(), redirects),
                lowering
                    .continuation_compositions()
                    .iter()
                    .cloned()
                    .map(|composition| {
                        let caller_resume_state =
                            redirect_state_id(composition.caller_resume_state(), redirects);
                        composition.with_caller_resume_state(caller_resume_state)
                    })
                    .collect(),
                lowering.consumed_runtime_error_case().cloned(),
            ))
        }
        LateLoweredBoundaryLowering::ClassCtor(lowering) => LateLoweredBoundaryLowering::ClassCtor(
            crate::effect_lowered::ir::LateLoweredClassCtorBoundaryLowering::new(
                lowering.facts().clone(),
                lowering.result_local(),
                lowering.class_fqn().to_string(),
                lowering.source_consumption(),
                lowering.emitted_steps().to_vec(),
            ),
        ),
        LateLoweredBoundaryLowering::Perform(lowering) => {
            LateLoweredBoundaryLowering::Perform(LateLoweredPerformBoundaryLowering::new(
                lowering.facts().clone(),
                lowering.operand_contract().clone(),
                lowering.emitted_step().clone(),
            ))
        }
        LateLoweredBoundaryLowering::Resume(lowering) => {
            LateLoweredBoundaryLowering::Resume(LateLoweredResumeBoundaryLowering::new(
                lowering.facts().clone(),
                lowering.result_local(),
                lowering.runtime_error_boundary(),
                lowering.operand_contract().clone(),
                rewrite_step_dispatch(lowering.dispatch(), redirects),
                lowering
                    .continuation_compositions()
                    .iter()
                    .cloned()
                    .map(|composition| {
                        let caller_resume_state =
                            redirect_state_id(composition.caller_resume_state(), redirects);
                        composition.with_caller_resume_state(caller_resume_state)
                    })
                    .collect(),
            ))
        }
        LateLoweredBoundaryLowering::RuntimeError(lowering) => {
            LateLoweredBoundaryLowering::RuntimeError(LateLoweredRuntimeErrorBoundaryLowering::new(
                lowering.origin_site(),
                lowering.resume_boundary(),
                lowering.emitted_step().clone(),
            ))
        }
        LateLoweredBoundaryLowering::Handle(lowering) => {
            LateLoweredBoundaryLowering::Handle(LateLoweredHandleBoundaryLowering::new(
                lowering.facts().clone(),
                lowering.outward_emissions().to_vec(),
            ))
        }
    }
}

fn rewrite_step_dispatch(
    dispatch: &LateLoweredStepDispatchPlan,
    redirects: &BTreeMap<StateId, StateId>,
) -> LateLoweredStepDispatchPlan {
    LateLoweredStepDispatchPlan::new(
        dispatch.input_step_schema(),
        rewrite_complete_dispatch(dispatch.complete(), redirects),
        dispatch.outward_cases().to_vec(),
    )
}

fn rewrite_complete_dispatch(
    complete: &LateLoweredCompleteStepDispatch,
    redirects: &BTreeMap<StateId, StateId>,
) -> LateLoweredCompleteStepDispatch {
    LateLoweredCompleteStepDispatch::new(
        complete.answer_ty(),
        redirect_state_id(complete.target_state(), redirects),
        complete.result_local(),
    )
}

fn resume_state_map_from_boundaries(
    boundary_map: &LateLoweredBoundaryMap,
) -> LateLoweredResumeStateMap {
    LateLoweredResumeStateMap::new(
        boundary_map
            .entries()
            .iter()
            .map(|boundary| {
                LateLoweredResumeState::new(boundary.boundary_id(), boundary.resume_state())
            })
            .collect(),
    )
}

fn rewrite_frame_schema(
    frame_schema: &LateLoweredFrameSchema,
    redirects: &BTreeMap<StateId, StateId>,
    live_states: &BTreeSet<StateId>,
    live_boundaries: &BTreeSet<BoundaryId>,
) -> LateLoweredFrameSchema {
    let slots = frame_schema
        .slots()
        .iter()
        .filter_map(|slot| {
            let kind = rewrite_frame_slot_kind(slot.kind(), live_boundaries)?;
            let write_points = rewrite_state_id_list(slot.write_points(), redirects, live_states);
            let read_points = rewrite_state_id_list(slot.read_points(), redirects, live_states);
            if !slot_is_live(kind, &read_points) {
                return None;
            }
            Some(LateLoweredFrameSlot::new(
                slot.slot_id(),
                kind,
                slot.ty(),
                write_points,
                read_points,
            ))
        })
        .collect::<Vec<_>>();
    let live_slots = slots
        .iter()
        .map(LateLoweredFrameSlot::slot_id)
        .collect::<BTreeSet<_>>();
    let resume_payload_bindings = frame_schema
        .resume_payload_bindings()
        .iter()
        .filter_map(|binding| {
            if !live_boundaries.contains(&binding.boundary_id()) {
                return None;
            }
            let resume_state = redirect_state_id(binding.resume_state(), redirects);
            if !live_states.contains(&resume_state) {
                return None;
            }
            Some(LateLoweredResumePayloadBinding::new(
                binding.boundary_id(),
                resume_state,
                binding.consumer_local(),
                binding
                    .consumer_frame_slot()
                    .filter(|slot_id| live_slots.contains(slot_id)),
            ))
        })
        .collect();
    let completion_payload_bindings = frame_schema
        .completion_payload_bindings()
        .iter()
        .filter_map(|binding| {
            let return_state = redirect_state_id(binding.return_state(), redirects);
            let complete_state = redirect_state_id(binding.complete_state(), redirects);
            if !live_states.contains(&return_state) || !live_states.contains(&complete_state) {
                return None;
            }
            Some(
                crate::effect_lowered::ir::LateLoweredCompletionPayloadBinding::new(
                    return_state,
                    complete_state,
                    binding.payload_source().clone(),
                    binding
                        .payload_frame_slot()
                        .filter(|slot_id| live_slots.contains(slot_id)),
                ),
            )
        })
        .collect();
    LateLoweredFrameSchema::new(slots)
        .with_resume_payload_bindings(resume_payload_bindings)
        .with_completion_payload_bindings(completion_payload_bindings)
}

fn rewrite_frame_slot_kind(
    kind: LateLoweredFrameSlotKind,
    live_boundaries: &BTreeSet<BoundaryId>,
) -> Option<LateLoweredFrameSlotKind> {
    match kind {
        LateLoweredFrameSlotKind::ResumePayload { boundary, .. }
        | LateLoweredFrameSlotKind::BoundaryResult { boundary, .. }
            if !live_boundaries.contains(&boundary) =>
        {
            None
        }
        other => Some(other),
    }
}

fn rewrite_state_id_list(
    state_ids: &[StateId],
    redirects: &BTreeMap<StateId, StateId>,
    live_states: &BTreeSet<StateId>,
) -> Vec<StateId> {
    state_ids
        .iter()
        .copied()
        .map(|state_id| redirect_state_id(state_id, redirects))
        .filter(|state_id| live_states.contains(state_id))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn slot_is_live(kind: LateLoweredFrameSlotKind, read_points: &[StateId]) -> bool {
    match kind {
        LateLoweredFrameSlotKind::System(_)
        | LateLoweredFrameSlotKind::HandleSavedEffectCtx { .. }
        | LateLoweredFrameSlotKind::HandleArmEffectCtx { .. }
        | LateLoweredFrameSlotKind::HandlePendingPayload { .. }
        | LateLoweredFrameSlotKind::ResumePayload { .. }
        | LateLoweredFrameSlotKind::BoundaryResult { .. } => true,
        _ => !read_points.is_empty(),
    }
}

fn rewrite_captures(
    captures: &[LateLoweredContinuationCapture],
    redirects: &BTreeMap<StateId, StateId>,
    live_states: &BTreeSet<StateId>,
    live_slots: &BTreeSet<FrameSlotId>,
) -> Vec<LateLoweredContinuationCapture> {
    let mut seen_slots = BTreeSet::new();
    let mut seen_states = BTreeSet::new();
    let mut rewritten = Vec::new();

    for capture in captures {
        match *capture {
            LateLoweredContinuationCapture::FrameSlot(slot_id)
                if live_slots.contains(&slot_id) && seen_slots.insert(slot_id) =>
            {
                rewritten.push(LateLoweredContinuationCapture::FrameSlot(slot_id));
            }
            LateLoweredContinuationCapture::State(state_id) => {
                let redirected = redirect_state_id(state_id, redirects);
                if live_states.contains(&redirected) && seen_states.insert(redirected) {
                    rewritten.push(LateLoweredContinuationCapture::State(redirected));
                }
            }
            LateLoweredContinuationCapture::FrameSlot(_) => {}
        }
    }

    rewritten
}

#[cfg(all(test, not(feature = "standalone-stage-crate")))]
mod tests {
    use std::path::PathBuf;

    use super::{LateLoweredOptOptions, optimize_program, run_lir_opt_pipeline};
    use crate::effect_facts::{
        CaseTag, ConcreteOpKey, ContinuationSchemaId, EffectFamilyKey, ImplPlan, StepSchemaId,
    };
    use crate::effect_lowered::ir::{
        BoundaryId, BoundarySiteKind, ContinuationObjectId, FrameSlotId, LateLoweredBodyVersionKey,
        LateLoweredBoundary, LateLoweredBoundaryMap, LateLoweredBoundarySource,
        LateLoweredCallable, LateLoweredCompletionPayloadSource, LateLoweredContinuationCapture,
        LateLoweredContinuationContract, LateLoweredContinuationMethod,
        LateLoweredContinuationObject, LateLoweredContinuationResumeBody,
        LateLoweredContinuationSurfaceResume, LateLoweredDynamicInvokeEntry,
        LateLoweredFrameSchema, LateLoweredFrameSlot, LateLoweredFrameSlotKind,
        LateLoweredHandleDispatchContract, LateLoweredOneShotPolicy, LateLoweredOperandSource,
        LateLoweredProgram, LateLoweredResumeInterface, LateLoweredResumeMethod,
        LateLoweredResumeState, LateLoweredResumeStateMap, LateLoweredState, LateLoweredStateGraph,
        LateLoweredStateRole, LateLoweredStateSlice, LateLoweredStateTerminator,
        LateLoweredStepCase, LateLoweredStepType, ResumeInterfaceId, StateId, SystemSlotKind,
    };
    use crate::effect_lowered::opt_verify::verify_post_opt_program;
    use crate::mir::{BasicBlockId, InstanceKey, LocalId, SiteId, TemplateKey};
    use crate::pipeline::load_effect_lowered_stage_output_for_dump;
    use crate::session::{Session, SessionOptions};
    use crate::source::SourceFile;
    use crate::span::Span;
    use crate::stable_id::{
        NoTypeParamResolver, StableConeKey, StableDefKey, StableDefNamespace, StableTemplateKey,
    };
    use crate::ty::{EffectRow, NominalType, RefTypeKind, TypeId, TypeKind, TypeStore};
    use scoopc_lir_facts::{
        LIR_OPT_PIPELINE_REVISION, LirCallableSourceKind, LirOptPassKind, LirOptPassStatus,
    };

    fn session() -> Session {
        Session::with_options(SessionOptions::new()).unwrap()
    }

    fn load_fixture(phase: &str, name: &str) -> SourceFile {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures")
            .join(phase)
            .join(name);
        SourceFile::load(&path).expect("fixture 应可加载")
    }

    fn sample_instance_key(fqn: &str) -> InstanceKey {
        InstanceKey {
            template: TemplateKey {
                fqn: fqn.to_string(),
                source_path: PathBuf::from("<mem>/late_opt.scoop"),
                decl_span: Span::new(0, 0),
            },
            type_args: Vec::new(),
            eff_args: Vec::new(),
        }
    }

    fn sample_stable_instance_key(
        instance: &InstanceKey,
        types: &TypeStore,
    ) -> crate::stable_id::StableInstanceKey {
        crate::stable_id::StableInstanceKey::from_type_arguments(
            StableTemplateKey::new(StableDefKey::new(
                StableConeKey::new("sample", "0.0.0"),
                StableDefNamespace::Fun,
                &instance.template.fqn,
                "top_level_fun",
                None,
            )),
            types,
            &instance.type_args,
            &instance.eff_args,
            &NoTypeParamResolver,
        )
        .expect("sample instance 应可构造 stable instance key")
    }

    fn sample_concrete_op_key(
        types: &TypeStore,
        fqn: &str,
        effect_family: EffectFamilyKey,
    ) -> ConcreteOpKey {
        let instance = sample_instance_key(fqn);
        let stable_instance = sample_stable_instance_key(&instance, types);
        ConcreteOpKey::new(instance, stable_instance, effect_family)
    }

    fn nominal_effect(types: &mut TypeStore, fqn: &str) -> TypeId {
        types.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
            fqn: fqn.to_string(),
            args: Vec::new(),
            eff: None,
        })))
    }

    fn sample_opt_program() -> LateLoweredProgram {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let ping_effect = nominal_effect(&mut types, "sample.Ping");
        let allowed_row = EffectRow::new(vec![ping_effect]);
        let invoke_args_tuple_ty = types.ty_tuple(vec![builtins.int]);
        let payload_tuple_ty = types.ty_tuple(vec![builtins.string]);
        let resume_tuple_ty = types.ty_tuple(vec![builtins.int]);
        let continuation_obj_ty = nominal_effect(&mut types, "sample.CompilerContinuation");
        let surface_ty0 = nominal_effect(&mut types, "sample.SurfaceContinuation0");
        let surface_ty1 = nominal_effect(&mut types, "sample.SurfaceContinuation1");
        let ping_family = EffectFamilyKey::new("sample.Ping".to_string(), Vec::new());

        let step_schema = StepSchemaId::new(7);
        let case0 = CaseTag::new(0);
        let case1 = CaseTag::new(1);
        let contract0 = LateLoweredContinuationContract::new(
            ContinuationSchemaId::new(3),
            resume_tuple_ty,
            builtins.unit,
            step_schema,
            surface_ty0,
        );
        let contract1 = LateLoweredContinuationContract::new(
            ContinuationSchemaId::new(4),
            builtins.unit,
            builtins.unit,
            step_schema,
            surface_ty1,
        );
        let step_type = LateLoweredStepType::new(
            step_schema,
            invoke_args_tuple_ty,
            builtins.unit,
            continuation_obj_ty,
            vec![
                LateLoweredStepCase::new(
                    case0,
                    sample_concrete_op_key(&types, "sample.Ping.hit", ping_family.clone()),
                    payload_tuple_ty,
                    contract0,
                ),
                LateLoweredStepCase::new(
                    case1,
                    sample_concrete_op_key(&types, "sample.Ping.pong", ping_family.clone()),
                    builtins.unit,
                    contract1,
                ),
            ],
        );
        let interface_id = ResumeInterfaceId::new(0);
        let resume_interface = LateLoweredResumeInterface::new(
            interface_id,
            ping_family.clone(),
            step_schema,
            vec![
                LateLoweredResumeMethod::new(
                    case0,
                    sample_concrete_op_key(&types, "sample.Ping.hit", ping_family.clone()),
                    contract0,
                ),
                LateLoweredResumeMethod::new(
                    case1,
                    sample_concrete_op_key(&types, "sample.Ping.pong", ping_family.clone()),
                    contract1,
                ),
            ],
        );

        let worker_instance = sample_instance_key("sample.worker");
        let worker_stable_instance = sample_stable_instance_key(&worker_instance, &types);
        let version_key = LateLoweredBodyVersionKey::new(
            worker_instance,
            allowed_row,
            ImplPlan::SingleCase(case0),
            true,
        );
        let continuation_object_id = ContinuationObjectId::new(0);
        let entry_state = StateId::new(0);
        let invoke_wrapper_state = StateId::new(1);
        let owner_state = StateId::new(2);
        let resume_wrapper_state = StateId::new(3);
        let resume_state = StateId::new(4);
        let dead_state = StateId::new(5);
        let complete_state = StateId::new(6);
        let boundary_id = BoundaryId::new(0);
        let live_slot = FrameSlotId::new(0);
        let dead_slot = FrameSlotId::new(1);
        let system_slot = FrameSlotId::new(2);

        let continuation_object = LateLoweredContinuationObject::new(
            continuation_object_id,
            version_key.clone(),
            continuation_obj_ty,
            vec![interface_id],
            vec![
                LateLoweredContinuationCapture::FrameSlot(live_slot),
                LateLoweredContinuationCapture::FrameSlot(dead_slot),
                LateLoweredContinuationCapture::State(resume_wrapper_state),
            ],
            vec![
                LateLoweredContinuationSurfaceResume::new(
                    case0,
                    sample_concrete_op_key(&types, "sample.Ping.hit", ping_family.clone()),
                    contract0,
                    LateLoweredContinuationResumeBody::ResumeCapturedState {
                        repeated_resume: LateLoweredOneShotPolicy::OrdinaryRuntimeErrorOutward,
                    },
                ),
                LateLoweredContinuationSurfaceResume::new(
                    case1,
                    sample_concrete_op_key(&types, "sample.Ping.pong", ping_family.clone()),
                    contract1,
                    LateLoweredContinuationResumeBody::Unreachable,
                ),
            ],
            vec![
                LateLoweredContinuationMethod::new(
                    interface_id,
                    case0,
                    sample_concrete_op_key(&types, "sample.Ping.hit", ping_family.clone()),
                    contract0,
                    LateLoweredContinuationResumeBody::ResumeCapturedState {
                        repeated_resume: LateLoweredOneShotPolicy::OrdinaryRuntimeErrorOutward,
                    },
                ),
                LateLoweredContinuationMethod::new(
                    interface_id,
                    case1,
                    sample_concrete_op_key(&types, "sample.Ping.pong", ping_family),
                    contract1,
                    LateLoweredContinuationResumeBody::Unreachable,
                ),
            ],
        );
        let callable = LateLoweredCallable::new(
            "sample.worker".to_string(),
            worker_stable_instance,
            version_key,
            step_schema,
            vec![case0],
            LateLoweredDynamicInvokeEntry::new(
                invoke_args_tuple_ty,
                step_schema,
                entry_state,
                complete_state,
            ),
            LateLoweredStateGraph::new(
                entry_state,
                complete_state,
                None,
                None,
                vec![
                    LateLoweredState::new(
                        entry_state,
                        LateLoweredStateRole::Entry,
                        Vec::new(),
                        LateLoweredStateTerminator::Goto {
                            target: invoke_wrapper_state,
                        },
                    ),
                    LateLoweredState::new(
                        invoke_wrapper_state,
                        LateLoweredStateRole::Segment,
                        Vec::new(),
                        LateLoweredStateTerminator::Goto {
                            target: owner_state,
                        },
                    ),
                    LateLoweredState::new(
                        owner_state,
                        LateLoweredStateRole::Segment,
                        vec![LateLoweredStateSlice::new(
                            BasicBlockId::from_raw(0),
                            0,
                            1,
                            false,
                        )],
                        LateLoweredStateTerminator::Suspend {
                            boundary_ids: vec![boundary_id],
                            resume_state: resume_wrapper_state,
                            local_runtime_error_states: Vec::new(),
                            cleanup_state: None,
                            drop_state: None,
                        },
                    ),
                    LateLoweredState::new(
                        resume_wrapper_state,
                        LateLoweredStateRole::Resume,
                        Vec::new(),
                        LateLoweredStateTerminator::Goto {
                            target: resume_state,
                        },
                    ),
                    LateLoweredState::new(
                        resume_state,
                        LateLoweredStateRole::Resume,
                        vec![LateLoweredStateSlice::new(
                            BasicBlockId::from_raw(0),
                            1,
                            1,
                            true,
                        )],
                        LateLoweredStateTerminator::Return {
                            payload_source: LateLoweredCompletionPayloadSource::operand(
                                LateLoweredOperandSource::new_local(
                                    LocalId::from_raw(0),
                                    builtins.int,
                                    None,
                                ),
                            ),
                            complete_state,
                        },
                    ),
                    LateLoweredState::new(
                        dead_state,
                        LateLoweredStateRole::Segment,
                        vec![LateLoweredStateSlice::new(
                            BasicBlockId::from_raw(1),
                            0,
                            0,
                            false,
                        )],
                        LateLoweredStateTerminator::Goto {
                            target: complete_state,
                        },
                    ),
                    LateLoweredState::new(
                        complete_state,
                        LateLoweredStateRole::Complete,
                        Vec::new(),
                        LateLoweredStateTerminator::Unreachable,
                    ),
                ],
            ),
            LateLoweredFrameSchema::new(vec![
                LateLoweredFrameSlot::new(
                    live_slot,
                    LateLoweredFrameSlotKind::SourceLocal(LocalId::from_raw(0)),
                    builtins.int,
                    vec![owner_state],
                    vec![resume_wrapper_state],
                ),
                LateLoweredFrameSlot::new(
                    dead_slot,
                    LateLoweredFrameSlotKind::CompilerTemporary(LocalId::from_raw(1)),
                    builtins.int,
                    vec![dead_state],
                    Vec::new(),
                ),
                LateLoweredFrameSlot::new(
                    system_slot,
                    LateLoweredFrameSlotKind::System(SystemSlotKind::StateTag),
                    builtins.int,
                    Vec::new(),
                    Vec::new(),
                ),
            ]),
            LateLoweredBoundaryMap::new(vec![LateLoweredBoundary::new(
                boundary_id,
                LateLoweredBoundarySource::Site {
                    site_id: SiteId::from_raw(1),
                    kind: BoundarySiteKind::Perform,
                },
                owner_state,
                resume_wrapper_state,
            )]),
            LateLoweredResumeStateMap::new(vec![LateLoweredResumeState::new(
                boundary_id,
                resume_wrapper_state,
            )]),
            continuation_object_id,
            vec![interface_id],
        )
        .with_source_kind(LirCallableSourceKind::MemberOrSynthetic);

        LateLoweredProgram::new(
            vec![step_type],
            vec![resume_interface],
            vec![continuation_object],
            vec![callable],
        )
    }

    #[test]
    fn late_opt_pipeline_publishes_named_pass_order_and_revision() {
        let output = run_lir_opt_pipeline(sample_opt_program(), LateLoweredOptOptions::default())
            .expect("sample program should pass LIR opt verification");
        let (_program, pipeline) = output.into_parts();
        let pass_kinds = pipeline
            .passes
            .iter()
            .map(|pass| pass.kind)
            .collect::<Vec<_>>();

        assert_eq!(pipeline.revision, LIR_OPT_PIPELINE_REVISION);
        assert!(!pipeline.preserve_published_resume_shells);
        assert_eq!(
            pass_kinds,
            vec![
                LirOptPassKind::LocalStateMachineElimination,
                LirOptPassKind::HigherOrderWrapperInlineDevirt,
                LirOptPassKind::WrapperStateFolding,
                LirOptPassKind::DynamicInvokeEntryRewrite,
                LirOptPassKind::DeadStateSlotCleanup,
                LirOptPassKind::ResumePackingPruning,
                LirOptPassKind::PostOptVerifier,
            ]
        );
        assert_eq!(
            pipeline.passes[1].status,
            LirOptPassStatus::NoOp,
            "higher-order wrapper inline/devirt has an explicit owner even before it grows non-trivial rewrites",
        );
        assert!(pipeline.passes[2].changed);
        assert_eq!(pipeline.passes[3].status, LirOptPassStatus::Applied);
        assert!(pipeline.passes[4].changed);
        assert!(pipeline.passes[5].changed);
    }

    #[test]
    fn late_opt_devirt_prunes_unreachable_internal_resume_methods() {
        let optimized = optimize_program(sample_opt_program());
        let callable = optimized
            .callable("sample.worker")
            .expect("优化后应保留 sample.worker callable");
        let continuation_object = optimized
            .continuation_object(callable.continuation_object())
            .expect("优化后应保留 continuation object");
        let resume_interface = optimized
            .resume_packing(callable.resume_packings()[0])
            .expect("优化后应保留 live resume interface");

        assert_eq!(callable.resume_packings().len(), 1);
        assert_eq!(resume_interface.methods().len(), 1);
        assert_eq!(resume_interface.methods()[0].case_tag(), CaseTag::new(0));
        assert_eq!(continuation_object.methods().len(), 1);
        assert_eq!(continuation_object.methods()[0].case_tag(), CaseTag::new(0));
        assert_eq!(continuation_object.surface_resumes().len(), 2);
        assert_eq!(
            optimized
                .step_type(callable.step_schema())
                .expect("优化后 Step shell 仍应存在")
                .cases()
                .len(),
            2,
        );
    }

    #[test]
    fn late_opt_inline_collapses_trivial_invoke_and_resume_wrappers() {
        let optimized = optimize_program(sample_opt_program());
        let callable = optimized
            .callable("sample.worker")
            .expect("优化后应保留 sample.worker callable");
        let continuation_object = optimized
            .continuation_object(callable.continuation_object())
            .expect("优化后应保留 continuation object");
        let boundary = callable
            .boundary_map()
            .boundary(BoundaryId::new(0))
            .expect("优化后应保留 boundary");

        assert!(callable.state_graph().state(StateId::new(1)).is_none());
        assert!(callable.state_graph().state(StateId::new(3)).is_none());
        assert!(matches!(
            callable
                .state_graph()
                .state(StateId::new(0))
                .expect("entry state 应保留")
                .terminator(),
            LateLoweredStateTerminator::Goto { target } if *target == StateId::new(2)
        ));
        assert_eq!(boundary.resume_state(), StateId::new(4));
        assert_eq!(
            callable.resume_state_map().state_for(BoundaryId::new(0)),
            Some(StateId::new(4))
        );
        assert!(
            continuation_object
                .captures()
                .contains(&LateLoweredContinuationCapture::State(StateId::new(4)))
        );
        assert!(
            !continuation_object
                .captures()
                .contains(&LateLoweredContinuationCapture::State(StateId::new(3)))
        );
    }

    #[test]
    fn late_opt_preserves_callable_source_kind_metadata() {
        let optimized = optimize_program(sample_opt_program());
        let callable = optimized
            .callable("sample.worker")
            .expect("优化后应保留 sample.worker callable");

        assert_eq!(
            callable.source_kind(),
            LirCallableSourceKind::MemberOrSynthetic
        );
    }

    #[test]
    fn late_opt_dce_removes_dead_states_and_unused_frame_slots() {
        let optimized = optimize_program(sample_opt_program());
        let callable = optimized
            .callable("sample.worker")
            .expect("优化后应保留 sample.worker callable");
        let continuation_object = optimized
            .continuation_object(callable.continuation_object())
            .expect("优化后应保留 continuation object");

        assert!(callable.state_graph().state(StateId::new(5)).is_none());
        assert!(
            callable
                .frame_schema()
                .slot_for_kind(LateLoweredFrameSlotKind::CompilerTemporary(
                    LocalId::from_raw(1)
                ))
                .is_none(),
            "无读者且只挂在死状态上的 compiler temporary 应被删除"
        );
        assert!(
            callable
                .frame_schema()
                .slot_for_kind(LateLoweredFrameSlotKind::System(SystemSlotKind::StateTag))
                .is_some(),
            "语义必需的系统 slot 不应被误删"
        );
        assert!(!continuation_object.captures().contains(
            &LateLoweredContinuationCapture::FrameSlot(FrameSlotId::new(1))
        ));
    }

    #[test]
    fn late_opt_devirt_stage_output_is_post_opt_final() {
        let session = session();
        let source = load_fixture("effect_facts", "single_case_impl_plan.scoop");
        let output = load_effect_lowered_stage_output_for_dump(&session, &source)
            .expect("fixture 应可通过 late-lowering stage");
        let leaf = output
            .program()
            .callable("sample.leaf")
            .expect("stage output 应发布 sample.leaf callable shell");
        let continuation_object = output
            .program()
            .continuation_object(leaf.continuation_object())
            .expect("callable 应能回查 continuation object shell");

        assert_eq!(leaf.resume_packings().len(), 1);
        assert_eq!(continuation_object.methods().len(), 1);
        assert_eq!(continuation_object.surface_resumes().len(), 2);
    }

    #[test]
    fn late_opt_preserves_dedicated_drop_state_paths() {
        let session = session();
        let source = load_fixture(
            "effect_lowered",
            "dropped_continuation_abandons_remaining_work.scoop",
        );
        let output = load_effect_lowered_stage_output_for_dump(&session, &source)
            .expect("fixture 应可通过 late-lowering stage");
        let callable = output
            .program()
            .callable("sample.helper")
            .expect("stage output 应保留 sample.helper callable shell");
        let drop_state = callable
            .state_graph()
            .drop_state()
            .expect("post-opt output 仍应保留 dedicated drop state");
        let drop_node = callable
            .state_graph()
            .state(drop_state)
            .expect("drop state 应可回查");

        assert_eq!(drop_node.role(), LateLoweredStateRole::Drop);
        assert!(matches!(
            drop_node.terminator(),
            LateLoweredStateTerminator::Abandon
        ));
        assert!(callable.state_graph().states().iter().any(|state| {
            matches!(
                state.terminator(),
                LateLoweredStateTerminator::Suspend {
                    cleanup_state: Some(cleanup_state),
                    drop_state: Some(explicit_drop_state),
                    ..
                } if *cleanup_state != drop_state && *explicit_drop_state == drop_state
            )
        }));
    }

    #[test]
    fn post_opt_verifier_rejects_dangling_handle_contract_state() {
        let valid =
            sample_program_with_entry_handle_contract(LateLoweredHandleDispatchContract::skeleton(
                StateId::new(6),
                StateId::new(6),
                None,
                None,
            ));
        verify_post_opt_program(&valid).expect("base handle contract should verify");

        let invalid =
            sample_program_with_entry_handle_contract(LateLoweredHandleDispatchContract::skeleton(
                StateId::new(999_999),
                StateId::new(6),
                None,
                None,
            ));
        let error = verify_post_opt_program(&invalid)
            .expect_err("dangling handle contract state should be rejected")
            .to_string();

        assert!(
            error.contains("handle body_complete_target references missing state st999999"),
            "unexpected verifier error: {error}"
        );
    }

    fn sample_program_with_entry_handle_contract(
        contract: LateLoweredHandleDispatchContract,
    ) -> LateLoweredProgram {
        let program = sample_opt_program();
        let callables = program
            .callables()
            .iter()
            .map(|callable| {
                if callable.root_fqn() == "sample.worker" {
                    return rewrite_callable_entry_handle_contract(callable, contract.clone());
                }
                callable.clone()
            })
            .collect::<Vec<_>>();

        LateLoweredProgram::new(
            program.step_types().to_vec(),
            program.resume_packings().to_vec(),
            program.continuation_objects().to_vec(),
            callables,
        )
        .with_stable_instance_keys(program.stable_instance_keys().clone())
        .with_dump_metadata(
            program.dump_type_texts().clone(),
            program.dump_body_labels_map().clone(),
        )
    }

    fn rewrite_callable_entry_handle_contract(
        callable: &LateLoweredCallable,
        contract: LateLoweredHandleDispatchContract,
    ) -> LateLoweredCallable {
        let state_graph =
            rewrite_state_graph_entry_handle_contract(callable.state_graph(), contract);
        super::preserve_source_callable(
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
            .with_source_statement_classifications(
                callable.source_statement_classifications().to_vec(),
            ),
            callable,
        )
    }

    fn rewrite_state_graph_entry_handle_contract(
        state_graph: &LateLoweredStateGraph,
        contract: LateLoweredHandleDispatchContract,
    ) -> LateLoweredStateGraph {
        let states = state_graph
            .states()
            .iter()
            .map(|state| {
                if state.state_id() != state_graph.entry_state() {
                    return state.clone();
                }
                LateLoweredState::new(
                    state.state_id(),
                    state.role(),
                    state.source_slices().to_vec(),
                    LateLoweredStateTerminator::HandleDispatch {
                        site_id: SiteId::from_raw(999),
                        body_state: StateId::new(1),
                        arm_states: Vec::new(),
                        finally_state: None,
                        exit_state: state_graph.complete_state(),
                        contract: contract.clone(),
                        boundary_ids: Vec::new(),
                        drop_state: None,
                    },
                )
            })
            .collect::<Vec<_>>();

        LateLoweredStateGraph::new(
            state_graph.entry_state(),
            state_graph.complete_state(),
            state_graph.cleanup_state(),
            state_graph.drop_state(),
            states,
        )
    }
}
