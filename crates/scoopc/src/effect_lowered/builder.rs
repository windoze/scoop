use crate::effect_facts::{CallableAbiKind, MaterializedEffectFacts, SiteEffectFacts};
use crate::mir::{
    BasicBlockId, Body, FunDecl, Item, MaterializedMir, MaterializedMirPassView, Rvalue,
    StatementKind,
};
use crate::ty::TypeStore;

use super::EffectLoweringError;
use super::frame::{FrameBuildInputs, build_callable_frame};
use super::ir::{
    ContinuationObjectId, LateLoweredBodyVersionKey, LateLoweredBoundaryMap, LateLoweredCallable,
    LateLoweredFrameSchema, LateLoweredPlainBodySlice, LateLoweredPlainCallSite,
    LateLoweredPlainCallable, LateLoweredProgram, LateLoweredResumeStateMap, LateLoweredStateGraph,
};
use super::materialize::{
    BoundaryMaterializationInputs, ContinuationObjectMaterializationInputs, StepMaterialization,
    materialize_boundary_map, materialize_completion_payload_bindings,
    materialize_continuation_object, materialize_dynamic_invoke_entry,
    materialize_resume_payload_bindings, materialize_source_statement_classifications,
    materialize_step_and_resume_interfaces,
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
            let body_version_key = LateLoweredBodyVersionKey::new(
                family.key().clone(),
                callable_facts.declared_row().clone(),
                callable_facts.impl_plan(),
                callable_facts.needs_reentry(),
            );

            if matches!(callable_facts.call_abi_kind(), CallableAbiKind::Plain) {
                let fun = family
                    .root_body()
                    .or_else(|| find_materialized_fun(pass_view.materialized(), family.root_fqn()))
                    .ok_or_else(|| EffectLoweringError::MissingPlainCallableSignature {
                        root_fqn: root_fqn.clone(),
                    })?;
                callables.push(LateLoweredCallable::new_plain(
                    root_fqn,
                    body_version_key,
                    callable_facts.resolved_outward_cases().tags().to_vec(),
                    build_plain_callable_abi(fun, effect_facts.body(family.key()))?,
                ));
                continue;
            }

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
                        let segmentation = build_callable_segmentation(
                            &root_fqn,
                            body,
                            body_facts,
                            step_schema.complete_ty(),
                        )?;
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
            let completion_payload_bindings = materialize_completion_payload_bindings(
                &root_fqn,
                step_type,
                &state_graph,
                &frame_schema,
                types,
            )?;
            let frame_schema = frame_schema
                .with_resume_payload_bindings(resume_payload_bindings)
                .with_completion_payload_bindings(completion_payload_bindings);
            let source_statement_classifications =
                match family.root_body().and_then(|fun| fun.body.as_ref()) {
                    Some(body) => materialize_source_statement_classifications(
                        &root_fqn,
                        body,
                        &state_graph,
                        &frame_schema,
                        &boundary_map,
                    )?,
                    None => Vec::new(),
                };
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
            callables.push(
                LateLoweredCallable::new(
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
                )
                .with_source_statement_classifications(source_statement_classifications),
            );
        }

        Ok(LateLoweredProgram::new(
            step_types,
            resume_packings,
            continuation_objects,
            callables,
        ))
    }
}

fn find_materialized_fun<'a>(materialized: &'a MaterializedMir, fqn: &str) -> Option<&'a FunDecl> {
    materialized.file.items.iter().find_map(|item| match item {
        Item::Fun(fun) if fun.fqn == fqn => Some(fun),
        Item::Fun(_) | Item::Todo { .. } => None,
    })
}

fn build_plain_callable_abi(
    fun: &FunDecl,
    body_facts: Option<&crate::effect_facts::BodyEffectFacts>,
) -> Result<LateLoweredPlainCallable, EffectLoweringError> {
    let body_slices = fun.body.as_ref().map(plain_body_slices).unwrap_or_default();
    let call_sites = match (&fun.body, body_facts) {
        (Some(body), Some(body_facts)) => build_plain_call_sites(&fun.fqn, body, body_facts)?,
        (Some(_), None) => {
            return Err(EffectLoweringError::MissingBodyFacts {
                root_fqn: fun.fqn.clone(),
            });
        }
        (None, _) => Vec::new(),
    };

    Ok(LateLoweredPlainCallable::new(
        fun.ty,
        fun.params.iter().map(|param| param.ty).collect(),
        fun.return_ty,
        body_slices,
        call_sites,
    ))
}

fn plain_body_slices(body: &Body) -> Vec<LateLoweredPlainBodySlice> {
    body.blocks
        .iter()
        .enumerate()
        .map(|(block_index, block)| {
            LateLoweredPlainBodySlice::new(
                BasicBlockId::from_raw(block_index as u32),
                0,
                block.stmts.len() as u32,
                true,
            )
        })
        .collect()
}

fn build_plain_call_sites(
    root_fqn: &str,
    body: &Body,
    body_facts: &crate::effect_facts::BodyEffectFacts,
) -> Result<Vec<LateLoweredPlainCallSite>, EffectLoweringError> {
    let mut call_sites = Vec::new();
    for (block_index, block) in body.blocks.iter().enumerate() {
        let source_slice = LateLoweredPlainBodySlice::new(
            BasicBlockId::from_raw(block_index as u32),
            0,
            block.stmts.len() as u32,
            true,
        );
        for (statement_index, statement) in block.stmts.iter().enumerate() {
            let StatementKind::Assign {
                value: Rvalue::Call { site_id, .. },
                ..
            } = &statement.kind
            else {
                continue;
            };
            let site_facts =
                body_facts
                    .site(*site_id)
                    .ok_or_else(|| EffectLoweringError::MissingSiteFacts {
                        root_fqn: root_fqn.to_string(),
                        site_id: site_id.as_u32(),
                    })?;
            let SiteEffectFacts::Call(call_facts) = site_facts else {
                return Err(EffectLoweringError::UnexpectedSiteFactsKind {
                    root_fqn: root_fqn.to_string(),
                    site_id: site_id.as_u32(),
                    expected: "Call",
                    actual: match site_facts {
                        SiteEffectFacts::Call(_) => "Call",
                        SiteEffectFacts::Perform(_) => "Perform",
                        SiteEffectFacts::Resume(_) => "Resume",
                        SiteEffectFacts::Handle(_) => "Handle",
                    },
                });
            };
            call_sites.push(LateLoweredPlainCallSite::new(
                *site_id,
                source_slice,
                statement_index as u32,
                call_facts.clone(),
            ));
        }
    }
    Ok(call_sites)
}
