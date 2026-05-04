use crate::effect_facts::MaterializedEffectFacts;
use crate::mir::MaterializedMirPassView;
use crate::ty::TypeStore;

use super::EffectLoweringError;
use super::frame::{FrameBuildInputs, build_callable_frame};
use super::ir::{
    ContinuationObjectId, LateLoweredBodyVersionKey, LateLoweredBoundaryMap, LateLoweredCallable,
    LateLoweredFrameSchema, LateLoweredProgram, LateLoweredResumeStateMap, LateLoweredStateGraph,
};
use super::materialize::{
    BoundaryMaterializationInputs, ContinuationObjectMaterializationInputs, StepMaterialization,
    materialize_boundary_map, materialize_continuation_object, materialize_dynamic_invoke_entry,
    materialize_resume_payload_bindings, materialize_step_and_resume_interfaces,
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

        let StepMaterialization {
            step_types,
            resume_packings,
            resume_packing_ids_by_step,
            resume_packing_ids_by_group,
        } = materialize_step_and_resume_interfaces(effect_facts)?;

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
            let step_type = step_types
                .iter()
                .find(|step_type| step_type.step_schema() == step_schema_id)
                .expect("every step schema should publish a canonical Step shell");
            let resume_packing_ids = resume_packing_ids_by_step
                .get(&step_schema_id)
                .cloned()
                .unwrap_or_default();
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
            let boundary_map = match family.root_body().and_then(|fun| fun.body.as_ref()) {
                Some(body) => {
                    let body_facts = effect_facts.body(family.key()).ok_or_else(|| {
                        EffectLoweringError::MissingBodyFacts {
                            root_fqn: root_fqn.clone(),
                        }
                    })?;
                    materialize_boundary_map(BoundaryMaterializationInputs {
                        root_fqn: &root_fqn,
                        owner_version_key: &body_version_key,
                        body,
                        body_facts,
                        step_type,
                        state_graph: &state_graph,
                        frame_schema: &frame_schema,
                        boundary_map: &boundary_map,
                        continuation_object: continuation_object_id,
                        step_types: &step_types,
                        types,
                    })?
                }
                None => super::materialize::BoundaryMaterialization {
                    state_graph: state_graph.clone(),
                    boundary_map: LateLoweredBoundaryMap::empty(),
                },
            };
            let state_graph = boundary_map.state_graph;
            let boundary_map = boundary_map.boundary_map;
            let resume_payload_bindings =
                materialize_resume_payload_bindings(&root_fqn, &frame_schema, &boundary_map)?;
            let frame_schema = frame_schema.with_resume_payload_bindings(resume_payload_bindings);
            continuation_objects.push(materialize_continuation_object(
                ContinuationObjectMaterializationInputs {
                    continuation_object_id,
                    owner_version_key: body_version_key.clone(),
                    step_schema_id,
                    step_schema,
                    implemented_packings: &resume_packing_ids,
                    resume_packing_ids_by_group: &resume_packing_ids_by_group,
                    captures: continuation_captures,
                    effect_facts,
                },
            )?);
            callables.push(LateLoweredCallable::new(
                family.root_fqn().to_string(),
                body_version_key,
                step_schema_id,
                callable_facts.resolved_outward_cases().tags().to_vec(),
                materialize_dynamic_invoke_entry(
                    step_schema_id,
                    step_type,
                    state_graph.entry_state(),
                    state_graph.complete_state(),
                ),
                state_graph,
                frame_schema,
                boundary_map,
                resume_state_map,
                continuation_object_id,
                resume_packing_ids,
            ));
        }

        Ok(LateLoweredProgram::new(
            step_types,
            resume_packings,
            continuation_objects,
            callables,
        ))
    }
}
