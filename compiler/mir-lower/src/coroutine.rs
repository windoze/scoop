//! Typed coroutine state-machine construction over normalized MIR CFG.

use std::collections::{HashMap, HashSet};

use la_arena::Arena;
use scoop_hir::concrete as hir;
use scoop_mir as mir;

use super::Lowerer;

mod adapters;
mod construction;
mod errors;
mod frame;
mod intrinsics;
mod liveness;
mod sites;
mod tail;

use adapters::*;
use construction::*;
use errors::protocol_error_block;
use intrinsics::*;
use liveness::analyze_sites;
pub(super) use liveness::suspend_effect;
use sites::*;

#[derive(Clone)]
struct SuspendSite {
    resume: ResumeSite,
    kind: SuspendKind,
}

#[derive(Clone)]
struct ResumeSite {
    block: mir::BlockId,
    statement: usize,
    destination: Option<mir::LocalId>,
    result: mir::Type,
    live_after: Vec<mir::LocalId>,
    pending: crate::cfg::CleanupContext,
    state: mir::CoroutineSuspendStateId,
    identity_path: hir::StructuralDefinitionPath,
}

#[derive(Clone)]
struct DiscoveredSuspendSite {
    order: usize,
    block: mir::BlockId,
    statement: usize,
    destination: Option<mir::LocalId>,
    result: mir::Type,
    kind: SuspendKind,
    live_after: Vec<mir::LocalId>,
    pending: crate::cfg::CleanupContext,
}

#[derive(Clone)]
enum SuspendKind {
    Call(mir::Call),
    Intrinsic(crate::cfg::SuspendCall),
}

pub(super) fn transform(lowerer: &mut Lowerer, module: &hir::Module) {
    let coroutine_ids: Vec<_> = lowerer
        .coroutines
        .functions
        .iter()
        .map(|(id, _)| id)
        .collect();
    for coroutine in coroutine_ids {
        let function = lowerer.coroutines.functions[coroutine].function;
        let calls = lowerer.coroutines.take_call_sites(function);
        let sites = analyze_sites(lowerer, &lowerer.functions[function].body, &calls);
        if sites.is_empty() {
            continue;
        }
        if lowerer.coroutine_elision && tail::rewrite(lowerer, coroutine, &sites) {
            continue;
        }
        transform_function(lowerer, module, coroutine, sites);
    }
    lowerer.coroutines.pre_coroutine_call_sites.clear();
}

fn transform_function(
    lowerer: &mut Lowerer,
    module: &hir::Module,
    coroutine: mir::CoroutineFunctionId,
    mut sites: Vec<SuspendSite>,
) {
    let coroutine_meta = &lowerer.coroutines.functions[coroutine];
    let function_id = coroutine_meta.function;
    let source_materialization = coroutine_meta.source;
    let source_odr_group = coroutine_meta.source_odr_group;
    let source_return = coroutine_meta.source_return.clone();
    let source_signature = coroutine_meta.logical_signature.clone();
    let step_ty = lowerer.functions[function_id].return_ty.clone();
    let (source_name, old_params, mut body) = {
        let function = &mut lowerer.functions[function_id];
        (
            function.name.clone(),
            std::mem::take(&mut function.params),
            std::mem::replace(&mut function.body, mir::Body::unreachable(Arena::new())),
        )
    };
    let completion = old_params
        .last()
        .expect("hidden coroutine ABI always has a completion parameter");
    let completion_old = completion.local;
    let completion_ty = completion.ty.clone();
    let source_params = &old_params[..old_params.len() - 1];
    let mut saved = HashSet::new();
    saved.extend(source_params.iter().map(|param| param.local));
    for planned in &sites {
        let site = &planned.resume;
        saved.extend(
            site.live_after
                .iter()
                .copied()
                .filter(|local| *local != completion_old),
        );
        if let Some(destination) = site.destination {
            saved.insert(destination);
        }
    }
    let mut saved: Vec<_> = saved.into_iter().collect();
    let frame::ConstructedFrame {
        class: frame_class,
        frame,
        layout: frame_layout,
        slots: frame_slots,
        saved_values: saved_value_by_local,
        failure_slot,
        failure_value,
    } = frame::construct(
        lowerer,
        module,
        coroutine,
        &body.locals,
        &completion_ty,
        &mut saved,
    );

    let driver = lowerer.functions.alloc(mir::Function {
        debug_source: None,
        gc_effect: mir::GcEffect::Managed,
        name: format!("{source_name}$drive"),
        params: Vec::new(),
        return_ty: step_ty.clone(),
        body: mir::Body::unreachable(Arena::new()),
    });
    lowerer.top_level.push(driver);
    let driver_identity = mir::CoroutineDriverIdentity::new(
        source_materialization,
        source_odr_group,
        source_signature,
    )
    .expect("a coroutine source has one persistent driver identity");
    let frame_local = body.locals.alloc(mir::Local {
        name: "$frame".to_string(),
        ty: mir::Type::Class(frame_class),
        mutable: false,
    });
    let dispatch_state = body.locals.alloc(mir::Local {
        name: "$dispatch_state".to_string(),
        ty: mir::Type::MachineScalar(mir::MachineScalarKind::CoroutineFrameState),
        mutable: false,
    });

    let protocol = lowerer.coroutine_protocol(module, &source_return);
    let continuation = match completion_ty {
        mir::Type::Interface(interface) => interface,
        _ => unreachable!("hidden completion has a concrete Continuation<R> type"),
    };
    assert_eq!(
        continuation,
        lowerer.interfaces.mir_id(protocol.continuation),
        "the hidden completion type matches the concrete coroutine protocol",
    );
    let outer_resume = crate::coroutine_registry::protocol_call(
        module,
        &lowerer.instances,
        &lowerer.interfaces,
        protocol.continuation_resume,
    );
    let outer_failure = crate::coroutine_registry::protocol_call(
        module,
        &lowerer.instances,
        &lowerer.interfaces,
        protocol.continuation_resume_with_exception,
    );

    sites.reverse();
    let mut resume_points = Vec::new();
    for site in sites {
        let generated = rewrite_site(
            lowerer,
            module,
            &mut body,
            frame_local,
            frame_class,
            frame,
            frame_layout,
            &frame_slots,
            &saved_value_by_local,
            failure_slot.clone(),
            failure_value,
            &step_ty,
            continuation,
            &outer_resume,
            &outer_failure,
            &source_name,
            driver,
            source_materialization,
            source_odr_group,
            site,
        );
        resume_points.push(generated.point);
    }
    frame::clear_consumed_context_marks(&mut body, frame_local, &frame_slots);
    resume_points.sort_by_key(|point| lowerer.coroutines.resume_points[*point].site());
    let mut dispatch_entries = resume_points
        .iter()
        .flat_map(|point| {
            let point = &lowerer.coroutines.resume_points[*point];
            [
                (point.success_state(), point.success().entry().block()),
                (point.failure_state(), point.failure().entry().block()),
            ]
        })
        .collect::<Vec<_>>();
    // Keep dispatch construction deterministic in the signed order of the
    // frozen state-word encoding: exceptional states precede positive
    // suspension-site ids. Entries are derived only from checked point
    // metadata; there is no second state-to-block source of truth.
    dispatch_entries.sort_by_key(|(state, _)| frame_state_value(*state).raw_bits() ^ (1_u64 << 63));

    let original_entry = body.entry;
    let initial = body.blocks.alloc(mir::BasicBlock {
        name: "coroutine.initial".to_string(),
        statements: source_params
            .iter()
            .map(|param| {
                restore_statement(
                    mir::Expr::local(frame_local, mir::Type::Class(frame_class)),
                    param.local,
                    &frame_slots[&param.local],
                )
            })
            .chain(std::iter::once(atomic_field_store(
                mir::Expr::local(frame_local, mir::Type::Class(frame_class)),
                frame_layout.state.field_index(),
                frame_state(STATE_RUNNING),
            )))
            .collect(),
        terminator: mir::Terminator::Goto(original_entry),
        unwind: None,
    });
    let invalid = protocol_error_block(lowerer, module, &mut body.locals, &mut body.blocks, None);
    let mut next = invalid;
    for (state, target) in dispatch_entries.iter().rev() {
        next = dispatch_block(&mut body.blocks, dispatch_state, *state, *target, next);
    }
    let entry = dispatch_block(
        &mut body.blocks,
        dispatch_state,
        STATE_INITIAL,
        initial,
        next,
    );
    body.entry = entry;

    lowerer.functions[driver].params = vec![
        mir::Param {
            name: "$frame".to_string(),
            ty: mir::Type::Class(frame_class),
            local: frame_local,
        },
        mir::Param {
            name: "$dispatch_state".to_string(),
            ty: mir::Type::MachineScalar(mir::MachineScalarKind::CoroutineFrameState),
            local: dispatch_state,
        },
    ];
    lowerer.functions[driver].body = body;

    let (wrapper_params, wrapper_locals, wrapper_param_map, wrapper_completion) =
        wrapper_params(&old_params);
    lowerer
        .local_values
        .remap_coroutine_function(function_id, driver, &wrapper_param_map);
    lowerer.functions[function_id].params = wrapper_params;
    lowerer.functions[function_id].body = wrapper_body(
        frame_class,
        frame_layout.task_storage,
        wrapper_locals,
        &saved,
        &frame_slots,
        &wrapper_param_map,
        wrapper_completion,
        failure_slot,
        driver,
        &step_ty,
    );
    lowerer.coroutines.functions[coroutine].lowering = mir::CoroutineLowering::StateMachine {
        frame,
        driver,
        driver_identity: Box::new(driver_identity),
        resume_points,
    };
}

const STATE_INITIAL: mir::CoroutineFrameState = mir::CoroutineFrameState::Initial;
const STATE_RUNNING: mir::CoroutineFrameState = mir::CoroutineFrameState::Running;
const STATE_COMPLETED: mir::CoroutineFrameState = mir::CoroutineFrameState::Completed;
const ADAPTER_REGISTERING: mir::CoroutineAdapterState = mir::CoroutineAdapterState::Registering;
const ADAPTER_WAITING: mir::CoroutineAdapterState = mir::CoroutineAdapterState::Waiting;
const ADAPTER_COMPLETING_SUCCESS: mir::CoroutineAdapterState =
    mir::CoroutineAdapterState::CompletingSuccess;
const ADAPTER_COMPLETING_FAILURE: mir::CoroutineAdapterState =
    mir::CoroutineAdapterState::CompletingFailure;
const ADAPTER_LATCHED_SUCCESS: mir::CoroutineAdapterState =
    mir::CoroutineAdapterState::LatchedSuccess;
const ADAPTER_LATCHED_FAILURE: mir::CoroutineAdapterState =
    mir::CoroutineAdapterState::LatchedFailure;
const ADAPTER_CONSUMED: mir::CoroutineAdapterState = mir::CoroutineAdapterState::Consumed;

fn suspended_state(state: mir::CoroutineSuspendStateId) -> mir::CoroutineFrameState {
    mir::CoroutineFrameState::Suspended(state)
}

fn failure_state(state: mir::CoroutineSuspendStateId) -> mir::CoroutineFrameState {
    mir::CoroutineFrameState::ResumeFailure(state)
}

#[derive(Clone)]
struct FrameSlot {
    field: u32,
    slot: mir::CoroutineSlotId,
    slot_ty: mir::Type,
    value_ty: mir::Type,
    empty: mir::MirVariantRef,
    value_payload: mir::MirVariantFieldRef,
}

#[derive(Clone, Copy)]
struct FrameLayout {
    state: mir::CoroutineFrameFieldRef,
    completion: mir::CoroutineFrameFieldRef,
    task: mir::CoroutineFrameFieldRef,
    task_storage: mir::ContextStorageType,
}

impl FrameSlot {
    fn new(
        field: u32,
        slot: mir::CoroutineSlotId,
        slot_ty: mir::Type,
        metadata: &mir::CoroutineSlot,
    ) -> Self {
        assert_eq!(slot_ty, mir::Type::Enum(metadata.enum_id(), Vec::new()));
        Self {
            field,
            slot,
            slot_ty,
            value_ty: metadata.value().clone(),
            empty: metadata.empty(),
            value_payload: metadata.value_payload(),
        }
    }
}

struct GeneratedSite {
    point: mir::CoroutineResumePointId,
}

fn freeze_pending_context(
    pending: crate::cfg::CleanupContext,
    saved_values: &HashMap<mir::LocalId, mir::CoroutineSavedValueId>,
) -> Vec<mir::CoroutinePendingTransfer> {
    use crate::cfg::CleanupTransfer;
    pending
        .0
        .into_iter()
        .map(|transfer| match transfer {
            CleanupTransfer::Fallthrough(target) => {
                mir::CoroutinePendingTransfer::Fallthrough(target)
            }
            CleanupTransfer::ReturnUnit => {
                mir::CoroutinePendingTransfer::Return(mir::CoroutineReturnTransfer::Unit)
            }
            CleanupTransfer::ReturnValue(value) => mir::CoroutinePendingTransfer::Return(
                mir::CoroutineReturnTransfer::Saved(saved_values[&value]),
            ),
            CleanupTransfer::Break(target) => mir::CoroutinePendingTransfer::Break(target),
            CleanupTransfer::Continue(target) => mir::CoroutinePendingTransfer::Continue(target),
            CleanupTransfer::ManagedThrow { exception, unwind } => {
                mir::CoroutinePendingTransfer::ManagedThrow(
                    mir::CoroutineManagedThrowTransfer::new(saved_values[&exception], unwind),
                )
            }
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn register_resume_point(
    lowerer: &mut Lowerer,
    frame: mir::CoroutineFrameId,
    site: mir::CoroutineSuspendStateId,
    result: mir::Type,
    adapter: &GeneratedAdapter,
    parents: Vec<mir::CoroutinePendingTransfer>,
    post: mir::BlockId,
    resume: mir::BlockId,
    failure: mir::BlockId,
    failure_value: mir::CoroutineFailureValueId,
    unwind: Option<mir::BlockId>,
) -> mir::CoroutineResumePointId {
    lowerer
        .coroutines
        .resume_points
        .alloc(mir::CoroutineResumePoint::new(
            frame,
            site,
            result,
            adapter.class,
            adapter.resume,
            adapter.resume_with_exception,
            parents,
            mir::CoroutineResumeSuccess::new(
                mir::CoroutineResumeEntryTarget::new(resume),
                mir::CoroutineCleanupFallthroughTarget::new(post),
            ),
            mir::CoroutineResumeFailure::new(
                mir::CoroutineResumeEntryTarget::new(failure),
                failure_value,
                unwind.map(mir::CoroutineUnwindTarget::new),
            ),
            adapter.identity.clone(),
        ))
}
