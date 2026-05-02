use std::collections::BTreeMap;

use crate::effect_facts::{ImplPlan, MaterializedEffectFacts, StepSchema, StepSchemaId};
use crate::mir::MaterializedMirPassView;
use crate::ty::TypeStore;

use super::EffectLoweringError;
use super::frame::{FrameBuildInputs, build_callable_frame};
use super::ir::{
    ContinuationObjectId, LateLoweredBodyVersionKey, LateLoweredBoundaryMap, LateLoweredCallable,
    LateLoweredContinuationCapture, LateLoweredContinuationMethod,
    LateLoweredContinuationMethodReachability, LateLoweredContinuationObject,
    LateLoweredDynamicInvokeEntry, LateLoweredFrameSchema, LateLoweredProgram,
    LateLoweredResumeInterface, LateLoweredResumeMethod, LateLoweredResumeStateMap,
    LateLoweredStateGraph, LateLoweredStepCase, LateLoweredStepType, ResumeInterfaceId,
};
use super::segment::build_callable_segmentation;

/// 把 canonical MIR snapshot + P4 facts 组装成独立 `LateLoweredProgram` 的统一入口。
pub(crate) struct LateLoweredProgramBuilder<'a> {
    pass_view: MaterializedMirPassView<'a>,
    effect_facts: &'a MaterializedEffectFacts,
    types: &'a TypeStore,
}

impl<'a> LateLoweredProgramBuilder<'a> {
    pub(crate) fn from_canonical_inputs(
        pass_view: MaterializedMirPassView<'a>,
        effect_facts: &'a MaterializedEffectFacts,
        types: &'a TypeStore,
    ) -> Self {
        Self {
            pass_view,
            effect_facts,
            types,
        }
    }

    pub(crate) fn build(self) -> Result<LateLoweredProgram, EffectLoweringError> {
        let pass_view = self.pass_view;
        let effect_facts = self.effect_facts;
        let types = self.types;

        let mut step_types = Vec::with_capacity(effect_facts.step_schemas().len());
        let mut resume_interfaces = Vec::with_capacity(effect_facts.step_schemas().len());
        let mut resume_interface_ids = BTreeMap::new();

        for (index, (&step_schema_id, step_schema)) in
            effect_facts.step_schemas().iter().enumerate()
        {
            let interface_id = ResumeInterfaceId::new(index as u32);
            step_types.push(build_step_type(step_schema_id, step_schema));
            resume_interfaces.push(build_resume_interface(
                interface_id,
                step_schema_id,
                step_schema,
                effect_facts,
            )?);
            resume_interface_ids.insert(step_schema_id, interface_id);
        }

        let mut continuation_objects = Vec::with_capacity(effect_facts.callable_facts().len());
        let mut callables = Vec::with_capacity(effect_facts.callable_facts().len());

        for family in pass_view.instances() {
            let root_fqn = family.root_fqn().to_string();
            let Some(callable_facts) = effect_facts.callable_facts().get(family.key()) else {
                if family.root_body().is_some() {
                    return Err(EffectLoweringError::MissingCallableFacts {
                        root_fqn: root_fqn.clone(),
                    });
                }
                continue;
            };
            let step_schema_id = callable_facts.step_schema();
            let step_schema = effect_facts
                .step_schemas()
                .get(&step_schema_id)
                .ok_or_else(|| EffectLoweringError::MissingStepSchema {
                    root_fqn: root_fqn.clone(),
                    step_schema: step_schema_id.as_u32(),
                })?;

            if callable_facts.invoke_args_tuple_ty() != step_schema.invoke_args_tuple_ty() {
                return Err(EffectLoweringError::InvokeArgsTupleMismatch {
                    root_fqn,
                    step_schema: step_schema_id.as_u32(),
                    callable_args_tuple: callable_facts.invoke_args_tuple_ty().as_u32(),
                    step_args_tuple: step_schema.invoke_args_tuple_ty().as_u32(),
                });
            }

            let body_version_key = LateLoweredBodyVersionKey::new(
                family.key().clone(),
                callable_facts.declared_row().clone(),
                callable_facts.impl_plan(),
                callable_facts.needs_reentry(),
            );
            let resume_interface_id = *resume_interface_ids
                .get(&step_schema_id)
                .expect("every step schema should publish a resume interface shell");
            let continuation_object_id =
                ContinuationObjectId::new(continuation_objects.len() as u32);
            let (state_graph, frame_schema, continuation_captures, boundary_map, resume_state_map) =
                match family.root_body().and_then(|fun| fun.body.as_ref()) {
                    Some(body) => {
                        let body_facts = effect_facts.body(family.key()).ok_or_else(|| {
                            EffectLoweringError::MissingBodyFacts {
                                root_fqn: root_fqn.clone(),
                            }
                        })?;
                        let segmentation =
                            build_callable_segmentation(&root_fqn, body, body_facts)?;
                        let frame = build_callable_frame(FrameBuildInputs {
                            root_fqn: &root_fqn,
                            body,
                            _body_facts: body_facts,
                            step_schema_id,
                            step_schema,
                            continuation_schemas: effect_facts.continuation_schemas(),
                            resolved_outward_cases: callable_facts.resolved_outward_cases().tags(),
                            impl_plan: callable_facts.impl_plan(),
                            state_graph: &segmentation.state_graph,
                            boundary_map: &segmentation.boundary_map,
                            types,
                        })?;
                        (
                            frame.state_graph,
                            frame.frame_schema,
                            frame.continuation_captures,
                            segmentation.boundary_map,
                            segmentation.resume_state_map,
                        )
                    }
                    None => (
                        LateLoweredStateGraph::minimal_shell(),
                        LateLoweredFrameSchema::empty(),
                        Vec::new(),
                        LateLoweredBoundaryMap::empty(),
                        LateLoweredResumeStateMap::empty(),
                    ),
                };
            continuation_objects.push(build_continuation_object(
                continuation_object_id,
                body_version_key.clone(),
                step_schema,
                resume_interface_id,
                continuation_captures,
            ));
            callables.push(LateLoweredCallable::new(
                family.root_fqn().to_string(),
                body_version_key,
                step_schema_id,
                callable_facts.resolved_outward_cases().tags().to_vec(),
                LateLoweredDynamicInvokeEntry::new(
                    step_schema.invoke_args_tuple_ty(),
                    step_schema_id,
                ),
                state_graph,
                frame_schema,
                boundary_map,
                resume_state_map,
                continuation_object_id,
                vec![resume_interface_id],
            ));
        }

        Ok(LateLoweredProgram::new(
            step_types,
            resume_interfaces,
            continuation_objects,
            callables,
        ))
    }
}

fn build_step_type(step_schema_id: StepSchemaId, step_schema: &StepSchema) -> LateLoweredStepType {
    LateLoweredStepType::new(
        step_schema_id,
        step_schema.invoke_args_tuple_ty(),
        step_schema.complete_ty(),
        step_schema.continuation_obj_ty(),
        step_schema
            .cases()
            .iter()
            .map(|case| {
                LateLoweredStepCase::new(
                    case.case_tag(),
                    case.concrete_op_key().clone(),
                    case.payload_tuple_ty(),
                    case.continuation_schema(),
                )
            })
            .collect(),
    )
}

fn build_resume_interface(
    interface_id: ResumeInterfaceId,
    step_schema_id: StepSchemaId,
    step_schema: &StepSchema,
    effect_facts: &MaterializedEffectFacts,
) -> Result<LateLoweredResumeInterface, EffectLoweringError> {
    let mut methods = Vec::with_capacity(step_schema.cases().len());
    for case in step_schema.cases() {
        let continuation_schema = effect_facts
            .continuation_schemas()
            .get(&case.continuation_schema())
            .ok_or_else(|| EffectLoweringError::MissingContinuationSchema {
                step_schema: step_schema_id.as_u32(),
                continuation_schema: case.continuation_schema().as_u32(),
                case_tag: case.case_tag().as_u32(),
            })?;
        methods.push(LateLoweredResumeMethod::new(
            case.case_tag(),
            case.concrete_op_key().clone(),
            case.continuation_schema(),
            continuation_schema.resume_tuple_ty(),
        ));
    }
    Ok(LateLoweredResumeInterface::new(
        interface_id,
        step_schema_id,
        methods,
    ))
}

fn build_continuation_object(
    continuation_object_id: ContinuationObjectId,
    owner_version_key: LateLoweredBodyVersionKey,
    step_schema: &StepSchema,
    resume_interface_id: ResumeInterfaceId,
    captures: Vec<LateLoweredContinuationCapture>,
) -> LateLoweredContinuationObject {
    let methods = step_schema
        .cases()
        .iter()
        .map(|case| {
            LateLoweredContinuationMethod::new(
                resume_interface_id,
                case.case_tag(),
                continuation_method_reachability(owner_version_key.impl_plan(), case.case_tag()),
            )
        })
        .collect();

    LateLoweredContinuationObject::new(
        continuation_object_id,
        owner_version_key,
        step_schema.continuation_obj_ty(),
        vec![resume_interface_id],
        captures,
        methods,
    )
}

fn continuation_method_reachability(
    impl_plan: ImplPlan,
    case_tag: crate::effect_facts::CaseTag,
) -> LateLoweredContinuationMethodReachability {
    match impl_plan {
        ImplPlan::NoOutward => LateLoweredContinuationMethodReachability::Unreachable,
        ImplPlan::SingleCase(selected) if selected == case_tag => {
            LateLoweredContinuationMethodReachability::Reachable
        }
        ImplPlan::SingleCase(_) => LateLoweredContinuationMethodReachability::Unreachable,
        ImplPlan::CanonicalFull => LateLoweredContinuationMethodReachability::Reachable,
    }
}
