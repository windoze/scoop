//! Typed coroutine state-machine construction over normalized MIR CFG.

use std::collections::{HashMap, HashSet};

use la_arena::Arena;
use scoop_ast::Span;
use scoop_hir::concrete as hir;
use scoop_mir as mir;

use super::{GeneratedNominalLinkRole, Lowerer, generated_nominal_link_stem};

mod adapters;
mod construction;
mod intrinsics;
mod liveness;
mod sites;

use adapters::*;
use construction::*;
use intrinsics::*;
use liveness::*;
use sites::*;

#[derive(Clone)]
struct SuspendSite {
    block: mir::BlockId,
    statement: usize,
    destination: Option<mir::LocalId>,
    result: mir::Type,
    kind: SuspendKind,
    live_after: Vec<mir::LocalId>,
    pending: mir::CoroutinePendingContext,
    state: mir::CoroutineSuspendStateId,
    identity_path: hir::StructuralDefinitionPath,
}

#[derive(Clone)]
struct DiscoveredSuspendSite {
    block: mir::BlockId,
    statement: usize,
    destination: Option<mir::LocalId>,
    result: mir::Type,
    kind: SuspendKind,
    live_after: Vec<mir::LocalId>,
    pending: mir::CoroutinePendingContext,
}

#[derive(Clone, Copy)]
enum SuspendKind {
    Call,
    Intrinsic {
        register: mir::MonomorphizedFunctionId,
    },
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
        let sites = analyze_sites(lowerer, function, &lowerer.functions[function].body);
        clear_pending_contexts(&mut lowerer.functions[function].body);
        if sites.is_empty() {
            continue;
        }
        transform_function(lowerer, module, coroutine, sites);
    }
}

fn clear_pending_contexts(body: &mut mir::Body) {
    for (_, block) in body.blocks.iter_mut() {
        for statement in &mut block.statements {
            let mir::StatementKind::Call(effect) = &mut statement.kind else {
                continue;
            };
            let call = match effect {
                mir::CallEffect::Unit(call) | mir::CallEffect::Value { call, .. } => call,
            };
            call.pending = mir::CoroutinePendingContext::Root;
        }
    }
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
    let source_signature = lowerer
        .source_callables
        .get(function_id)
        .expect("a coroutine source has one callable materialization")
        .signature_record()
        .signature()
        .clone();
    let step_ty = lowerer.functions[function_id].return_ty.clone();
    let (source_name, source_symbol, old_params, mut body) = {
        let function = &mut lowerer.functions[function_id];
        (
            function.name.clone(),
            function.symbol.clone(),
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
    let throwable_ty =
        mir::Type::Class(lowerer.class_map[&module.exception_core.throwable.class()]);
    lowerer
        .source_exact_types
        .get(&throwable_ty)
        .expect("local-concrete HIR contains the Throwable exact type");

    let mut saved = HashSet::new();
    saved.extend(source_params.iter().map(|param| param.local));
    for site in &sites {
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
    let saved_identity_by_local = saved
        .iter()
        .map(|local| {
            (
                *local,
                lowerer
                    .local_values
                    .get(function_id, *local)
                    .unwrap_or_else(|| {
                        panic!(
                            "coroutine-saved local `{}` has no persistent value identity",
                            body.locals[*local].name
                        )
                    })
                    .clone(),
            )
        })
        .collect::<HashMap<_, _>>();
    let frame_identity = mir::CoroutineFrameIdentity::new(
        source_materialization,
        saved_identity_by_local.values().cloned().collect(),
        source_odr_group,
    )
    .expect("a coroutine frame has one persistent generated identity");
    saved.sort_by_key(|local| saved_identity_by_local[local].id());

    let mut frame_fields = vec![
        mir::Field {
            name: "state".to_string(),
            ty: mir::Type::MachineScalar(mir::MachineScalarKind::CoroutineFrameState),
        },
        mir::Field {
            name: "completion".to_string(),
            ty: completion_ty.clone(),
        },
    ];
    let mut frame_slots = HashMap::new();
    for local in &saved {
        let value_ty = body.locals[*local].ty.clone();
        let (slot_id, slot_ty) = lowerer.coroutines.slot_for(
            &lowerer.source_exact_types,
            &value_ty,
            &lowerer.structs,
            &mut lowerer.enums,
            &mut lowerer.shell,
        );
        let field = frame_fields.len() as u32;
        frame_fields.push(mir::Field {
            name: format!("local${}", body.locals[*local].name),
            ty: slot_ty.clone(),
        });
        frame_slots.insert(
            *local,
            FrameSlot::new(
                field,
                slot_id,
                slot_ty.clone(),
                &lowerer.coroutines.slots[slot_id],
            ),
        );
    }
    let (failure_slot_id, failure_slot_ty) = lowerer.coroutines.slot_for(
        &lowerer.source_exact_types,
        &throwable_ty,
        &lowerer.structs,
        &mut lowerer.enums,
        &mut lowerer.shell,
    );
    let failure_slot = FrameSlot::new(
        frame_fields.len() as u32,
        failure_slot_id,
        failure_slot_ty.clone(),
        &lowerer.coroutines.slots[failure_slot_id],
    );
    frame_fields.push(mir::Field {
        name: "failure".to_string(),
        ty: failure_slot_ty,
    });
    let frame_name = format!("CoroutineFrame${}", encode_symbol_component(&source_symbol));
    let frame_class = generated_class(
        lowerer,
        GeneratedNominalLinkRole::CoroutineFrame {
            source_symbol: &source_symbol,
        },
        frame_name,
        frame_fields,
        Vec::new(),
        Vec::new(),
    );
    let state_field = mir::CoroutineFrameFieldRef::checked(&lowerer.classes, frame_class, 0)
        .expect("the generated coroutine frame has a state field");
    let completion_field = mir::CoroutineFrameFieldRef::checked(&lowerer.classes, frame_class, 1)
        .expect("the generated coroutine frame has a completion field");
    let frame_layout = FrameLayout {
        state: state_field,
        completion: completion_field,
    };
    let mut saved_value_by_local = HashMap::new();
    let mut saved_values = Vec::with_capacity(saved.len());
    for local in &saved {
        let slot = &frame_slots[local];
        let field = mir::CoroutineFrameFieldRef::checked(&lowerer.classes, frame_class, slot.field)
            .expect("each generated coroutine saved slot has a typed frame field");
        let metadata = mir::CoroutineSavedValue::checked(
            &lowerer.classes,
            &lowerer.coroutines.slots,
            field,
            slot.slot,
        )
        .expect("each generated coroutine saved slot has its exact value type");
        let value = lowerer.coroutines.saved_values.alloc(metadata);
        saved_value_by_local.insert(*local, value);
        saved_values.push(value);
    }
    let failure_field =
        mir::CoroutineFrameFieldRef::checked(&lowerer.classes, frame_class, failure_slot.field)
            .expect("the generated coroutine frame has a typed failure field");
    let mir::Type::Class(throwable_class) = &throwable_ty else {
        unreachable!("the coroutine failure slot carries the canonical Throwable class")
    };
    let throwable_class = *throwable_class;
    let failure_metadata = mir::CoroutineFailureValue::checked(
        &lowerer.classes,
        &lowerer.coroutines.slots,
        failure_field,
        failure_slot.slot,
        throwable_class,
    )
    .expect("the generated coroutine failure slot carries exact Throwable");
    let failure_value = lowerer.coroutines.failure_values.alloc(failure_metadata);
    let frame_metadata = mir::CoroutineFrame::checked(
        &lowerer.classes,
        &lowerer.coroutines.saved_values,
        &lowerer.coroutines.failure_values,
        frame_class,
        coroutine,
        state_field,
        completion_field,
        saved_values,
        failure_value,
        frame_identity,
    )
    .expect("the generated coroutine frame has disjoint typed field roles");
    let frame = lowerer.coroutines.frames.alloc(frame_metadata);

    let driver = lowerer.functions.alloc(mir::Function {
        gc_effect: mir::GcEffect::Managed,
        name: format!("{source_name}$drive"),
        symbol: format!("{source_symbol}$drive"),
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
    let (outer_resume, outer_failure) = lowerer.coroutines.continuation_shells(
        &lowerer.source_exact_types,
        &source_return,
        crate::source_callables::exact_function_signature(module, protocol.continuation_resume),
        crate::source_callables::exact_function_signature(
            module,
            protocol.continuation_resume_with_exception,
        ),
        continuation,
        throwable_ty,
        &mut lowerer.functions,
        &lowerer.shell,
    );

    sites.sort_by_key(|site| (raw(site.block), site.statement));
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
            outer_resume,
            outer_failure,
            &source_symbol,
            driver,
            source_materialization,
            source_odr_group,
            site,
        );
        resume_points.push(generated.point);
    }
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
    pending: mir::CoroutinePendingContext,
    saved_values: &HashMap<mir::LocalId, mir::CoroutineSavedValueId>,
) -> Vec<mir::CoroutinePendingTransfer> {
    let mir::CoroutinePendingContext::Chain(chain) = pending else {
        return Vec::new();
    };
    chain
        .into_vec()
        .into_iter()
        .map(|transfer| match transfer {
            mir::CoroutinePendingSourceTransfer::Fallthrough(target) => {
                mir::CoroutinePendingTransfer::Fallthrough(target)
            }
            mir::CoroutinePendingSourceTransfer::Return(
                mir::CoroutinePendingSourceReturn::Unit,
            ) => mir::CoroutinePendingTransfer::Return(mir::CoroutineReturnTransfer::Unit),
            mir::CoroutinePendingSourceTransfer::Return(
                mir::CoroutinePendingSourceReturn::Value(value),
            ) => mir::CoroutinePendingTransfer::Return(mir::CoroutineReturnTransfer::Saved(
                *saved_values
                    .get(&value.local())
                    .expect("a pending return value is saved in its owner frame"),
            )),
            mir::CoroutinePendingSourceTransfer::Break(target) => {
                mir::CoroutinePendingTransfer::Break(target)
            }
            mir::CoroutinePendingSourceTransfer::Continue(target) => {
                mir::CoroutinePendingTransfer::Continue(target)
            }
            mir::CoroutinePendingSourceTransfer::ManagedThrow(throw_) => {
                mir::CoroutinePendingTransfer::ManagedThrow(
                    mir::CoroutineManagedThrowTransfer::new(
                        *saved_values
                            .get(&throw_.exception_local())
                            .expect("a pending managed Throwable is saved in its owner frame"),
                        throw_.unwind(),
                    ),
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
