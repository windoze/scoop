use std::collections::{BTreeMap, HashMap};
use std::fmt::Write;
use std::path::Path;

use crate::effect_facts::{CaseTag, ConcreteOpKey, EffectFamilyKey, ImplPlan};
use crate::stable_id::stable_dump_label;
use crate::ty::{EffectRow, TypeId};

use super::ir::{
    BoundarySiteKind, LateLoweredBodyVersionKey, LateLoweredBoundary, LateLoweredBoundaryLowering,
    LateLoweredBoundarySource, LateLoweredBoundarySourceConsumption,
    LateLoweredCallBoundaryOperandContract, LateLoweredCallable, LateLoweredCallableAbi,
    LateLoweredCompleteStepDispatch, LateLoweredCompletionPayloadBinding,
    LateLoweredCompletionPayloadSource, LateLoweredConsumedRuntimeErrorCase,
    LateLoweredContinuationCapture, LateLoweredContinuationMethod, LateLoweredContinuationObject,
    LateLoweredContinuationResumeBody, LateLoweredContinuationSurfaceResume,
    LateLoweredFrameSchema, LateLoweredFrameSlot, LateLoweredFrameSlotKind,
    LateLoweredHandleBoundaryCaseRoutingAction, LateLoweredHandleDispatchContract,
    LateLoweredHandlePendingCompletion, LateLoweredHandleStateRegion,
    LateLoweredLocalRuntimeErrorTerminalAction, LateLoweredOneShotPolicy, LateLoweredOperandSource,
    LateLoweredOperandValueSource, LateLoweredPerformBoundaryOperandContract,
    LateLoweredPlainBodySlice, LateLoweredPlainCallSite, LateLoweredPlainCallable,
    LateLoweredProgram, LateLoweredPublishedRuntimeEntry, LateLoweredResumeBoundaryOperandContract,
    LateLoweredResumeInterface, LateLoweredResumeMethod, LateLoweredResumePayloadBinding,
    LateLoweredResumeStateMap, LateLoweredSourceStatementClassification,
    LateLoweredSourceStatementClassificationKind, LateLoweredState, LateLoweredStateRole,
    LateLoweredStateSlice, LateLoweredStateTerminator, LateLoweredStepCase,
    LateLoweredStepCaseEmission, LateLoweredStepCaseForwarding, LateLoweredStepDispatchPlan,
    LateLoweredStepType, LateLoweredSurfaceResumeDispatchInventoryEntry,
    LateLoweredSurfaceResumeDispatchPublication, LateLoweredSurfaceResumeDispatchSourceKind,
    LateLoweredSurfaceResumeWrapperCaseProjection,
    LateLoweredSurfaceResumeWrapperCompletePayloadSource,
    LateLoweredSurfaceResumeWrapperCompleteProjection, LateLoweredSurfaceResumeWrapperProjection,
    ResumeInterfaceId, StateId, SystemSlotKind,
};

#[derive(Default)]
struct CallableDumpLabels {
    states: BTreeMap<StateId, String>,
    boundaries: BTreeMap<super::ir::BoundaryId, String>,
    slots: BTreeMap<super::ir::FrameSlotId, String>,
}

struct DumpCtx<'a> {
    program: &'a LateLoweredProgram,
    step_labels: BTreeMap<crate::effect_facts::StepSchemaId, String>,
    continuation_labels: BTreeMap<crate::effect_facts::ContinuationSchemaId, String>,
    case_labels: BTreeMap<(crate::effect_facts::StepSchemaId, CaseTag), String>,
    resume_packing_labels: BTreeMap<ResumeInterfaceId, String>,
    continuation_object_labels: BTreeMap<super::ir::ContinuationObjectId, String>,
    callable_labels: HashMap<LateLoweredBodyVersionKey, CallableDumpLabels>,
}

impl<'a> DumpCtx<'a> {
    fn new(program: &'a LateLoweredProgram) -> Self {
        let step_owner_roots = program
            .callables()
            .iter()
            .filter_map(|callable| {
                callable
                    .body_step_schema()
                    .map(|step| (step, callable.root_fqn()))
            })
            .fold(
                BTreeMap::<crate::effect_facts::StepSchemaId, Vec<String>>::new(),
                |mut acc, (step, root)| {
                    acc.entry(step).or_default().push(root.to_string());
                    acc
                },
            );

        let continuation_object_labels = program
            .continuation_objects()
            .iter()
            .map(|object| {
                let canonical = format!(
                    "owner={}|continuation_obj_ty={}",
                    body_version_identity(program, object.owner_version_key()),
                    type_text(program, object.continuation_obj_ty()),
                );
                (
                    object.object_id(),
                    stable_dump_label("cont_obj", &canonical),
                )
            })
            .collect::<BTreeMap<_, _>>();

        let resume_packing_labels = program
            .resume_packings()
            .iter()
            .map(|interface| {
                let methods = interface
                    .methods()
                    .iter()
                    .map(|method| concrete_op_text(program, method.concrete_op_key()))
                    .collect::<Vec<_>>()
                    .join(" | ");
                let owners = step_owner_roots
                    .get(&interface.return_step_schema())
                    .cloned()
                    .unwrap_or_else(|| vec!["unowned".to_string()])
                    .join(", ");
                let canonical = format!(
                    "effect_family={}|step_owners=[{}]|methods=[{}]",
                    effect_family_text(program, interface.effect_family()),
                    owners,
                    methods,
                );
                (
                    interface.interface_id(),
                    stable_dump_label("packing", &canonical),
                )
            })
            .collect::<BTreeMap<_, _>>();

        let continuation_labels = program
            .surface_resume_dispatch_inventory()
            .iter()
            .map(|entry| {
                let contract = entry.contract();
                let publications = entry
                    .publications()
                    .iter()
                    .map(|publication| publication_identity(program, publication))
                    .collect::<Vec<_>>()
                    .join(" | ");
                let owners = step_owner_roots
                    .get(&contract.out_step_schema())
                    .cloned()
                    .unwrap_or_else(|| vec!["unowned".to_string()])
                    .join(", ");
                let canonical = format!(
                    "source={:?}|resume={}|answer={}|out_step_owners=[{}]|publications=[{}]",
                    entry.source_kind(),
                    type_text(program, contract.resume_tuple_ty()),
                    type_text(program, contract.answer_ty()),
                    owners,
                    publications,
                );
                (
                    entry.continuation_schema(),
                    stable_dump_label("cont", &canonical),
                )
            })
            .collect::<BTreeMap<_, _>>();

        let case_labels = program
            .step_types()
            .iter()
            .flat_map(|step_type| {
                step_type.cases().iter().map(|case| {
                    let owners = step_owner_roots
                        .get(&step_type.step_schema())
                        .cloned()
                        .unwrap_or_else(|| vec!["unowned".to_string()])
                        .join(", ");
                    let continuation = continuation_labels
                        .get(&case.continuation_schema())
                        .cloned()
                        .unwrap_or_else(|| "cont_missing".to_string());
                    let canonical = format!(
                        "step_owners=[{}]|op={}|payload={}|continuation={}",
                        owners,
                        concrete_op_text(program, case.concrete_op_key()),
                        type_text(program, case.payload_tuple_ty()),
                        continuation,
                    );
                    (
                        (step_type.step_schema(), case.case_tag()),
                        stable_dump_label("case", &canonical),
                    )
                })
            })
            .collect::<BTreeMap<_, _>>();

        let step_labels = program
            .step_types()
            .iter()
            .map(|step_type| {
                let owners = step_owner_roots
                    .get(&step_type.step_schema())
                    .cloned()
                    .unwrap_or_else(|| vec!["unowned".to_string()])
                    .join(", ");
                let cases = step_type
                    .cases()
                    .iter()
                    .map(|case| {
                        format!(
                            "{}={}",
                            case_labels
                                .get(&(step_type.step_schema(), case.case_tag()))
                                .cloned()
                                .unwrap_or_else(|| "case_missing".to_string()),
                            concrete_op_text(program, case.concrete_op_key()),
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" | ");
                let canonical = format!(
                    "owners=[{}]|invoke={}|complete={}|continuation_obj={}|cases=[{}]",
                    owners,
                    type_text(program, step_type.invoke_args_tuple_ty()),
                    type_text(program, step_type.complete_ty()),
                    type_text(program, step_type.continuation_obj_ty()),
                    cases,
                );
                (
                    step_type.step_schema(),
                    stable_dump_label("step", &canonical),
                )
            })
            .collect::<BTreeMap<_, _>>();

        let callable_labels = program
            .callables()
            .iter()
            .filter(|callable| callable.has_control_body())
            .map(|callable| {
                (
                    callable.body_version_key().clone(),
                    build_callable_labels(program, callable, &step_labels, &case_labels),
                )
            })
            .collect::<HashMap<_, _>>();

        Self {
            program,
            step_labels,
            continuation_labels,
            case_labels,
            resume_packing_labels,
            continuation_object_labels,
            callable_labels,
        }
    }

    fn type_text(&self, ty: TypeId) -> String {
        type_text(self.program, ty)
    }

    fn step_label(&self, step_schema: crate::effect_facts::StepSchemaId) -> String {
        self.step_labels
            .get(&step_schema)
            .cloned()
            .unwrap_or_else(|| "step_missing".to_string())
    }

    fn continuation_label(
        &self,
        continuation_schema: crate::effect_facts::ContinuationSchemaId,
    ) -> String {
        self.continuation_labels
            .get(&continuation_schema)
            .cloned()
            .unwrap_or_else(|| "cont_missing".to_string())
    }

    fn case_ref(
        &self,
        step_schema: crate::effect_facts::StepSchemaId,
        case_tag: CaseTag,
    ) -> String {
        let label = self
            .case_labels
            .get(&(step_schema, case_tag))
            .cloned()
            .unwrap_or_else(|| "case_missing".to_string());
        format!(
            "{}={}",
            label,
            case_op_text(self.program, step_schema, case_tag)
        )
    }

    fn resume_packing_label(&self, interface_id: ResumeInterfaceId) -> String {
        self.resume_packing_labels
            .get(&interface_id)
            .cloned()
            .unwrap_or_else(|| "packing_missing".to_string())
    }

    fn continuation_object_label(&self, object_id: super::ir::ContinuationObjectId) -> String {
        self.continuation_object_labels
            .get(&object_id)
            .cloned()
            .unwrap_or_else(|| "cont_obj_missing".to_string())
    }

    fn local_label(&self, key: &LateLoweredBodyVersionKey, local: crate::mir::LocalId) -> String {
        self.program
            .dump_body_labels(key)
            .map(|labels| labels.local_label(local))
            .unwrap_or_else(|| "local_missing".to_string())
    }

    fn block_label(
        &self,
        key: &LateLoweredBodyVersionKey,
        block: crate::mir::BasicBlockId,
    ) -> String {
        self.program
            .dump_body_labels(key)
            .map(|labels| labels.block_label(block))
            .unwrap_or_else(|| "bb_missing".to_string())
    }

    fn site_label(&self, key: &LateLoweredBodyVersionKey, site: crate::mir::SiteId) -> String {
        self.program
            .dump_body_labels(key)
            .map(|labels| labels.site_label(site))
            .unwrap_or_else(|| "site_missing".to_string())
    }

    fn state_label(&self, key: &LateLoweredBodyVersionKey, state: StateId) -> String {
        self.callable_labels
            .get(key)
            .and_then(|labels| labels.states.get(&state))
            .cloned()
            .unwrap_or_else(|| "state_missing".to_string())
    }

    fn boundary_label(
        &self,
        key: &LateLoweredBodyVersionKey,
        boundary: super::ir::BoundaryId,
    ) -> String {
        self.callable_labels
            .get(key)
            .and_then(|labels| labels.boundaries.get(&boundary))
            .cloned()
            .unwrap_or_else(|| "boundary_missing".to_string())
    }

    fn slot_label(&self, key: &LateLoweredBodyVersionKey, slot: super::ir::FrameSlotId) -> String {
        self.callable_labels
            .get(key)
            .and_then(|labels| labels.slots.get(&slot))
            .cloned()
            .unwrap_or_else(|| "slot_missing".to_string())
    }
}

fn type_text(program: &LateLoweredProgram, ty: TypeId) -> String {
    program
        .dump_type_text(ty)
        .map(|text| normalize_display_text(text.to_string()))
        .unwrap_or_else(|| "<invalid-type>".to_string())
}

fn normalize_display_text(text: String) -> String {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let Some(workspace_root) = manifest_dir.parent().and_then(Path::parent) else {
        return text;
    };
    let prefix = format!("{}/", workspace_root.display());
    text.replace(&prefix, "")
}

fn effect_row_text(program: &LateLoweredProgram, row: &EffectRow) -> String {
    if row.is_pure() {
        return "Pure".to_string();
    }
    let rendered = row
        .terms
        .iter()
        .copied()
        .map(|ty| type_text(program, ty))
        .collect::<Vec<_>>()
        .join(" + ");
    format!("({rendered})")
}

fn instance_text(program: &LateLoweredProgram, key: &crate::mir::InstanceKey) -> String {
    let mut args = key
        .type_args
        .iter()
        .copied()
        .map(|ty| type_text(program, ty))
        .collect::<Vec<_>>();
    args.extend(
        key.eff_args
            .iter()
            .map(|row| format!("eff {}", effect_row_text(program, row))),
    );
    if args.is_empty() {
        key.template.fqn.clone()
    } else {
        format!("{}<{}>", key.template.fqn, args.join(", "))
    }
}

fn concrete_op_text(program: &LateLoweredProgram, key: &ConcreteOpKey) -> String {
    instance_text(program, key.instance_key())
}

fn effect_family_text(program: &LateLoweredProgram, key: &EffectFamilyKey) -> String {
    if key.type_args().is_empty() {
        return key.effect_fqn().to_string();
    }
    format!(
        "{}<{}>",
        key.effect_fqn(),
        key.type_args()
            .iter()
            .copied()
            .map(|ty| type_text(program, ty))
            .collect::<Vec<_>>()
            .join(", "),
    )
}

fn body_version_identity(program: &LateLoweredProgram, key: &LateLoweredBodyVersionKey) -> String {
    let impl_plan = match key.impl_plan() {
        ImplPlan::NoOutward => "NoOutward".to_string(),
        ImplPlan::CanonicalFull => "CanonicalFull".to_string(),
        ImplPlan::SingleCase(tag) => program
            .callable_by_version_key(key)
            .and_then(|callable| {
                callable
                    .body_step_schema()
                    .map(|step| case_op_text(program, step, tag))
            })
            .map(|case| format!("SingleCase({case})"))
            .unwrap_or_else(|| "SingleCase(case_missing)".to_string()),
    };
    format!(
        "instance={} allowed_row={} impl_plan={} needs_reentry={}",
        instance_text(program, key.surface_instance()),
        effect_row_text(program, key.allowed_row()),
        impl_plan,
        key.needs_reentry(),
    )
}

fn case_op_text(
    program: &LateLoweredProgram,
    step_schema: crate::effect_facts::StepSchemaId,
    case_tag: CaseTag,
) -> String {
    program
        .step_type(step_schema)
        .and_then(|step_type| step_type.case(case_tag))
        .map(|case| concrete_op_text(program, case.concrete_op_key()))
        .unwrap_or_else(|| "missing_case".to_string())
}

fn publication_identity(
    program: &LateLoweredProgram,
    publication: &LateLoweredSurfaceResumeDispatchPublication,
) -> String {
    match publication {
        LateLoweredSurfaceResumeDispatchPublication::SurfaceCase {
            object_id,
            case_tag,
            reachability,
        } => {
            let owner = program
                .continuation_object(*object_id)
                .map(|object| body_version_identity(program, object.owner_version_key()))
                .unwrap_or_else(|| "missing_object_owner".to_string());
            let op = program
                .continuation_object(*object_id)
                .and_then(|object| {
                    object
                        .surface_resumes()
                        .iter()
                        .find(|resume| resume.case_tag() == *case_tag)
                })
                .map(|resume| concrete_op_text(program, resume.concrete_op_key()))
                .unwrap_or_else(|| "missing_case".to_string());
            format!("surface_case owner={owner} op={op} reachability={reachability:?}")
        }
        LateLoweredSurfaceResumeDispatchPublication::InternalMethod {
            object_id,
            packing_interface_id,
            case_tag,
            reachability,
        } => {
            let owner = program
                .continuation_object(*object_id)
                .map(|object| body_version_identity(program, object.owner_version_key()))
                .unwrap_or_else(|| "missing_object_owner".to_string());
            let op = program
                .continuation_object(*object_id)
                .and_then(|object| {
                    object.methods().iter().find(|method| {
                        method.case_tag() == *case_tag
                            && method.packing_interface_id() == *packing_interface_id
                    })
                })
                .map(|method| concrete_op_text(program, method.concrete_op_key()))
                .unwrap_or_else(|| "missing_method".to_string());
            let packing = program
                .resume_packing(*packing_interface_id)
                .map(|interface| effect_family_text(program, interface.effect_family()))
                .unwrap_or_else(|| "missing_packing".to_string());
            format!(
                "internal_method owner={owner} packing={packing} op={op} reachability={reachability:?}"
            )
        }
        LateLoweredSurfaceResumeDispatchPublication::ResumeBoundary {
            owner_version_key,
            site_id,
            ..
        } => format!(
            "resume_boundary {} {}",
            body_version_identity(program, owner_version_key),
            program
                .dump_body_labels(owner_version_key)
                .map(|labels| labels.site_label(*site_id))
                .unwrap_or_else(|| "site_missing".to_string())
        ),
        LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
            owner_version_key,
            site_id,
            arm_ordinal,
            handled_case,
            ..
        } => {
            let handled = program
                .callable_by_version_key(owner_version_key)
                .and_then(|callable| {
                    callable
                        .body_step_schema()
                        .map(|step| case_op_text(program, step, *handled_case))
                })
                .unwrap_or_else(|| "missing_case".to_string());
            format!(
                "handle_continuation_binder {} {} arm#{} handled={}",
                body_version_identity(program, owner_version_key),
                program
                    .dump_body_labels(owner_version_key)
                    .map(|labels| labels.site_label(*site_id))
                    .unwrap_or_else(|| "site_missing".to_string()),
                arm_ordinal,
                handled,
            )
        }
    }
}

fn projection_owner_callable<'a>(
    ctx: &'a DumpCtx<'_>,
    projection: &LateLoweredSurfaceResumeWrapperProjection,
) -> Option<&'a LateLoweredCallable> {
    match projection.underlying_route().publication() {
        LateLoweredSurfaceResumeDispatchPublication::ResumeBoundary {
            owner_version_key, ..
        }
        | LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
            owner_version_key,
            ..
        } => ctx.program.callable_by_version_key(owner_version_key),
        LateLoweredSurfaceResumeDispatchPublication::SurfaceCase { object_id, .. }
        | LateLoweredSurfaceResumeDispatchPublication::InternalMethod { object_id, .. } => ctx
            .program
            .continuation_object(*object_id)
            .and_then(|object| {
                ctx.program
                    .callable_by_version_key(object.owner_version_key())
            }),
    }
}

fn build_callable_labels(
    program: &LateLoweredProgram,
    callable: &LateLoweredCallable,
    step_labels: &BTreeMap<crate::effect_facts::StepSchemaId, String>,
    case_labels: &BTreeMap<(crate::effect_facts::StepSchemaId, CaseTag), String>,
) -> CallableDumpLabels {
    let body_key = callable.body_version_key();
    let Some(state_graph) = callable
        .effect_step_abi()
        .map(|effect| effect.state_graph())
        .or_else(|| {
            callable
                .plain_local_effect_control()
                .map(|local| local.state_graph())
        })
    else {
        return CallableDumpLabels::default();
    };
    let Some(frame_schema) = callable
        .effect_step_abi()
        .map(|effect| effect.frame_schema())
        .or_else(|| {
            callable
                .plain_local_effect_control()
                .map(|local| local.frame_schema())
        })
    else {
        return CallableDumpLabels::default();
    };
    let Some(boundary_map) = callable
        .effect_step_abi()
        .map(|effect| effect.boundary_map())
        .or_else(|| {
            callable
                .plain_local_effect_control()
                .map(|local| local.boundary_map())
        })
    else {
        return CallableDumpLabels::default();
    };
    let mut state_signatures = HashMap::<String, usize>::new();
    let states = state_graph
        .states()
        .iter()
        .map(|state| {
            let signature = format!(
                "role={:?}|slices=[{}]|terminator={}",
                state.role(),
                state
                    .source_slices()
                    .iter()
                    .map(|slice| state_slice_identity(program, body_key, *slice))
                    .collect::<Vec<_>>()
                    .join(" | "),
                state_terminator_kind_name(state.terminator()),
            );
            let ordinal = next_ordinal(&mut state_signatures, &signature);
            (
                state.state_id(),
                stable_dump_label(
                    "state",
                    &format!(
                        "owner={}|signature={signature}|ordinal={ordinal}",
                        body_version_identity(program, body_key)
                    ),
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();

    let mut boundary_signatures = HashMap::<String, usize>::new();
    let boundaries = boundary_map
        .entries()
        .iter()
        .map(|boundary| {
            let signature = format!(
                "source={}|owner={}|resume={}|lowering={}",
                boundary_source_identity(program, body_key, boundary.source()),
                states
                    .get(&boundary.owner_state())
                    .cloned()
                    .unwrap_or_else(|| "state_missing".to_string()),
                states
                    .get(&boundary.resume_state())
                    .cloned()
                    .unwrap_or_else(|| "state_missing".to_string()),
                boundary
                    .lowering()
                    .map(boundary_lowering_kind_name)
                    .unwrap_or("None"),
            );
            let ordinal = next_ordinal(&mut boundary_signatures, &signature);
            (
                boundary.boundary_id(),
                stable_dump_label(
                    "boundary",
                    &format!(
                        "owner={}|signature={signature}|ordinal={ordinal}",
                        body_version_identity(program, body_key)
                    ),
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();

    let mut slot_signatures = HashMap::<String, usize>::new();
    let slots = frame_schema
        .slots()
        .iter()
        .map(|slot| {
            let signature = format!(
                "kind={}|ty={}|writes=[{}]|reads=[{}]",
                frame_slot_kind_identity(
                    program,
                    callable,
                    slot.kind(),
                    &states,
                    &boundaries,
                    step_labels,
                    case_labels
                ),
                type_text(program, slot.ty()),
                slot.write_points()
                    .iter()
                    .map(|state| states
                        .get(state)
                        .cloned()
                        .unwrap_or_else(|| "state_missing".to_string()))
                    .collect::<Vec<_>>()
                    .join(", "),
                slot.read_points()
                    .iter()
                    .map(|state| states
                        .get(state)
                        .cloned()
                        .unwrap_or_else(|| "state_missing".to_string()))
                    .collect::<Vec<_>>()
                    .join(", "),
            );
            let ordinal = next_ordinal(&mut slot_signatures, &signature);
            (
                slot.slot_id(),
                stable_dump_label(
                    "slot",
                    &format!(
                        "owner={}|signature={signature}|ordinal={ordinal}",
                        body_version_identity(program, body_key)
                    ),
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();

    CallableDumpLabels {
        states,
        boundaries,
        slots,
    }
}

fn state_slice_identity(
    program: &LateLoweredProgram,
    key: &LateLoweredBodyVersionKey,
    slice: LateLoweredStateSlice,
) -> String {
    let term = if slice.includes_terminator() {
        "+term"
    } else {
        ""
    };
    format!(
        "{}:{}..{}{}",
        program
            .dump_body_labels(key)
            .map(|labels| labels.block_label(slice.block_id()))
            .unwrap_or_else(|| "bb_missing".to_string()),
        slice.start_statement_index(),
        slice.end_statement_index(),
        term,
    )
}

fn boundary_source_identity(
    program: &LateLoweredProgram,
    key: &LateLoweredBodyVersionKey,
    source: LateLoweredBoundarySource,
) -> String {
    match source {
        LateLoweredBoundarySource::Site { site_id, kind } => format!(
            "{:?}({})",
            kind,
            program
                .dump_body_labels(key)
                .map(|labels| labels.site_label(site_id))
                .unwrap_or_else(|| "site_missing".to_string())
        ),
        LateLoweredBoundarySource::RuntimeError { origin_site } => format!(
            "RuntimeError({})",
            program
                .dump_body_labels(key)
                .map(|labels| labels.site_label(origin_site))
                .unwrap_or_else(|| "site_missing".to_string())
        ),
    }
}

fn frame_slot_kind_identity(
    program: &LateLoweredProgram,
    callable: &LateLoweredCallable,
    kind: LateLoweredFrameSlotKind,
    _states: &BTreeMap<StateId, String>,
    boundaries: &BTreeMap<super::ir::BoundaryId, String>,
    step_labels: &BTreeMap<crate::effect_facts::StepSchemaId, String>,
    case_labels: &BTreeMap<(crate::effect_facts::StepSchemaId, CaseTag), String>,
) -> String {
    let body_key = callable.body_version_key();
    match kind {
        LateLoweredFrameSlotKind::SourceLocal(local) => format!(
            "SourceLocal({})",
            program
                .dump_body_labels(body_key)
                .map(|labels| labels.local_label(local))
                .unwrap_or_else(|| "local_missing".to_string())
        ),
        LateLoweredFrameSlotKind::CompilerTemporary(local) => format!(
            "CompilerTemporary({})",
            program
                .dump_body_labels(body_key)
                .map(|labels| labels.local_label(local))
                .unwrap_or_else(|| "local_missing".to_string())
        ),
        LateLoweredFrameSlotKind::JoinValue {
            local,
            block,
            ordinal,
        } => format!(
            "JoinValue({}, {}, #{ordinal})",
            program
                .dump_body_labels(body_key)
                .map(|labels| labels.local_label(local))
                .unwrap_or_else(|| "local_missing".to_string()),
            program
                .dump_body_labels(body_key)
                .map(|labels| labels.block_label(block))
                .unwrap_or_else(|| "bb_missing".to_string())
        ),
        LateLoweredFrameSlotKind::HandleBinder {
            site_id,
            local,
            ordinal,
        } => format!(
            "HandleBinder({}, {}, #{ordinal})",
            program
                .dump_body_labels(body_key)
                .map(|labels| labels.site_label(site_id))
                .unwrap_or_else(|| "site_missing".to_string()),
            program
                .dump_body_labels(body_key)
                .map(|labels| labels.local_label(local))
                .unwrap_or_else(|| "local_missing".to_string())
        ),
        LateLoweredFrameSlotKind::HandleSavedEffectCtx { site_id } => format!(
            "HandleSavedEffectCtx({})",
            program
                .dump_body_labels(body_key)
                .map(|labels| labels.site_label(site_id))
                .unwrap_or_else(|| "site_missing".to_string())
        ),
        LateLoweredFrameSlotKind::HandleArmEffectCtx {
            site_id,
            arm_ordinal,
        } => format!(
            "HandleArmEffectCtx({}, arm#{arm_ordinal})",
            program
                .dump_body_labels(body_key)
                .map(|labels| labels.site_label(site_id))
                .unwrap_or_else(|| "site_missing".to_string())
        ),
        LateLoweredFrameSlotKind::HandlePendingPayload { site_id, case_tag } => {
            let step_schema = callable.body_step_schema();
            let case = step_schema
                .and_then(|step| case_labels.get(&(step, case_tag)).cloned())
                .unwrap_or_else(|| "case_missing".to_string());
            format!(
                "HandlePendingPayload({}, {})",
                program
                    .dump_body_labels(body_key)
                    .map(|labels| labels.site_label(site_id))
                    .unwrap_or_else(|| "site_missing".to_string()),
                case,
            )
        }
        LateLoweredFrameSlotKind::ResumePayload { boundary, case_tag } => {
            let boundary_label = boundaries
                .get(&boundary)
                .cloned()
                .unwrap_or_else(|| "boundary_missing".to_string());
            let case = continuation_owner_case_ref(
                program,
                callable,
                boundary,
                case_tag,
                step_labels,
                case_labels,
            );
            format!("ResumePayload({boundary_label}, {case})")
        }
        LateLoweredFrameSlotKind::BoundaryResult { boundary, local } => format!(
            "BoundaryResult({}, {})",
            boundaries
                .get(&boundary)
                .cloned()
                .unwrap_or_else(|| "boundary_missing".to_string()),
            program
                .dump_body_labels(body_key)
                .map(|labels| labels.local_label(local))
                .unwrap_or_else(|| "local_missing".to_string())
        ),
        LateLoweredFrameSlotKind::System(system) => format!("System({system:?})"),
    }
}

fn continuation_owner_case_ref(
    program: &LateLoweredProgram,
    callable: &LateLoweredCallable,
    boundary: super::ir::BoundaryId,
    case_tag: CaseTag,
    _step_labels: &BTreeMap<crate::effect_facts::StepSchemaId, String>,
    case_labels: &BTreeMap<(crate::effect_facts::StepSchemaId, CaseTag), String>,
) -> String {
    let Some(boundary) = callable.boundary_map().boundary(boundary) else {
        return "case_missing".to_string();
    };
    let step_schema = match boundary.lowering() {
        Some(LateLoweredBoundaryLowering::Perform(lowering)) => lowering
            .emitted_step()
            .continuation_contract()
            .out_step_schema(),
        Some(LateLoweredBoundaryLowering::Call(lowering)) => {
            lowering.dispatch().input_step_schema()
        }
        Some(LateLoweredBoundaryLowering::Resume(lowering)) => {
            lowering.dispatch().input_step_schema()
        }
        Some(LateLoweredBoundaryLowering::ClassCtor(lowering)) => lowering
            .emitted_steps()
            .first()
            .map(|emission| emission.continuation_contract().out_step_schema())
            .unwrap_or_else(|| {
                callable
                    .body_step_schema()
                    .unwrap_or_else(|| callable.step_schema())
            }),
        Some(LateLoweredBoundaryLowering::RuntimeError(lowering)) => lowering
            .emitted_step()
            .continuation_contract()
            .out_step_schema(),
        Some(LateLoweredBoundaryLowering::Handle(_)) | None => callable
            .body_step_schema()
            .unwrap_or_else(|| callable.step_schema()),
    };
    let label = case_labels
        .get(&(step_schema, case_tag))
        .cloned()
        .unwrap_or_else(|| "case_missing".to_string());
    format!("{}={}", label, case_op_text(program, step_schema, case_tag))
}

fn state_terminator_kind_name(terminator: &LateLoweredStateTerminator) -> &'static str {
    match terminator {
        LateLoweredStateTerminator::Suspend { .. } => "Suspend",
        LateLoweredStateTerminator::Goto { .. } => "Goto",
        LateLoweredStateTerminator::Branch { .. } => "Branch",
        LateLoweredStateTerminator::Return { .. } => "Return",
        LateLoweredStateTerminator::HandleDispatch { .. } => "HandleDispatch",
        LateLoweredStateTerminator::LocalRuntimeError { .. } => "LocalRuntimeError",
        LateLoweredStateTerminator::ResumeUnwind => "ResumeUnwind",
        LateLoweredStateTerminator::Unreachable => "Unreachable",
        LateLoweredStateTerminator::Abandon => "Abandon",
    }
}

fn boundary_lowering_kind_name(lowering: &LateLoweredBoundaryLowering) -> &'static str {
    match lowering {
        LateLoweredBoundaryLowering::Call(_) => "Call",
        LateLoweredBoundaryLowering::ClassCtor(_) => "ClassCtor",
        LateLoweredBoundaryLowering::Perform(_) => "Perform",
        LateLoweredBoundaryLowering::Resume(_) => "Resume",
        LateLoweredBoundaryLowering::RuntimeError(_) => "RuntimeError",
        LateLoweredBoundaryLowering::Handle(_) => "Handle",
    }
}

fn next_ordinal(map: &mut HashMap<String, usize>, signature: &str) -> usize {
    let entry = map.entry(signature.to_string()).or_insert(0);
    let ordinal = *entry;
    *entry += 1;
    ordinal
}

/// 渲染 late-lowered program 的稳定文本格式。
pub fn render_late_lowered_program(program: &LateLoweredProgram) -> String {
    let ctx = DumpCtx::new(program);
    let mut rendered = String::new();
    writeln!(&mut rendered, "LateLoweredProgram").unwrap();
    writeln!(
        &mut rendered,
        "  step_type_count: {}",
        program.step_types().len()
    )
    .unwrap();
    writeln!(
        &mut rendered,
        "  resume_packing_interface_count: {}",
        program.resume_packings().len()
    )
    .unwrap();
    writeln!(
        &mut rendered,
        "  continuation_object_count: {}",
        program.continuation_objects().len()
    )
    .unwrap();
    writeln!(
        &mut rendered,
        "  surface_resume_dispatch_count: {}",
        program.surface_resume_dispatch_inventory().len()
    )
    .unwrap();
    writeln!(&mut rendered, "  callable_count: {}", program.len()).unwrap();

    writeln!(&mut rendered, "  step_types:").unwrap();
    if program.step_types().is_empty() {
        writeln!(&mut rendered, "    <none>").unwrap();
    } else {
        for step_type in program.step_types() {
            render_step_type(&ctx, &mut rendered, step_type);
        }
    }

    writeln!(&mut rendered, "  continuation_objects:").unwrap();
    if program.continuation_objects().is_empty() {
        writeln!(&mut rendered, "    <none>").unwrap();
    } else {
        for object in program.continuation_objects() {
            render_continuation_object(&ctx, &mut rendered, object);
        }
    }

    writeln!(
        &mut rendered,
        "  authoritative_surface_resume_dispatch_inventory:"
    )
    .unwrap();
    if program.surface_resume_dispatch_inventory().is_empty() {
        writeln!(&mut rendered, "    <none>").unwrap();
    } else {
        for entry in program.surface_resume_dispatch_inventory() {
            render_surface_resume_dispatch_inventory_entry(&ctx, &mut rendered, entry);
        }
    }

    writeln!(&mut rendered, "  resume_packing_interfaces:").unwrap();
    if program.resume_packings().is_empty() {
        writeln!(&mut rendered, "    <none>").unwrap();
    } else {
        for interface in program.resume_packings() {
            render_resume_interface(&ctx, &mut rendered, interface);
        }
    }

    writeln!(&mut rendered, "  callables:").unwrap();
    if program.is_empty() {
        writeln!(&mut rendered, "    <none>").unwrap();
    } else {
        for callable in program.callables() {
            render_callable(&ctx, &mut rendered, callable);
        }
    }

    rendered
}

fn render_step_type(ctx: &DumpCtx<'_>, rendered: &mut String, step_type: &LateLoweredStepType) {
    writeln!(
        rendered,
        "    - step_schema: {}",
        ctx.step_label(step_type.step_schema())
    )
    .unwrap();
    writeln!(
        rendered,
        "      invoke_args_tuple_ty: {}",
        ctx.type_text(step_type.invoke_args_tuple_ty())
    )
    .unwrap();
    writeln!(
        rendered,
        "      complete_variant: Complete({})",
        ctx.type_text(step_type.complete_ty())
    )
    .unwrap();
    writeln!(
        rendered,
        "      continuation_obj_ty: {}",
        ctx.type_text(step_type.continuation_obj_ty())
    )
    .unwrap();
    writeln!(rendered, "      case_variants:").unwrap();
    if step_type.cases().is_empty() {
        writeln!(rendered, "        <none>").unwrap();
        return;
    }
    for case in step_type.cases() {
        render_step_case(ctx, rendered, step_type.step_schema(), case);
    }
}

fn render_step_case(
    ctx: &DumpCtx<'_>,
    rendered: &mut String,
    step_schema: crate::effect_facts::StepSchemaId,
    case: &LateLoweredStepCase,
) {
    writeln!(
        rendered,
        "        - Case({}) payload_tuple_ty={} continuation_schema={} resume_tuple_ty={} answer_ty={} surface_ty={} out_step_schema={} concrete_op={}",
        ctx.case_ref(step_schema, case.case_tag()),
        ctx.type_text(case.payload_tuple_ty()),
        ctx.continuation_label(case.continuation_schema()),
        ctx.type_text(case.resume_tuple_ty()),
        ctx.type_text(case.answer_ty()),
        ctx.type_text(case.surface_ty()),
        ctx.step_label(case.out_step_schema()),
        concrete_op_text(ctx.program, case.concrete_op_key()),
    )
    .unwrap();
}

fn render_resume_interface(
    ctx: &DumpCtx<'_>,
    rendered: &mut String,
    interface: &LateLoweredResumeInterface,
) {
    writeln!(
        rendered,
        "    - resume_packing_interface: {}",
        ctx.resume_packing_label(interface.interface_id())
    )
    .unwrap();
    writeln!(
        rendered,
        "      packing_effect_family: {}",
        effect_family_text(ctx.program, interface.effect_family())
    )
    .unwrap();
    writeln!(
        rendered,
        "      authoritative_step_schema: {}",
        ctx.step_label(interface.return_step_schema())
    )
    .unwrap();
    writeln!(rendered, "      packed_methods:").unwrap();
    if interface.methods().is_empty() {
        writeln!(rendered, "        <none>").unwrap();
        return;
    }
    for method in interface.methods() {
        render_resume_method(ctx, rendered, interface.return_step_schema(), method);
    }
}

fn render_resume_method(
    ctx: &DumpCtx<'_>,
    rendered: &mut String,
    step_schema: crate::effect_facts::StepSchemaId,
    method: &LateLoweredResumeMethod,
) {
    writeln!(
        rendered,
        "        - case: {} resume_tuple_ty={} answer_ty={} surface_ty={} out_step_schema={} continuation_schema={} concrete_op={}",
        ctx.case_ref(step_schema, method.case_tag()),
        ctx.type_text(method.resume_tuple_ty()),
        ctx.type_text(method.answer_ty()),
        ctx.type_text(method.surface_ty()),
        ctx.step_label(method.out_step_schema()),
        ctx.continuation_label(method.continuation_schema()),
        concrete_op_text(ctx.program, method.concrete_op_key()),
    )
    .unwrap();
}

fn render_continuation_object(
    ctx: &DumpCtx<'_>,
    rendered: &mut String,
    object: &LateLoweredContinuationObject,
) {
    writeln!(
        rendered,
        "    - continuation_object: {}",
        ctx.continuation_object_label(object.object_id())
    )
    .unwrap();
    writeln!(
        rendered,
        "      owner_version: {}",
        render_body_version_key(ctx, object.owner_version_key())
    )
    .unwrap();
    writeln!(
        rendered,
        "      continuation_obj_ty: {}",
        ctx.type_text(object.continuation_obj_ty())
    )
    .unwrap();
    writeln!(
        rendered,
        "      implemented_packings: {}",
        render_resume_interface_ids(ctx, object.implemented_packings())
    )
    .unwrap();
    writeln!(rendered, "      captures:").unwrap();
    if object.captures().is_empty() {
        writeln!(rendered, "        <none>").unwrap();
    } else {
        for capture in object.captures() {
            writeln!(
                rendered,
                "        - {}",
                render_capture(ctx, object.owner_version_key(), *capture)
            )
            .unwrap();
        }
    }
    writeln!(rendered, "      authoritative_surface_resumes:").unwrap();
    if object.surface_resumes().is_empty() {
        writeln!(rendered, "        <none>").unwrap();
    } else {
        for surface_resume in object.surface_resumes() {
            render_surface_resume(ctx, rendered, surface_resume);
        }
    }
    writeln!(rendered, "      authoritative_internal_methods:").unwrap();
    if object.methods().is_empty() {
        writeln!(rendered, "        <none>").unwrap();
        return;
    }
    for method in object.methods() {
        render_continuation_method(ctx, rendered, method);
    }
}

fn render_continuation_method(
    ctx: &DumpCtx<'_>,
    rendered: &mut String,
    method: &LateLoweredContinuationMethod,
) {
    writeln!(
        rendered,
        "        - case={} packed_by={} resume_tuple_ty={} answer_ty={} surface_ty={} out_step_schema={} continuation_schema={} concrete_op={} => {}",
        ctx.case_ref(method.out_step_schema(), method.case_tag()),
        ctx.resume_packing_label(method.packing_interface_id()),
        ctx.type_text(method.resume_tuple_ty()),
        ctx.type_text(method.answer_ty()),
        ctx.type_text(method.surface_ty()),
        ctx.step_label(method.out_step_schema()),
        ctx.continuation_label(method.continuation_schema()),
        concrete_op_text(ctx.program, method.concrete_op_key()),
        render_continuation_resume_body(method.body()),
    )
    .unwrap();
}

fn render_surface_resume(
    ctx: &DumpCtx<'_>,
    rendered: &mut String,
    surface_resume: &LateLoweredContinuationSurfaceResume,
) {
    writeln!(
        rendered,
        "        - case={} resume_tuple_ty={} answer_ty={} surface_ty={} out_step_schema={} continuation_schema={} concrete_op={} => {}",
        ctx.case_ref(surface_resume.out_step_schema(), surface_resume.case_tag()),
        ctx.type_text(surface_resume.resume_tuple_ty()),
        ctx.type_text(surface_resume.answer_ty()),
        ctx.type_text(surface_resume.surface_ty()),
        ctx.step_label(surface_resume.out_step_schema()),
        ctx.continuation_label(surface_resume.continuation_schema()),
        concrete_op_text(ctx.program, surface_resume.concrete_op_key()),
        render_continuation_resume_body(surface_resume.body()),
    )
    .unwrap();
}

fn render_surface_resume_dispatch_inventory_entry(
    ctx: &DumpCtx<'_>,
    rendered: &mut String,
    entry: &LateLoweredSurfaceResumeDispatchInventoryEntry,
) {
    let contract = entry.contract();
    writeln!(
        rendered,
        "    - continuation_schema: {} source={} resume_tuple_ty={} answer_ty={} out_step_schema={}",
        ctx.continuation_label(entry.continuation_schema()),
        render_surface_resume_dispatch_source_kind(entry.source_kind()),
        ctx.type_text(contract.resume_tuple_ty()),
        ctx.type_text(contract.answer_ty()),
        ctx.step_label(contract.out_step_schema()),
    )
    .unwrap();
    writeln!(rendered, "      publications:").unwrap();
    if entry.publications().is_empty() {
        writeln!(rendered, "        <none>").unwrap();
        return;
    }
    for publication in entry.publications() {
        writeln!(
            rendered,
            "        - {}",
            render_surface_resume_dispatch_publication(ctx, publication)
        )
        .unwrap();
    }
    for projection in entry.wrapper_projections() {
        render_surface_resume_wrapper_projection(ctx, rendered, projection);
    }
}

fn render_surface_resume_dispatch_source_kind(
    kind: LateLoweredSurfaceResumeDispatchSourceKind,
) -> &'static str {
    match kind {
        LateLoweredSurfaceResumeDispatchSourceKind::ContinuationObjectMethod => {
            "ContinuationObjectMethod"
        }
        LateLoweredSurfaceResumeDispatchSourceKind::ResumeBoundaryOnly => "ResumeBoundaryOnly",
        LateLoweredSurfaceResumeDispatchSourceKind::HandleContinuationBinderOnly => {
            "HandleContinuationBinderOnly"
        }
        LateLoweredSurfaceResumeDispatchSourceKind::OwnerTrampolineMixed => "OwnerTrampolineMixed",
        LateLoweredSurfaceResumeDispatchSourceKind::Unreachable => "Unreachable",
    }
}

fn render_surface_resume_dispatch_publication(
    ctx: &DumpCtx<'_>,
    publication: &LateLoweredSurfaceResumeDispatchPublication,
) -> String {
    match publication {
        LateLoweredSurfaceResumeDispatchPublication::SurfaceCase {
            object_id,
            case_tag,
            reachability,
        } => format!(
            "surface_case {} case={} reachability={reachability:?}",
            ctx.continuation_object_label(*object_id),
            ctx.program
                .continuation_object(*object_id)
                .and_then(|object| {
                    object
                        .surface_resumes()
                        .iter()
                        .find(|resume| resume.case_tag() == *case_tag)
                        .map(|resume| ctx.case_ref(resume.out_step_schema(), *case_tag))
                })
                .unwrap_or_else(|| "case_missing".to_string()),
        ),
        LateLoweredSurfaceResumeDispatchPublication::InternalMethod {
            object_id,
            packing_interface_id,
            case_tag,
            reachability,
        } => format!(
            "internal_method {} case={} packed_by={} reachability={reachability:?}",
            ctx.continuation_object_label(*object_id),
            ctx.program
                .continuation_object(*object_id)
                .and_then(|object| {
                    object
                        .methods()
                        .iter()
                        .find(|method| {
                            method.case_tag() == *case_tag
                                && method.packing_interface_id() == *packing_interface_id
                        })
                        .map(|method| ctx.case_ref(method.out_step_schema(), *case_tag))
                })
                .unwrap_or_else(|| "case_missing".to_string()),
            ctx.resume_packing_label(*packing_interface_id),
        ),
        LateLoweredSurfaceResumeDispatchPublication::ResumeBoundary {
            owner_version_key,
            owner_continuation_object,
            site_id,
        } => format!(
            "resume_boundary {} {} {}",
            render_body_version_key(ctx, owner_version_key),
            ctx.continuation_object_label(*owner_continuation_object),
            ctx.site_label(owner_version_key, *site_id),
        ),
        LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
            owner_version_key,
            owner_continuation_object,
            site_id,
            arm_ordinal,
            handled_case,
        } => format!(
            "handle_continuation_binder {} {} {} arm#{} handled_case={}",
            render_body_version_key(ctx, owner_version_key),
            ctx.continuation_object_label(*owner_continuation_object),
            ctx.site_label(owner_version_key, *site_id),
            arm_ordinal,
            ctx.program
                .callable_by_version_key(owner_version_key)
                .and_then(|callable| callable
                    .body_step_schema()
                    .map(|step| ctx.case_ref(step, *handled_case)))
                .unwrap_or_else(|| "case_missing".to_string()),
        ),
    }
}

fn render_surface_resume_wrapper_projection(
    ctx: &DumpCtx<'_>,
    rendered: &mut String,
    projection: &LateLoweredSurfaceResumeWrapperProjection,
) {
    let projection_owner = projection_owner_callable(ctx, projection);
    writeln!(rendered, "      wrapper_projection:").unwrap();
    writeln!(
        rendered,
        "        underlying_route: continuation_schema={} via {}",
        ctx.continuation_label(projection.underlying_route().continuation_schema()),
        render_surface_resume_dispatch_publication(
            ctx,
            projection.underlying_route().publication()
        ),
    )
    .unwrap();
    writeln!(
        rendered,
        "        owner_step_schema: {}",
        ctx.step_label(projection.owner_step_schema()),
    )
    .unwrap();
    writeln!(
        rendered,
        "        wrapper_step_schema: {}",
        ctx.step_label(projection.wrapper_step_schema()),
    )
    .unwrap();
    render_surface_resume_wrapper_complete_projection(
        ctx,
        rendered,
        projection_owner,
        projection.complete(),
    );
    writeln!(rendered, "        outward_cases:").unwrap();
    if projection.outward_cases().is_empty() {
        writeln!(rendered, "          <none>").unwrap();
    } else {
        for case in projection.outward_cases() {
            render_surface_resume_wrapper_case_projection(
                ctx,
                rendered,
                projection.owner_step_schema(),
                projection.wrapper_step_schema(),
                case,
            );
        }
    }
}

fn render_surface_resume_wrapper_complete_projection(
    ctx: &DumpCtx<'_>,
    rendered: &mut String,
    owner_callable: Option<&LateLoweredCallable>,
    complete: &LateLoweredSurfaceResumeWrapperCompleteProjection,
) {
    writeln!(
        rendered,
        "        complete: owner_answer_ty={} -> wrapper_answer_ty={} payload={}",
        ctx.type_text(complete.owner_answer_ty()),
        ctx.type_text(complete.wrapper_answer_ty()),
        render_surface_resume_wrapper_complete_payload_source(
            ctx,
            owner_callable,
            complete.payload_source(),
        ),
    )
    .unwrap();
}

fn render_surface_resume_wrapper_complete_payload_source(
    ctx: &DumpCtx<'_>,
    owner_callable: Option<&LateLoweredCallable>,
    source: &LateLoweredSurfaceResumeWrapperCompletePayloadSource,
) -> String {
    match source {
        LateLoweredSurfaceResumeWrapperCompletePayloadSource::OwnerComplete { answer_ty } => {
            format!("owner_complete:{}", ctx.type_text(*answer_ty))
        }
        LateLoweredSurfaceResumeWrapperCompletePayloadSource::WrapperPayload(source) => {
            render_completion_payload_source(ctx, owner_callable, source)
        }
    }
}

fn render_surface_resume_wrapper_case_projection(
    ctx: &DumpCtx<'_>,
    rendered: &mut String,
    owner_step_schema: crate::effect_facts::StepSchemaId,
    wrapper_step_schema: crate::effect_facts::StepSchemaId,
    projection: &LateLoweredSurfaceResumeWrapperCaseProjection,
) {
    writeln!(
        rendered,
        "          - owner {} op={} payload_tuple_ty={} -> wrapper {} op={} payload_tuple_ty={} cont_schema={} out_step_schema={}",
        ctx.case_ref(owner_step_schema, projection.owner_case_tag()),
        concrete_op_text(ctx.program, projection.owner_concrete_op_key()),
        ctx.type_text(projection.owner_payload_tuple_ty()),
        ctx.case_ref(wrapper_step_schema, projection.wrapper_case_tag()),
        concrete_op_text(ctx.program, projection.wrapper_concrete_op_key()),
        ctx.type_text(projection.wrapper_payload_tuple_ty()),
        ctx.continuation_label(
            projection
                .wrapper_continuation_contract()
                .continuation_schema(),
        ),
        ctx.step_label(projection.wrapper_continuation_contract().out_step_schema()),
    )
    .unwrap();
}

fn render_callable(ctx: &DumpCtx<'_>, rendered: &mut String, callable: &LateLoweredCallable) {
    writeln!(rendered, "    - root: {}", callable.root_fqn()).unwrap();
    writeln!(
        rendered,
        "      body_version_key: {}",
        render_body_version_key(ctx, callable.body_version_key())
    )
    .unwrap();
    writeln!(
        rendered,
        "      resolved_outward_cases: {}",
        render_cases(
            ctx,
            callable.body_step_schema(),
            callable.resolved_outward_cases()
        )
    )
    .unwrap();
    match callable.abi() {
        LateLoweredCallableAbi::Plain(plain) => {
            render_plain_callable(ctx, rendered, callable, plain)
        }
        LateLoweredCallableAbi::EffectStep(_) => {
            render_effect_step_callable(ctx, rendered, callable)
        }
    }
}

fn render_plain_callable(
    ctx: &DumpCtx<'_>,
    rendered: &mut String,
    callable: &LateLoweredCallable,
    plain: &LateLoweredPlainCallable,
) {
    writeln!(rendered, "      abi: Plain").unwrap();
    writeln!(
        rendered,
        "      ordinary_signature: fn_ty={} params=[{}] return={}",
        ctx.type_text(plain.function_ty()),
        plain
            .param_tys()
            .iter()
            .copied()
            .map(|ty| ctx.type_text(ty))
            .collect::<Vec<_>>()
            .join(", "),
        ctx.type_text(plain.return_ty()),
    )
    .unwrap();
    writeln!(rendered, "      plain_source_slices:").unwrap();
    if plain.body_slices().is_empty() {
        writeln!(rendered, "        <none>").unwrap();
    } else {
        for slice in plain.body_slices() {
            writeln!(
                rendered,
                "        - {}",
                render_plain_body_slice(ctx, callable, *slice)
            )
            .unwrap();
        }
    }
    writeln!(rendered, "      plain_call_sites:").unwrap();
    if plain.call_sites().is_empty() {
        writeln!(rendered, "        <none>").unwrap();
    } else {
        for call_site in plain.call_sites() {
            render_plain_call_site(ctx, rendered, callable, call_site);
        }
    }
    if let Some(local) = plain.local_effect_control() {
        writeln!(
            rendered,
            "      plain_local_effect_control: {} {}",
            ctx.step_label(local.step_schema()),
            ctx.continuation_object_label(local.continuation_object())
        )
        .unwrap();
        render_state_graph(ctx, rendered, callable);
        render_frame_schema(ctx, rendered, callable, callable.frame_schema());
        render_boundary_map(ctx, rendered, callable, callable.boundary_map().entries());
        render_resume_state_map(ctx, rendered, callable, callable.resume_state_map());
        writeln!(
            rendered,
            "      resume_packing_interfaces: {}",
            render_resume_interface_ids(ctx, local.resume_packings())
        )
        .unwrap();
    } else {
        writeln!(rendered, "      plain_local_effect_control: <none>").unwrap();
    }
    writeln!(rendered, "      effect_step_handoff: <none>").unwrap();
}

fn render_plain_call_site(
    ctx: &DumpCtx<'_>,
    rendered: &mut String,
    callable: &LateLoweredCallable,
    call_site: &LateLoweredPlainCallSite,
) {
    let facts = call_site.facts();
    writeln!(
        rendered,
        "        - {} {:?} target_mode={:?} target={} callee_abi={} invoke_args_tuple_ty={} callee_step_schema={} resolved_cases={} anchor={} stmt{} dispatch={}",
        ctx.site_label(callable.body_version_key(), call_site.site_id()),
        facts.kind(),
        facts.target_mode(),
        render_call_target(ctx, facts.target()),
        render_callable_abi_kind(facts.callee_abi_kind()),
        ctx.type_text(facts.invoke_args_tuple_ty()),
        facts
            .callee_step_schema()
            .map(|schema| ctx.step_label(schema))
            .unwrap_or_else(|| "<none>".to_string()),
        render_cases(ctx, Some(facts.resolved_cases().schema()), facts.resolved_cases().tags()),
        ctx.block_label(callable.body_version_key(), call_site.source_slice().block_id()),
        call_site.statement_index(),
        match facts.callee_abi_kind() {
            crate::effect_facts::CallableAbiKind::Plain => "PlainCall",
            crate::effect_facts::CallableAbiKind::EffectStep => "EffectStepDispatch",
        },
    )
    .unwrap();
}

fn render_effect_step_callable(
    ctx: &DumpCtx<'_>,
    rendered: &mut String,
    callable: &LateLoweredCallable,
) {
    writeln!(rendered, "      abi: EffectStep").unwrap();
    writeln!(
        rendered,
        "      authoritative_step_schema: {}",
        ctx.step_label(callable.step_schema())
    )
    .unwrap();
    writeln!(
        rendered,
        "      dynamic_invoke_entry: invoke({}) -> {} entry={} complete={}",
        ctx.type_text(callable.dynamic_invoke_entry().invoke_args_tuple_ty()),
        ctx.step_label(callable.dynamic_invoke_entry().step_schema()),
        ctx.state_label(
            callable.body_version_key(),
            callable.dynamic_invoke_entry().entry_state(),
        ),
        ctx.state_label(
            callable.body_version_key(),
            callable.dynamic_invoke_entry().complete_state(),
        ),
    )
    .unwrap();
    render_state_graph(ctx, rendered, callable);
    render_frame_schema(ctx, rendered, callable, callable.frame_schema());
    render_boundary_map(ctx, rendered, callable, callable.boundary_map().entries());
    render_resume_state_map(ctx, rendered, callable, callable.resume_state_map());
    writeln!(
        rendered,
        "      resume_packing_interfaces: {}",
        render_resume_interface_ids(ctx, callable.resume_packings())
    )
    .unwrap();
    writeln!(
        rendered,
        "      continuation_object: {}",
        ctx.continuation_object_label(callable.continuation_object())
    )
    .unwrap();
}

fn render_plain_body_slice(
    ctx: &DumpCtx<'_>,
    callable: &LateLoweredCallable,
    slice: LateLoweredPlainBodySlice,
) -> String {
    let terminator = if slice.includes_terminator() {
        " + term"
    } else {
        ""
    };
    format!(
        "{} stmts[{}..{}]{terminator}",
        ctx.block_label(callable.body_version_key(), slice.block_id()),
        slice.start_statement_index(),
        slice.end_statement_index(),
    )
}

fn render_state_graph(ctx: &DumpCtx<'_>, rendered: &mut String, callable: &LateLoweredCallable) {
    let state_graph = callable.state_graph();
    writeln!(rendered, "      state_graph:").unwrap();
    writeln!(
        rendered,
        "        entry_state: {}",
        ctx.state_label(callable.body_version_key(), state_graph.entry_state())
    )
    .unwrap();
    writeln!(
        rendered,
        "        complete_state: {}",
        ctx.state_label(callable.body_version_key(), state_graph.complete_state())
    )
    .unwrap();
    writeln!(
        rendered,
        "        cleanup_state: {}",
        render_optional_state(
            ctx,
            callable.body_version_key(),
            state_graph.cleanup_state()
        )
    )
    .unwrap();
    writeln!(
        rendered,
        "        drop_state: {}",
        render_optional_state(ctx, callable.body_version_key(), state_graph.drop_state())
    )
    .unwrap();
    writeln!(rendered, "        states:").unwrap();
    if state_graph.states().is_empty() {
        writeln!(rendered, "          <none>").unwrap();
        return;
    }
    for state in state_graph.states() {
        render_state(
            ctx,
            rendered,
            callable,
            state,
            callable.source_statement_classifications(),
        );
    }
}

fn render_state(
    ctx: &DumpCtx<'_>,
    rendered: &mut String,
    callable: &LateLoweredCallable,
    state: &LateLoweredState,
    classifications: &[LateLoweredSourceStatementClassification],
) {
    writeln!(
        rendered,
        "          - {} {} term={} successors={}",
        ctx.state_label(callable.body_version_key(), state.state_id()),
        render_state_role(state.role()),
        render_state_terminator(ctx, callable, state.terminator()),
        render_state_successors(ctx, callable.body_version_key(), state.successors())
    )
    .unwrap();
    if let LateLoweredStateTerminator::HandleDispatch { contract, .. } = state.terminator() {
        render_handle_dispatch_contract(ctx, rendered, callable, contract);
    }
    writeln!(rendered, "            source_slices:").unwrap();
    if state.source_slices().is_empty() {
        writeln!(rendered, "              <synthetic>").unwrap();
        return;
    }
    for slice in state.source_slices() {
        render_state_slice(ctx, rendered, callable, *slice);
        render_state_slice_classifications(ctx, rendered, callable, *slice, classifications);
    }
}

fn render_state_slice_classifications(
    ctx: &DumpCtx<'_>,
    rendered: &mut String,
    callable: &LateLoweredCallable,
    slice: LateLoweredStateSlice,
    classifications: &[LateLoweredSourceStatementClassification],
) {
    writeln!(rendered, "                statement_classification:").unwrap();
    let mut rendered_any = false;
    for classification in classifications.iter().filter(|classification| {
        classification.source_slice() == slice
            && classification.statement_index() >= slice.start_statement_index()
            && classification.statement_index() < slice.end_statement_index()
    }) {
        rendered_any = true;
        writeln!(
            rendered,
            "                  - stmt{}: {}",
            classification.statement_index(),
            render_source_statement_classification_kind(ctx, callable, classification.kind()),
        )
        .unwrap();
    }
    if !rendered_any {
        let marker = if slice.start_statement_index() == slice.end_statement_index() {
            "<none>"
        } else {
            "<unclassified>"
        };
        writeln!(rendered, "                  {marker}").unwrap();
    }
}

fn render_source_statement_classification_kind(
    ctx: &DumpCtx<'_>,
    callable: &LateLoweredCallable,
    kind: LateLoweredSourceStatementClassificationKind,
) -> String {
    match kind {
        LateLoweredSourceStatementClassificationKind::EffectNeutralValue => {
            "effect-neutral-value".to_string()
        }
        LateLoweredSourceStatementClassificationKind::BoundaryConsumedAnchor { boundary_id } => {
            format!(
                "boundary-consumed-anchor {}",
                ctx.boundary_label(callable.body_version_key(), boundary_id)
            )
        }
        LateLoweredSourceStatementClassificationKind::ResumePayloadInjection {
            boundary_id,
            resume_state,
            consumer_local,
        } => format!(
            "resume-payload-injection {} resume={} {}",
            ctx.boundary_label(callable.body_version_key(), boundary_id),
            ctx.state_label(callable.body_version_key(), resume_state),
            ctx.local_label(callable.body_version_key(), consumer_local),
        ),
        LateLoweredSourceStatementClassificationKind::BoundaryResultInjection {
            boundary_id,
            resume_state,
            result_local,
        } => format!(
            "boundary-result-injection {} resume={} {}",
            ctx.boundary_label(callable.body_version_key(), boundary_id),
            ctx.state_label(callable.body_version_key(), resume_state),
            ctx.local_label(callable.body_version_key(), result_local),
        ),
        LateLoweredSourceStatementClassificationKind::CompletionPayloadInjection {
            return_state,
            complete_state,
        } => format!(
            "completion-payload-injection return={} complete={}",
            ctx.state_label(callable.body_version_key(), return_state),
            ctx.state_label(callable.body_version_key(), complete_state),
        ),
        LateLoweredSourceStatementClassificationKind::HandleSyntheticCarrierBinder {
            site_id,
            state_id,
        } => format!(
            "handle-synthetic-carrier-binder {} state={}",
            ctx.site_label(callable.body_version_key(), site_id),
            ctx.state_label(callable.body_version_key(), state_id),
        ),
        LateLoweredSourceStatementClassificationKind::DynamicInvokeCall { site_id, metadata } => {
            format!(
                "dynamic-invoke-call {} kind={:?} args={}",
                ctx.site_label(callable.body_version_key(), site_id),
                metadata.kind(),
                metadata.arg_count(),
            )
        }
        LateLoweredSourceStatementClassificationKind::ElidedUnreachable => {
            "elided-unreachable".to_string()
        }
        LateLoweredSourceStatementClassificationKind::Unsupported { reason } => {
            format!("unsupported reason={reason}")
        }
    }
}

fn render_handle_dispatch_contract(
    ctx: &DumpCtx<'_>,
    rendered: &mut String,
    callable: &LateLoweredCallable,
    contract: &LateLoweredHandleDispatchContract,
) {
    writeln!(rendered, "            handle_contract:").unwrap();
    writeln!(
        rendered,
        "              carriers: state={} completion={} payload={}",
        render_system_slot_kind(contract.carrier().state_tag_slot()),
        render_system_slot_kind(contract.carrier().completion_tag_slot()),
        render_system_slot_kind(contract.carrier().payload_carrier_slot()),
    )
    .unwrap();
    writeln!(
        rendered,
        "              body_complete_target: {}",
        ctx.state_label(callable.body_version_key(), contract.body_complete_target()),
    )
    .unwrap();
    writeln!(
        rendered,
        "              arm_complete_target: {}",
        ctx.state_label(callable.body_version_key(), contract.arm_complete_target()),
    )
    .unwrap();
    writeln!(
        rendered,
        "              finally_complete_target: {}",
        render_optional_state(
            ctx,
            callable.body_version_key(),
            contract.finally_complete_target()
        ),
    )
    .unwrap();
    let body_completion_payload = contract
        .body_completion_payload_source()
        .map(|source| render_completion_payload_source(ctx, Some(callable), source))
        .unwrap_or_else(|| "<unpublished>".to_string());
    writeln!(
        rendered,
        "              body_completion_payload: {body_completion_payload}",
    )
    .unwrap();
    writeln!(
        rendered,
        "              abandon_target: {}",
        render_optional_state(ctx, callable.body_version_key(), contract.abandon_target()),
    )
    .unwrap();
    writeln!(
        rendered,
        "              body_outward_cases: {}",
        render_cases(
            ctx,
            callable.body_step_schema(),
            contract.body_outward_cases()
        ),
    )
    .unwrap();
    writeln!(
        rendered,
        "              finally_outward_cases: {}",
        render_cases(
            ctx,
            callable.body_step_schema(),
            contract.finally_outward_cases()
        ),
    )
    .unwrap();
    writeln!(rendered, "              handled_arms:").unwrap();
    if contract.handled_arms().is_empty() {
        writeln!(rendered, "                <none>").unwrap();
    } else {
        for arm in contract.handled_arms() {
            writeln!(
                rendered,
                "                - handled={} ordinal={} -> {} payload_tuple_ty={} outward={}",
                callable
                    .body_step_schema()
                    .map(|step| ctx.case_ref(step, arm.handled_case()))
                    .unwrap_or_else(|| "case_missing".to_string()),
                arm.arm_ordinal(),
                ctx.state_label(callable.body_version_key(), arm.arm_state()),
                ctx.type_text(arm.payload_tuple_ty()),
                render_cases(ctx, callable.body_step_schema(), arm.arm_outward_cases()),
            )
            .unwrap();
            writeln!(rendered, "                  payload_binders:").unwrap();
            if arm.payload_binders().is_empty() {
                writeln!(rendered, "                    <none>").unwrap();
            } else {
                for binder in arm.payload_binders() {
                    writeln!(
                        rendered,
                        "                    - #{} {} slot={}",
                        binder.ordinal(),
                        ctx.local_label(callable.body_version_key(), binder.local()),
                        render_optional_frame_slot(
                            ctx,
                            callable.body_version_key(),
                            binder.frame_slot()
                        ),
                    )
                    .unwrap();
                }
            }
            let continuation_binder = arm.continuation_binder().map_or_else(
                || "<none>".to_string(),
                |binder| {
                    format!(
                        "{} slot={} continuation_schema={} continuation_object={}",
                        ctx.local_label(callable.body_version_key(), binder.local()),
                        render_optional_frame_slot(
                            ctx,
                            callable.body_version_key(),
                            binder.frame_slot()
                        ),
                        ctx.continuation_label(binder.continuation_schema()),
                        ctx.continuation_object_label(binder.continuation_object()),
                    )
                },
            );
            writeln!(
                rendered,
                "                  continuation_binder: {continuation_binder}",
            )
            .unwrap();
            writeln!(
                rendered,
                "                  completion_payload: {}",
                render_completion_payload_source(
                    ctx,
                    Some(callable),
                    arm.completion_payload_source()
                ),
            )
            .unwrap();
        }
    }
    writeln!(rendered, "              pending_completions:").unwrap();
    if contract.pending_completions().is_empty() {
        writeln!(rendered, "                <none>").unwrap();
    } else {
        for pending in contract.pending_completions() {
            writeln!(
                rendered,
                "                - {}",
                render_handle_pending_completion(ctx, callable, *pending),
            )
            .unwrap();
        }
    }
    writeln!(rendered, "              pending_completion_origins:").unwrap();
    if contract.pending_completion_origins().is_empty() {
        writeln!(rendered, "                <none>").unwrap();
    } else {
        for origin in contract.pending_completion_origins() {
            writeln!(
                rendered,
                "                - {} via {} owner={} resume={}",
                render_handle_pending_completion(ctx, callable, origin.completion()),
                ctx.boundary_label(callable.body_version_key(), origin.boundary_id()),
                ctx.state_label(callable.body_version_key(), origin.owner_state()),
                ctx.state_label(callable.body_version_key(), origin.resume_state()),
            )
            .unwrap();
        }
    }
    writeln!(rendered, "              pending_payload_transports:").unwrap();
    if contract.pending_payload_transports().is_empty() {
        writeln!(rendered, "                <none>").unwrap();
    } else {
        for transport in contract.pending_payload_transports() {
            writeln!(
                rendered,
                "                - {} payload_tuple_ty={} frame_slot={}",
                render_handle_pending_completion(ctx, callable, transport.completion()),
                ctx.type_text(transport.payload_tuple_ty()),
                ctx.slot_label(callable.body_version_key(), transport.frame_slot()),
            )
            .unwrap();
        }
    }
    writeln!(rendered, "              state_regions:").unwrap();
    if contract.state_regions().is_empty() {
        writeln!(rendered, "                <none>").unwrap();
    } else {
        for entry in contract.state_regions() {
            writeln!(
                rendered,
                "                - {} => {}",
                ctx.state_label(callable.body_version_key(), entry.state_id()),
                render_handle_state_region(ctx, callable, entry.region()),
            )
            .unwrap();
        }
    }
    writeln!(rendered, "              boundary_routings:").unwrap();
    if contract.boundary_routings().is_empty() {
        writeln!(rendered, "                <none>").unwrap();
    } else {
        for routing in contract.boundary_routings() {
            writeln!(
                rendered,
                "                - {} owner={} region={} resume={}",
                ctx.boundary_label(callable.body_version_key(), routing.boundary_id()),
                ctx.state_label(callable.body_version_key(), routing.owner_state()),
                render_handle_state_region(ctx, callable, routing.owner_region()),
                ctx.state_label(callable.body_version_key(), routing.resume_state()),
            )
            .unwrap();
            writeln!(rendered, "                  case_routings:").unwrap();
            if routing.case_routings().is_empty() {
                writeln!(rendered, "                    <none>").unwrap();
            } else {
                for route in routing.case_routings() {
                    writeln!(
                        rendered,
                        "                    - {} => {}",
                        callable
                            .body_step_schema()
                            .map(|step| ctx.case_ref(step, route.case_tag()))
                            .unwrap_or_else(|| "case_missing".to_string()),
                        render_handle_boundary_case_routing_action(ctx, callable, route.action()),
                    )
                    .unwrap();
                }
            }
        }
    }
    writeln!(rendered, "              outward_emissions:").unwrap();
    if contract.outward_emissions().is_empty() {
        writeln!(rendered, "                <none>").unwrap();
    } else {
        for emission in contract.outward_emissions() {
            writeln!(
                rendered,
                "                - {} op={} payload_tuple_ty={} {} cont_schema={} out_step_schema={}",
                ctx.case_ref(
                    emission.continuation_contract().out_step_schema(),
                    emission.case_tag(),
                ),
                concrete_op_text(ctx.program, emission.concrete_op_key()),
                ctx.type_text(emission.payload_tuple_ty()),
                ctx.continuation_object_label(emission.continuation_object()),
                ctx.continuation_label(
                    emission.continuation_contract().continuation_schema(),
                ),
                ctx.step_label(emission.continuation_contract().out_step_schema()),
            )
            .unwrap();
        }
    }
}

fn render_state_slice(
    ctx: &DumpCtx<'_>,
    rendered: &mut String,
    callable: &LateLoweredCallable,
    slice: LateLoweredStateSlice,
) {
    writeln!(
        rendered,
        "              - {}",
        render_state_slice_inline(ctx, callable, slice)
    )
    .unwrap();
}

fn render_state_slice_inline(
    ctx: &DumpCtx<'_>,
    callable: &LateLoweredCallable,
    slice: LateLoweredStateSlice,
) -> String {
    let terminator = if slice.includes_terminator() {
        " + term"
    } else {
        ""
    };
    format!(
        "{} stmts[{}..{}]{terminator}",
        ctx.block_label(callable.body_version_key(), slice.block_id()),
        slice.start_statement_index(),
        slice.end_statement_index(),
    )
}

fn render_frame_schema(
    ctx: &DumpCtx<'_>,
    rendered: &mut String,
    callable: &LateLoweredCallable,
    frame_schema: &LateLoweredFrameSchema,
) {
    writeln!(rendered, "      frame_schema:").unwrap();
    writeln!(rendered, "        slots:").unwrap();
    if frame_schema.slots().is_empty() {
        writeln!(rendered, "          <none>").unwrap();
    } else {
        for slot in frame_schema.slots() {
            render_frame_slot(ctx, rendered, callable, slot);
        }
    }
    writeln!(rendered, "        resume_payload_bindings:").unwrap();
    if frame_schema.resume_payload_bindings().is_empty() {
        writeln!(rendered, "          <none>").unwrap();
    } else {
        for binding in frame_schema.resume_payload_bindings() {
            render_resume_payload_binding(ctx, rendered, callable, binding);
        }
    }
    writeln!(rendered, "        completion_payload_bindings:").unwrap();
    if frame_schema.completion_payload_bindings().is_empty() {
        writeln!(rendered, "          <none>").unwrap();
    } else {
        for binding in frame_schema.completion_payload_bindings() {
            render_completion_payload_binding(ctx, rendered, callable, binding);
        }
    }
}

fn render_frame_slot(
    ctx: &DumpCtx<'_>,
    rendered: &mut String,
    callable: &LateLoweredCallable,
    slot: &LateLoweredFrameSlot,
) {
    writeln!(
        rendered,
        "          - {} {} ty={} writes={} reads={}",
        ctx.slot_label(callable.body_version_key(), slot.slot_id()),
        frame_slot_kind_identity(
            ctx.program,
            callable,
            slot.kind(),
            &ctx.callable_labels
                .get(callable.body_version_key())
                .map(|labels| labels.states.clone())
                .unwrap_or_default(),
            &ctx.callable_labels
                .get(callable.body_version_key())
                .map(|labels| labels.boundaries.clone())
                .unwrap_or_default(),
            &ctx.step_labels,
            &ctx.case_labels,
        ),
        ctx.type_text(slot.ty()),
        render_state_successors(ctx, callable.body_version_key(), slot.write_points()),
        render_state_successors(ctx, callable.body_version_key(), slot.read_points()),
    )
    .unwrap();
}

fn render_resume_payload_binding(
    ctx: &DumpCtx<'_>,
    rendered: &mut String,
    callable: &LateLoweredCallable,
    binding: &LateLoweredResumePayloadBinding,
) {
    writeln!(
        rendered,
        "          - {} resume={} {} home={}",
        ctx.boundary_label(callable.body_version_key(), binding.boundary_id()),
        ctx.state_label(callable.body_version_key(), binding.resume_state()),
        ctx.local_label(callable.body_version_key(), binding.consumer_local()),
        binding
            .consumer_frame_slot()
            .map(|slot| ctx.slot_label(callable.body_version_key(), slot))
            .unwrap_or_else(|| "<none>".to_string()),
    )
    .unwrap();
}

fn render_completion_payload_binding(
    ctx: &DumpCtx<'_>,
    rendered: &mut String,
    callable: &LateLoweredCallable,
    binding: &LateLoweredCompletionPayloadBinding,
) {
    writeln!(
        rendered,
        "          - return={} complete={} payload={} home={}",
        ctx.state_label(callable.body_version_key(), binding.return_state()),
        ctx.state_label(callable.body_version_key(), binding.complete_state()),
        render_completion_payload_source(ctx, Some(callable), binding.payload_source()),
        render_optional_frame_slot(
            ctx,
            callable.body_version_key(),
            binding.payload_frame_slot()
        ),
    )
    .unwrap();
}

fn render_boundary_map(
    ctx: &DumpCtx<'_>,
    rendered: &mut String,
    callable: &LateLoweredCallable,
    boundaries: &[LateLoweredBoundary],
) {
    writeln!(rendered, "      boundary_map:").unwrap();
    if boundaries.is_empty() {
        writeln!(rendered, "        <none>").unwrap();
        return;
    }
    for boundary in boundaries {
        writeln!(
            rendered,
            "        - {} {} owner={} resume={}",
            ctx.boundary_label(callable.body_version_key(), boundary.boundary_id()),
            render_boundary_source(ctx, callable, boundary.source()),
            ctx.state_label(callable.body_version_key(), boundary.owner_state()),
            ctx.state_label(callable.body_version_key(), boundary.resume_state()),
        )
        .unwrap();
        if let Some(lowering) = boundary.lowering() {
            render_boundary_lowering(ctx, rendered, callable, lowering);
        }
    }
}

fn render_resume_state_map(
    ctx: &DumpCtx<'_>,
    rendered: &mut String,
    callable: &LateLoweredCallable,
    resume_state_map: &LateLoweredResumeStateMap,
) {
    writeln!(rendered, "      resume_state_map:").unwrap();
    if resume_state_map.entries().is_empty() {
        writeln!(rendered, "        <none>").unwrap();
        return;
    }
    for entry in resume_state_map.entries() {
        writeln!(
            rendered,
            "        - {} -> {}",
            ctx.boundary_label(callable.body_version_key(), entry.boundary_id()),
            ctx.state_label(callable.body_version_key(), entry.state_id()),
        )
        .unwrap();
    }
}

fn render_boundary_lowering(
    ctx: &DumpCtx<'_>,
    rendered: &mut String,
    callable: &LateLoweredCallable,
    lowering: &LateLoweredBoundaryLowering,
) {
    match lowering {
        LateLoweredBoundaryLowering::Call(lowering) => {
            writeln!(
                rendered,
                "          lowering: Call kind={:?} target_mode={:?} callee_step={} result={} target={}",
                lowering.facts().kind(),
                lowering.facts().target_mode(),
                ctx.step_label(lowering.facts().callee_schema()),
                ctx.local_label(callable.body_version_key(), lowering.result_local()),
                render_call_target(ctx, lowering.facts().target()),
            )
            .unwrap();
            render_call_operand_contract(ctx, rendered, callable, lowering.operand_contract());
            if let Some(consumed_runtime_error_case) = lowering.consumed_runtime_error_case() {
                render_consumed_runtime_error_case(
                    ctx,
                    rendered,
                    callable,
                    lowering.dispatch().input_step_schema(),
                    consumed_runtime_error_case,
                );
            }
            render_call_boundary_continuation_compositions(
                ctx,
                rendered,
                callable,
                lowering.continuation_compositions(),
            );
            render_step_dispatch_plan(ctx, rendered, callable, lowering.dispatch());
        }
        LateLoweredBoundaryLowering::ClassCtor(lowering) => {
            writeln!(
                rendered,
                "          lowering: ClassCtor class={} result={} emitted={}",
                lowering.class_fqn(),
                ctx.local_label(callable.body_version_key(), lowering.result_local()),
                render_cases(
                    ctx,
                    Some(lowering.facts().emitted_cases().schema()),
                    lowering.facts().emitted_cases().tags(),
                ),
            )
            .unwrap();
            render_source_consumption(ctx, rendered, callable, lowering.source_consumption());
            for emission in lowering.emitted_steps() {
                render_step_case_emission(ctx, rendered, emission);
            }
        }
        LateLoweredBoundaryLowering::Perform(lowering) => {
            writeln!(
                rendered,
                "          lowering: Perform emitted_case={} captured_cont_schema={} payload_tuple_ty={}",
                callable
                    .body_step_schema()
                    .map(|step| ctx.case_ref(step, lowering.facts().emitted_case()))
                    .unwrap_or_else(|| "case_missing".to_string()),
                ctx.continuation_label(lowering.facts().captured_cont_schema()),
                ctx.type_text(lowering.facts().payload_tuple_ty()),
            )
            .unwrap();
            render_perform_operand_contract(ctx, rendered, callable, lowering.operand_contract());
            render_step_case_emission(ctx, rendered, lowering.emitted_step());
        }
        LateLoweredBoundaryLowering::Resume(lowering) => {
            writeln!(
                rendered,
                "          lowering: Resume continuation_schema={} out_step_schema={} result={} runtime_error_boundary={}",
                ctx.continuation_label(lowering.facts().continuation_schema()),
                ctx.step_label(lowering.facts().out_step_schema()),
                ctx.local_label(callable.body_version_key(), lowering.result_local()),
                ctx.boundary_label(callable.body_version_key(), lowering.runtime_error_boundary()),
            )
            .unwrap();
            render_resume_operand_contract(ctx, rendered, callable, lowering.operand_contract());
            render_call_boundary_continuation_compositions(
                ctx,
                rendered,
                callable,
                lowering.continuation_compositions(),
            );
            render_step_dispatch_plan(ctx, rendered, callable, lowering.dispatch());
        }
        LateLoweredBoundaryLowering::RuntimeError(lowering) => {
            writeln!(
                rendered,
                "          lowering: RuntimeError origin={} paired_resume={}",
                ctx.site_label(callable.body_version_key(), lowering.origin_site()),
                ctx.boundary_label(callable.body_version_key(), lowering.resume_boundary()),
            )
            .unwrap();
            render_step_case_emission(ctx, rendered, lowering.emitted_step());
        }
        LateLoweredBoundaryLowering::Handle(lowering) => {
            writeln!(
                rendered,
                "          lowering: Handle result_ty={} classification={:?} handled={} body_outward={} finally_outward={}",
                ctx.type_text(lowering.facts().result_ty()),
                lowering.facts().nested_handle_classification(),
                render_cases(
                    ctx,
                    Some(lowering.facts().handled_cases().schema()),
                    lowering.facts().handled_cases().tags(),
                ),
                render_cases(
                    ctx,
                    Some(lowering.facts().body_outward_cases().schema()),
                    lowering.facts().body_outward_cases().tags(),
                ),
                render_cases(
                    ctx,
                    Some(lowering.facts().finally_outward_cases().schema()),
                    lowering.facts().finally_outward_cases().tags(),
                ),
            )
            .unwrap();
            writeln!(rendered, "            arm_outward_cases:").unwrap();
            if lowering.facts().arm_facts().is_empty() {
                writeln!(rendered, "              <none>").unwrap();
            } else {
                for arm in lowering.facts().arm_facts() {
                    writeln!(
                        rendered,
                        "              - handled={} continuation_schema={} outward={}",
                        ctx.case_ref(
                            lowering.facts().handled_cases().schema(),
                            arm.handled_case()
                        ),
                        ctx.continuation_label(arm.continuation_schema()),
                        render_cases(
                            ctx,
                            Some(arm.arm_outward_cases().schema()),
                            arm.arm_outward_cases().tags(),
                        ),
                    )
                    .unwrap();
                }
            }
            writeln!(rendered, "            outward_emissions:").unwrap();
            if lowering.outward_emissions().is_empty() {
                writeln!(rendered, "              <none>").unwrap();
            } else {
                for emission in lowering.outward_emissions() {
                    render_step_case_emission(ctx, rendered, emission);
                }
            }
        }
    }
}

fn render_step_dispatch_plan(
    ctx: &DumpCtx<'_>,
    rendered: &mut String,
    callable: &LateLoweredCallable,
    dispatch: &LateLoweredStepDispatchPlan,
) {
    writeln!(
        rendered,
        "            dispatch_input_step_schema: {}",
        ctx.step_label(dispatch.input_step_schema())
    )
    .unwrap();
    render_complete_step_dispatch(ctx, rendered, callable, dispatch.complete());
    writeln!(rendered, "            outward_cases:").unwrap();
    if dispatch.outward_cases().is_empty() {
        writeln!(rendered, "              <none>").unwrap();
    } else {
        for forwarding in dispatch.outward_cases() {
            render_step_case_forwarding(ctx, rendered, dispatch.input_step_schema(), forwarding);
        }
    }
}

fn render_consumed_runtime_error_case(
    ctx: &DumpCtx<'_>,
    rendered: &mut String,
    callable: &LateLoweredCallable,
    input_step_schema: crate::effect_facts::StepSchemaId,
    runtime_error_case: &LateLoweredConsumedRuntimeErrorCase,
) {
    writeln!(
        rendered,
        "            consumed_runtime_error_case: in {} op={} payload_tuple_ty={} target={} terminal={}",
        ctx.case_ref(input_step_schema, runtime_error_case.input_case_tag()),
        concrete_op_text(ctx.program, runtime_error_case.input_concrete_op_key()),
        ctx.type_text(runtime_error_case.payload_tuple_ty()),
        ctx.state_label(callable.body_version_key(), runtime_error_case.target_state()),
        render_local_runtime_error_terminal_action(runtime_error_case.terminal_action()),
    )
    .unwrap();
}

fn render_call_boundary_continuation_compositions(
    ctx: &DumpCtx<'_>,
    rendered: &mut String,
    callable: &LateLoweredCallable,
    compositions: &[crate::effect_lowered::ir::LateLoweredCallBoundaryContinuationComposition],
) {
    writeln!(rendered, "            continuation_compositions:").unwrap();
    if compositions.is_empty() {
        writeln!(rendered, "              <none>").unwrap();
        return;
    }
    for composition in compositions {
        writeln!(
            rendered,
            "              - in {} -> out {} callee={} caller={} resume={} result={}{} result_ty={}",
            ctx.case_ref(composition.input_step_schema(), composition.input_case_tag()),
            ctx.case_ref(
                composition.caller_continuation_contract().out_step_schema(),
                composition.output_case_tag(),
            ),
            ctx.continuation_label(composition.callee_continuation_schema()),
            ctx.continuation_label(composition.caller_continuation_schema()),
            ctx.state_label(callable.body_version_key(), composition.caller_resume_state()),
            ctx.local_label(callable.body_version_key(), composition.caller_result_local()),
            composition
                .caller_result_frame_slot()
                .map(|slot| format!(" frame={}", ctx.slot_label(callable.body_version_key(), slot)))
                .unwrap_or_default(),
            ctx.type_text(composition.caller_result_ty()),
        )
        .unwrap();
    }
}

fn render_call_operand_contract(
    ctx: &DumpCtx<'_>,
    rendered: &mut String,
    callable: &LateLoweredCallable,
    contract: &LateLoweredCallBoundaryOperandContract,
) {
    writeln!(rendered, "            operand_contract:").unwrap();
    render_source_consumption(ctx, rendered, callable, contract.source_consumption());
    writeln!(
        rendered,
        "              carrier: {}",
        contract
            .carrier_source()
            .map(|source| render_operand_source(ctx, Some(callable), source))
            .unwrap_or_else(|| "<none>".to_string()),
    )
    .unwrap();
    writeln!(rendered, "              ordered_args:").unwrap();
    render_operand_sources(ctx, rendered, Some(callable), contract.arg_sources());
}

fn render_perform_operand_contract(
    ctx: &DumpCtx<'_>,
    rendered: &mut String,
    callable: &LateLoweredCallable,
    contract: &LateLoweredPerformBoundaryOperandContract,
) {
    writeln!(rendered, "            operand_contract:").unwrap();
    render_source_consumption(ctx, rendered, callable, contract.source_consumption());
    writeln!(rendered, "              payload_sources:").unwrap();
    render_operand_sources(ctx, rendered, Some(callable), contract.payload_sources());
}

fn render_resume_operand_contract(
    ctx: &DumpCtx<'_>,
    rendered: &mut String,
    callable: &LateLoweredCallable,
    contract: &LateLoweredResumeBoundaryOperandContract,
) {
    writeln!(rendered, "            operand_contract:").unwrap();
    render_source_consumption(ctx, rendered, callable, contract.source_consumption());
    writeln!(
        rendered,
        "              continuation: {}",
        render_operand_source(ctx, Some(callable), contract.continuation_source()),
    )
    .unwrap();
    let route = contract.underlying_continuation_route();
    writeln!(
        rendered,
        "              underlying_route: continuation_schema={} via {}",
        ctx.continuation_label(route.continuation_schema()),
        render_surface_resume_dispatch_publication(ctx, route.publication()),
    )
    .unwrap();
    writeln!(rendered, "              ordered_args:").unwrap();
    render_operand_sources(ctx, rendered, Some(callable), contract.arg_sources());
}

fn render_source_consumption(
    ctx: &DumpCtx<'_>,
    rendered: &mut String,
    callable: &LateLoweredCallable,
    consumption: LateLoweredBoundarySourceConsumption,
) {
    match consumption {
        LateLoweredBoundarySourceConsumption::Statement {
            source_slice,
            statement_index,
            consumes_last_statement,
        } => {
            writeln!(
                rendered,
                "              anchor: statement {} stmt{} slice={} slice_stmt_index={} last_in_slice={}",
                ctx.block_label(callable.body_version_key(), source_slice.block_id()),
                statement_index,
                render_state_slice_inline(ctx, callable, source_slice),
                statement_index.saturating_sub(source_slice.start_statement_index()),
                consumes_last_statement,
            )
            .unwrap();
        }
        LateLoweredBoundarySourceConsumption::Terminator { source_slice } => {
            writeln!(
                rendered,
                "              anchor: terminator slice={}",
                render_state_slice_inline(ctx, callable, source_slice),
            )
            .unwrap();
        }
    }
}

fn render_operand_sources(
    ctx: &DumpCtx<'_>,
    rendered: &mut String,
    callable: Option<&LateLoweredCallable>,
    sources: &[LateLoweredOperandSource],
) {
    if sources.is_empty() {
        writeln!(rendered, "                <none>").unwrap();
        return;
    }
    for source in sources {
        writeln!(
            rendered,
            "                - {}",
            render_operand_source(ctx, callable, source)
        )
        .unwrap();
    }
}

fn render_operand_source(
    ctx: &DumpCtx<'_>,
    callable: Option<&LateLoweredCallable>,
    source: &LateLoweredOperandSource,
) -> String {
    let value = match source.value() {
        LateLoweredOperandValueSource::Local(local) => callable
            .map(|callable| ctx.local_label(callable.body_version_key(), *local))
            .unwrap_or_else(|| "local_missing".to_string()),
        LateLoweredOperandValueSource::Const(value) => format!("const({value:?})"),
    };
    format!("{value}:{}", ctx.type_text(source.source_ty()))
}

fn render_completion_payload_source(
    ctx: &DumpCtx<'_>,
    callable: Option<&LateLoweredCallable>,
    source: &LateLoweredCompletionPayloadSource,
) -> String {
    match source {
        LateLoweredCompletionPayloadSource::Unit { complete_ty } => {
            format!("Unit:{}", ctx.type_text(*complete_ty))
        }
        LateLoweredCompletionPayloadSource::Operand(source) => {
            render_operand_source(ctx, callable, source)
        }
    }
}

fn render_complete_step_dispatch(
    ctx: &DumpCtx<'_>,
    rendered: &mut String,
    callable: &LateLoweredCallable,
    complete: &LateLoweredCompleteStepDispatch,
) {
    writeln!(
        rendered,
        "            complete: answer_ty={} target={} result={}",
        ctx.type_text(complete.answer_ty()),
        ctx.state_label(callable.body_version_key(), complete.target_state()),
        complete
            .result_local()
            .map(|local| ctx.local_label(callable.body_version_key(), local))
            .unwrap_or_else(|| "<none>".to_string()),
    )
    .unwrap();
}

fn render_step_case_forwarding(
    ctx: &DumpCtx<'_>,
    rendered: &mut String,
    input_step_schema: crate::effect_facts::StepSchemaId,
    forwarding: &LateLoweredStepCaseForwarding,
) {
    writeln!(
        rendered,
        "              - in {} op={} -> out {} op={} payload_tuple_ty={} {} cont_schema={} out_step_schema={}",
        ctx.case_ref(input_step_schema, forwarding.input_case_tag()),
        concrete_op_text(ctx.program, forwarding.input_concrete_op_key()),
        ctx.case_ref(
            forwarding.emission().continuation_contract().out_step_schema(),
            forwarding.emission().case_tag(),
        ),
        concrete_op_text(ctx.program, forwarding.emission().concrete_op_key()),
        ctx.type_text(forwarding.emission().payload_tuple_ty()),
        ctx.continuation_object_label(forwarding.emission().continuation_object()),
        ctx.continuation_label(
            forwarding
                .emission()
                .continuation_contract()
                .continuation_schema(),
        ),
        ctx.step_label(forwarding.emission().continuation_contract().out_step_schema()),
    )
    .unwrap();
}

fn render_step_case_emission(
    ctx: &DumpCtx<'_>,
    rendered: &mut String,
    emission: &LateLoweredStepCaseEmission,
) {
    writeln!(
        rendered,
        "            emit: {} op={} payload_tuple_ty={} {} cont_schema={} out_step_schema={}",
        ctx.case_ref(
            emission.continuation_contract().out_step_schema(),
            emission.case_tag(),
        ),
        concrete_op_text(ctx.program, emission.concrete_op_key()),
        ctx.type_text(emission.payload_tuple_ty()),
        ctx.continuation_object_label(emission.continuation_object()),
        ctx.continuation_label(emission.continuation_contract().continuation_schema()),
        ctx.step_label(emission.continuation_contract().out_step_schema()),
    )
    .unwrap();
}

fn render_body_version_key(ctx: &DumpCtx<'_>, key: &LateLoweredBodyVersionKey) -> String {
    let _ = ctx;
    body_version_identity(ctx.program, key)
}

fn render_call_target(ctx: &DumpCtx<'_>, target: &crate::effect_facts::CallSiteTarget) -> String {
    match target {
        crate::effect_facts::CallSiteTarget::KnownInstance(instance) => {
            instance_text(ctx.program, instance)
        }
        crate::effect_facts::CallSiteTarget::CandidateSet(instances) => format!(
            "[{}]",
            instances
                .iter()
                .map(|instance| instance_text(ctx.program, instance))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        crate::effect_facts::CallSiteTarget::BodylessDirect { fqn } => {
            format!("BodylessDirect({fqn})")
        }
        crate::effect_facts::CallSiteTarget::DynamicFallback => "DynamicFallback".to_string(),
    }
}

fn render_callable_abi_kind(kind: crate::effect_facts::CallableAbiKind) -> &'static str {
    match kind {
        crate::effect_facts::CallableAbiKind::Plain => "Plain",
        crate::effect_facts::CallableAbiKind::EffectStep => "EffectStep",
    }
}

fn render_resume_interface_ids(ctx: &DumpCtx<'_>, interface_ids: &[ResumeInterfaceId]) -> String {
    if interface_ids.is_empty() {
        return "[]".to_string();
    }
    format!(
        "[{}]",
        interface_ids
            .iter()
            .map(|id| ctx.resume_packing_label(*id))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn render_capture(
    ctx: &DumpCtx<'_>,
    owner: &LateLoweredBodyVersionKey,
    capture: LateLoweredContinuationCapture,
) -> String {
    match capture {
        LateLoweredContinuationCapture::FrameSlot(slot) => {
            format!("FrameSlot({})", ctx.slot_label(owner, slot))
        }
        LateLoweredContinuationCapture::State(state) => {
            format!("State({})", ctx.state_label(owner, state))
        }
    }
}

fn render_continuation_resume_body(body: LateLoweredContinuationResumeBody) -> String {
    match body {
        LateLoweredContinuationResumeBody::ResumeCapturedState { repeated_resume } => {
            format!(
                "ResumeCapturedState(one_shot={})",
                render_one_shot_policy(repeated_resume)
            )
        }
        LateLoweredContinuationResumeBody::Unreachable => "Unreachable".to_string(),
    }
}

fn render_one_shot_policy(policy: LateLoweredOneShotPolicy) -> &'static str {
    match policy {
        LateLoweredOneShotPolicy::OrdinaryRuntimeErrorOutward => "RuntimeErrorOutward",
    }
}

fn render_boundary_source(
    ctx: &DumpCtx<'_>,
    callable: &LateLoweredCallable,
    source: LateLoweredBoundarySource,
) -> String {
    match source {
        LateLoweredBoundarySource::Site { site_id, kind } => {
            format!(
                "{}({})",
                render_boundary_site_kind(kind),
                ctx.site_label(callable.body_version_key(), site_id)
            )
        }
        LateLoweredBoundarySource::RuntimeError { origin_site } => {
            format!(
                "RuntimeError({})",
                ctx.site_label(callable.body_version_key(), origin_site)
            )
        }
    }
}

fn render_boundary_site_kind(kind: BoundarySiteKind) -> &'static str {
    match kind {
        BoundarySiteKind::Call => "Call",
        BoundarySiteKind::ClassCtor => "ClassCtor",
        BoundarySiteKind::Perform => "Perform",
        BoundarySiteKind::Resume => "Resume",
        BoundarySiteKind::Handle => "Handle",
    }
}

fn render_state_role(role: LateLoweredStateRole) -> &'static str {
    match role {
        LateLoweredStateRole::Entry => "Entry",
        LateLoweredStateRole::Segment => "Segment",
        LateLoweredStateRole::Resume => "Resume",
        LateLoweredStateRole::Complete => "Complete",
        LateLoweredStateRole::Cleanup => "Cleanup",
        LateLoweredStateRole::Drop => "Drop",
    }
}

fn render_state_terminator(
    ctx: &DumpCtx<'_>,
    callable: &LateLoweredCallable,
    terminator: &LateLoweredStateTerminator,
) -> String {
    match terminator {
        LateLoweredStateTerminator::Suspend {
            boundary_ids,
            resume_state,
            local_runtime_error_states,
            cleanup_state,
            drop_state,
        } => format!(
            "Suspend(boundaries={}, resume={}, local_runtime_error={}, cleanup={}, drop={})",
            render_boundary_ids(ctx, callable.body_version_key(), boundary_ids),
            ctx.state_label(callable.body_version_key(), *resume_state),
            render_state_successors(ctx, callable.body_version_key(), local_runtime_error_states),
            render_optional_state(ctx, callable.body_version_key(), *cleanup_state),
            render_optional_state(ctx, callable.body_version_key(), *drop_state),
        ),
        LateLoweredStateTerminator::Goto { target } => {
            format!(
                "Goto({})",
                ctx.state_label(callable.body_version_key(), *target)
            )
        }
        LateLoweredStateTerminator::Branch {
            cond_local,
            then_state,
            else_state,
        } => format!(
            "Branch({} ? {} : {})",
            ctx.local_label(callable.body_version_key(), *cond_local),
            ctx.state_label(callable.body_version_key(), *then_state),
            ctx.state_label(callable.body_version_key(), *else_state),
        ),
        LateLoweredStateTerminator::Return {
            payload_source,
            complete_state,
        } => format!(
            "Return({} -> {})",
            render_completion_payload_source(ctx, Some(callable), payload_source),
            ctx.state_label(callable.body_version_key(), *complete_state),
        ),
        LateLoweredStateTerminator::HandleDispatch {
            site_id,
            body_state,
            arm_states,
            finally_state,
            exit_state,
            contract: _,
            boundary_ids,
            drop_state,
        } => format!(
            "Handle({} body={} arms={} finally={} exit={} boundaries={} drop={})",
            ctx.site_label(callable.body_version_key(), *site_id),
            ctx.state_label(callable.body_version_key(), *body_state),
            render_state_successors(ctx, callable.body_version_key(), arm_states),
            render_optional_state(ctx, callable.body_version_key(), *finally_state),
            ctx.state_label(callable.body_version_key(), *exit_state),
            render_boundary_ids(ctx, callable.body_version_key(), boundary_ids),
            render_optional_state(ctx, callable.body_version_key(), *drop_state),
        ),
        LateLoweredStateTerminator::LocalRuntimeError {
            payload_tuple_ty,
            terminal_action,
        } => {
            format!(
                "LocalRuntimeError(payload_tuple_ty={}, terminal={})",
                ctx.type_text(*payload_tuple_ty),
                render_local_runtime_error_terminal_action(*terminal_action)
            )
        }
        LateLoweredStateTerminator::ResumeUnwind => "ResumeUnwind".to_string(),
        LateLoweredStateTerminator::Unreachable => "Unreachable".to_string(),
        LateLoweredStateTerminator::Abandon => "Abandon".to_string(),
    }
}

fn render_local_runtime_error_terminal_action(
    action: LateLoweredLocalRuntimeErrorTerminalAction,
) -> String {
    match action {
        LateLoweredLocalRuntimeErrorTerminalAction::RuntimeFatal { runtime_entry } => {
            format!(
                "RuntimeFatal(runtime_entry={})",
                render_published_runtime_entry(runtime_entry)
            )
        }
    }
}

fn render_handle_pending_completion(
    ctx: &DumpCtx<'_>,
    callable: &LateLoweredCallable,
    pending: LateLoweredHandlePendingCompletion,
) -> String {
    match pending {
        LateLoweredHandlePendingCompletion::ContinueToExit => "ContinueToExit".to_string(),
        LateLoweredHandlePendingCompletion::ReturnFromFunction => "ReturnFromFunction".to_string(),
        LateLoweredHandlePendingCompletion::PropagateOutward(case_tag) => callable
            .body_step_schema()
            .map(|step| format!("PropagateOutward({})", ctx.case_ref(step, case_tag)))
            .unwrap_or_else(|| "PropagateOutward(case_missing)".to_string()),
    }
}

fn render_handle_state_region(
    ctx: &DumpCtx<'_>,
    callable: &LateLoweredCallable,
    region: LateLoweredHandleStateRegion,
) -> String {
    match region {
        LateLoweredHandleStateRegion::OutsideHandle => "outside".to_string(),
        LateLoweredHandleStateRegion::Dispatch => "dispatch".to_string(),
        LateLoweredHandleStateRegion::Body => "body".to_string(),
        LateLoweredHandleStateRegion::Arm {
            handled_case,
            arm_ordinal,
        } => callable
            .body_step_schema()
            .map(|step| {
                format!(
                    "arm({}, ordinal={arm_ordinal})",
                    ctx.case_ref(step, handled_case)
                )
            })
            .unwrap_or_else(|| "arm(case_missing)".to_string()),
        LateLoweredHandleStateRegion::Finally => "finally".to_string(),
        LateLoweredHandleStateRegion::Exit => "exit".to_string(),
    }
}

fn render_handle_boundary_case_routing_action(
    ctx: &DumpCtx<'_>,
    callable: &LateLoweredCallable,
    action: LateLoweredHandleBoundaryCaseRoutingAction,
) -> String {
    match action {
        LateLoweredHandleBoundaryCaseRoutingAction::ConsumeToArm {
            arm_state,
            arm_ordinal,
            continuation_resume_state,
        } => format!(
            "consume_to_arm({}, ordinal={}, resume={})",
            ctx.state_label(callable.body_version_key(), arm_state),
            arm_ordinal,
            ctx.state_label(callable.body_version_key(), continuation_resume_state),
        ),
        LateLoweredHandleBoundaryCaseRoutingAction::PendingCompletion { completion } => {
            format!(
                "pending:{}",
                render_handle_pending_completion(ctx, callable, completion)
            )
        }
        LateLoweredHandleBoundaryCaseRoutingAction::EmitOutward => "emit_outward".to_string(),
    }
}

fn render_published_runtime_entry(entry: LateLoweredPublishedRuntimeEntry) -> &'static str {
    entry.symbol_name()
}

fn render_optional_frame_slot(
    ctx: &DumpCtx<'_>,
    key: &LateLoweredBodyVersionKey,
    slot: Option<super::ir::FrameSlotId>,
) -> String {
    slot.map_or_else(|| "<none>".to_string(), |slot| ctx.slot_label(key, slot))
}

fn render_boundary_ids(
    ctx: &DumpCtx<'_>,
    key: &LateLoweredBodyVersionKey,
    boundary_ids: &[super::ir::BoundaryId],
) -> String {
    if boundary_ids.is_empty() {
        return "[]".to_string();
    }
    format!(
        "[{}]",
        boundary_ids
            .iter()
            .map(|boundary| ctx.boundary_label(key, *boundary))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn render_system_slot_kind(kind: SystemSlotKind) -> &'static str {
    match kind {
        SystemSlotKind::StateTag => "StateTag",
        SystemSlotKind::ResumePayloadCarrier => "ResumePayloadCarrier",
        SystemSlotKind::CleanupFlag => "CleanupFlag",
        SystemSlotKind::OneShotFlag => "OneShotFlag",
        SystemSlotKind::CompletionTag => "CompletionTag",
        SystemSlotKind::CurrentEffectCtx => "CurrentEffectCtx",
    }
}

fn render_optional_state(
    ctx: &DumpCtx<'_>,
    key: &LateLoweredBodyVersionKey,
    state: Option<StateId>,
) -> String {
    state
        .map(|state| ctx.state_label(key, state))
        .unwrap_or_else(|| "<none>".to_string())
}

fn render_state_successors(
    ctx: &DumpCtx<'_>,
    key: &LateLoweredBodyVersionKey,
    successors: &[StateId],
) -> String {
    if successors.is_empty() {
        return "[]".to_string();
    }
    format!(
        "[{}]",
        successors
            .iter()
            .map(|state| ctx.state_label(key, *state))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn render_cases(
    ctx: &DumpCtx<'_>,
    step_schema: Option<crate::effect_facts::StepSchemaId>,
    cases: &[CaseTag],
) -> String {
    if cases.is_empty() {
        return "[]".to_string();
    }
    let rendered = cases
        .iter()
        .map(|tag| {
            step_schema
                .map(|step| ctx.case_ref(step, *tag))
                .unwrap_or_else(|| "case_missing".to_string())
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{rendered}]")
}
