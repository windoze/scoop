use std::collections::{BTreeSet, HashMap};

use crate::effect_facts::{
    CallableAbiKind, MaterializedEffectFacts, SiteEffectFacts, StepSchemaId,
};
use crate::mir::{
    BasicBlockId, Body, FunDecl, Item, MaterializedMir, MaterializedMirPassView, Rvalue,
    StatementKind,
};
use crate::ty::TypeStore;

use super::EffectLoweringError;
use super::frame::{FrameBuildInputs, augment_frame_for_handle_dispatch, build_callable_frame};
use super::ir::{
    ContinuationObjectId, LateLoweredBodyVersionKey, LateLoweredBoundaryMap, LateLoweredCallable,
    LateLoweredFrameSchema, LateLoweredPlainBodySlice, LateLoweredPlainCallSite,
    LateLoweredPlainCallable, LateLoweredPlainLocalEffectControl, LateLoweredProgram,
    LateLoweredResumeStateMap, LateLoweredStateGraph,
};
use super::materialize::{
    BoundaryMaterializationInputs, ContinuationObjectMaterializationInputs,
    ContinuationRouteOwnerPlan, NominalDirectSupertypeIndex, StepMaterialization,
    build_cross_callable_continuation_provenance, materialize_boundary_map,
    materialize_completion_payload_bindings, materialize_continuation_object,
    materialize_dynamic_invoke_entry, materialize_resume_payload_bindings,
    materialize_source_statement_classifications, materialize_step_and_resume_interfaces,
};
use super::segment::build_callable_segmentation;

/// 把 canonical MIR snapshot + P4 facts 组装成独立 `LateLoweredProgram` 的统一入口。
pub(crate) struct LateLoweredProgramBuilder<'a> {
    pass_view: MaterializedMirPassView<'a>,
    effect_facts: &'a MaterializedEffectFacts,
    types: &'a TypeStore,
    nominal_direct_supertypes: NominalDirectSupertypeIndex,
}

impl<'a> LateLoweredProgramBuilder<'a> {
    pub(crate) fn from_canonical_inputs(
        pass_view: MaterializedMirPassView<'a>,
        effect_facts: &'a MaterializedEffectFacts,
        types: &'a TypeStore,
    ) -> Self {
        Self {
            nominal_direct_supertypes: collect_nominal_direct_supertypes_from_mir_file(
                &pass_view.materialized().file,
            ),
            pass_view,
            effect_facts,
            types,
        }
    }

    pub(crate) fn with_nominal_direct_supertypes(
        mut self,
        nominal_direct_supertypes: NominalDirectSupertypeIndex,
    ) -> Self {
        self.nominal_direct_supertypes = nominal_direct_supertypes;
        self
    }

    pub(crate) fn build(self) -> Result<LateLoweredProgram, EffectLoweringError> {
        let pass_view = self.pass_view;
        let effect_facts = self.effect_facts;
        let types = self.types;
        let nominal_direct_supertypes = self.nominal_direct_supertypes;

        let StepMaterialization {
            step_types,
            resume_packings,
            resume_packing_ids_by_step,
            resume_packing_ids_by_group,
        } = materialize_step_and_resume_interfaces(effect_facts)?;

        let continuation_route_owner_plans =
            plan_continuation_route_owners(&pass_view, effect_facts);
        let cross_callable_continuation_provenance = build_cross_callable_continuation_provenance(
            &pass_view,
            effect_facts,
            &continuation_route_owner_plans,
        )?;

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
                let plain = build_plain_callable_abi(PlainCallableBuildInputs {
                    root_fqn: &root_fqn,
                    body_version_key: &body_version_key,
                    fun,
                    body_facts: effect_facts.body(family.key()),
                    effect_facts,
                    step_types: &step_types,
                    resume_packing_ids_by_step: &resume_packing_ids_by_step,
                    resume_packing_ids_by_group: &resume_packing_ids_by_group,
                    continuation_object_id: ContinuationObjectId::new(
                        continuation_objects.len() as u32
                    ),
                    cross_callable_continuation_provenance: Some(
                        &cross_callable_continuation_provenance,
                    ),
                    nominal_direct_supertypes: &nominal_direct_supertypes,
                    types,
                })?;
                if let Some(object) = plain.continuation_object {
                    continuation_objects.push(object);
                }
                callables.push(LateLoweredCallable::new_plain(
                    root_fqn,
                    body_version_key,
                    callable_facts.resolved_outward_cases().tags().to_vec(),
                    plain.callable,
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
            let (state_graph, frame_schema, _continuation_captures, boundary_map, resume_state_map) =
                match family.root_body().and_then(|fun| fun.body.as_ref()) {
                    Some(body) => {
                        let body_facts = effect_facts.body(family.key()).ok_or_else(|| {
                            EffectLoweringError::MissingBodyFacts {
                                root_fqn: root_fqn.clone(),
                            }
                        })?;
                        let segmentation = build_callable_segmentation(
                            &root_fqn,
                            self.types,
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
                        nominal_direct_supertypes: &nominal_direct_supertypes,
                        cross_callable_continuation_provenance: Some(
                            &cross_callable_continuation_provenance,
                        ),
                    })?
                }
                None => super::materialize::BoundaryMaterialization {
                    state_graph: state_graph.clone(),
                    boundary_map: LateLoweredBoundaryMap::empty(),
                },
            };
            let state_graph = boundary_map.state_graph;
            let boundary_map = boundary_map.boundary_map;
            let builtins =
                types
                    .builtins()
                    .ok_or_else(|| EffectLoweringError::MissingBuiltinTypes {
                        root_fqn: root_fqn.clone(),
                    })?;
            let frame = augment_frame_for_handle_dispatch(
                &frame_schema,
                &boundary_map,
                &state_graph,
                builtins.any,
            );
            let frame_schema = frame.frame_schema;
            let continuation_captures = frame.continuation_captures;
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
        _ => None,
    })
}

pub(crate) fn collect_nominal_direct_supertypes_from_mir_file(
    file: &crate::mir::File,
) -> NominalDirectSupertypeIndex {
    let mut out = NominalDirectSupertypeIndex::new();
    for item in &file.items {
        match item {
            Item::Metadata(crate::mir::MetadataRoot::Nominal(nominal)) => {
                let supers = nominal
                    .supertypes
                    .iter()
                    .filter_map(|supertype| supertype.fqn.clone())
                    .collect::<Vec<_>>();
                out.insert(nominal.fqn.clone(), supers);
            }
            Item::Metadata(crate::mir::MetadataRoot::Object(object)) => {
                let supers = object
                    .supertypes
                    .iter()
                    .filter_map(|supertype| supertype.fqn.clone())
                    .collect::<Vec<_>>();
                out.insert(object.fqn.clone(), supers);
            }
            _ => {}
        }
    }
    out
}

fn plan_continuation_route_owners(
    pass_view: &MaterializedMirPassView<'_>,
    effect_facts: &MaterializedEffectFacts,
) -> HashMap<String, ContinuationRouteOwnerPlan> {
    let mut plans = HashMap::new();
    let mut next_object_id = 0u32;

    for family in pass_view.instances() {
        let Some(callable_facts) = effect_facts.callable_facts().get(family.key()) else {
            continue;
        };
        let body_version_key = LateLoweredBodyVersionKey::new(
            family.key().clone(),
            callable_facts.declared_row().clone(),
            callable_facts.impl_plan(),
            callable_facts.needs_reentry(),
        );
        let needs_continuation_object = match callable_facts.call_abi_kind() {
            CallableAbiKind::EffectStep => true,
            CallableAbiKind::Plain => effect_facts
                .body(family.key())
                .is_some_and(plain_body_has_local_effect_control),
        };
        if !needs_continuation_object {
            continue;
        }
        plans.insert(
            family.root_fqn().to_string(),
            ContinuationRouteOwnerPlan::new(
                body_version_key,
                ContinuationObjectId::new(next_object_id),
            ),
        );
        next_object_id = next_object_id.saturating_add(1);
    }

    plans
}

struct PlainCallableBuildInputs<'a> {
    root_fqn: &'a str,
    body_version_key: &'a LateLoweredBodyVersionKey,
    fun: &'a FunDecl,
    body_facts: Option<&'a crate::effect_facts::BodyEffectFacts>,
    effect_facts: &'a MaterializedEffectFacts,
    step_types: &'a [crate::effect_lowered::ir::LateLoweredStepType],
    resume_packing_ids_by_step: &'a std::collections::BTreeMap<
        StepSchemaId,
        Vec<crate::effect_lowered::ir::ResumeInterfaceId>,
    >,
    resume_packing_ids_by_group: &'a std::collections::BTreeMap<
        (StepSchemaId, crate::effect_facts::EffectFamilyKey),
        crate::effect_lowered::ir::ResumeInterfaceId,
    >,
    continuation_object_id: ContinuationObjectId,
    cross_callable_continuation_provenance:
        Option<&'a super::materialize::CrossCallableContinuationProvenance>,
    nominal_direct_supertypes: &'a NominalDirectSupertypeIndex,
    types: &'a TypeStore,
}

struct PlainCallableBuildOutput {
    callable: LateLoweredPlainCallable,
    continuation_object: Option<crate::effect_lowered::ir::LateLoweredContinuationObject>,
}

fn build_plain_callable_abi(
    inputs: PlainCallableBuildInputs<'_>,
) -> Result<PlainCallableBuildOutput, EffectLoweringError> {
    let PlainCallableBuildInputs {
        root_fqn,
        body_version_key,
        fun,
        body_facts,
        effect_facts,
        step_types,
        resume_packing_ids_by_step,
        resume_packing_ids_by_group,
        continuation_object_id,
        cross_callable_continuation_provenance,
        nominal_direct_supertypes,
        types,
    } = inputs;
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
    let local_effect_control = match (&fun.body, body_facts) {
        (Some(body), Some(body_facts)) if plain_body_has_local_effect_control(body_facts) => Some(
            build_plain_local_effect_control(PlainLocalEffectControlBuildInputs {
                root_fqn,
                body_version_key,
                body,
                body_facts,
                effect_facts,
                step_types,
                resume_packing_ids_by_step,
                resume_packing_ids_by_group,
                continuation_object_id,
                cross_callable_continuation_provenance,
                nominal_direct_supertypes,
                types,
                return_ty: fun.return_ty,
            })?,
        ),
        (Some(_), Some(_)) | (None, _) => None,
        (Some(_), None) => unreachable!("body_facts absence handled before local control build"),
    };
    let (local_effect_control, continuation_object) = match local_effect_control {
        Some(output) => (Some(output.control), Some(output.continuation_object)),
        None => (None, None),
    };

    Ok(PlainCallableBuildOutput {
        callable: LateLoweredPlainCallable::new(
            fun.ty,
            fun.params.iter().map(|param| param.ty).collect(),
            fun.return_ty,
            body_slices,
            call_sites,
            local_effect_control,
        ),
        continuation_object,
    })
}

struct PlainLocalEffectControlBuildInputs<'a> {
    root_fqn: &'a str,
    body_version_key: &'a LateLoweredBodyVersionKey,
    body: &'a Body,
    body_facts: &'a crate::effect_facts::BodyEffectFacts,
    effect_facts: &'a MaterializedEffectFacts,
    step_types: &'a [crate::effect_lowered::ir::LateLoweredStepType],
    resume_packing_ids_by_step: &'a std::collections::BTreeMap<
        StepSchemaId,
        Vec<crate::effect_lowered::ir::ResumeInterfaceId>,
    >,
    resume_packing_ids_by_group: &'a std::collections::BTreeMap<
        (StepSchemaId, crate::effect_facts::EffectFamilyKey),
        crate::effect_lowered::ir::ResumeInterfaceId,
    >,
    continuation_object_id: ContinuationObjectId,
    cross_callable_continuation_provenance:
        Option<&'a super::materialize::CrossCallableContinuationProvenance>,
    nominal_direct_supertypes: &'a NominalDirectSupertypeIndex,
    types: &'a TypeStore,
    return_ty: crate::ty::TypeId,
}

struct PlainLocalEffectControlBuildOutput {
    control: LateLoweredPlainLocalEffectControl,
    continuation_object: crate::effect_lowered::ir::LateLoweredContinuationObject,
}

fn build_plain_local_effect_control(
    inputs: PlainLocalEffectControlBuildInputs<'_>,
) -> Result<PlainLocalEffectControlBuildOutput, EffectLoweringError> {
    let PlainLocalEffectControlBuildInputs {
        root_fqn,
        body_version_key,
        body,
        body_facts,
        effect_facts,
        step_types,
        resume_packing_ids_by_step,
        resume_packing_ids_by_group,
        continuation_object_id,
        cross_callable_continuation_provenance,
        nominal_direct_supertypes,
        types,
        return_ty,
    } = inputs;
    let step_schema_id =
        discover_plain_local_effect_control_step_schema(root_fqn, body_facts, effect_facts)?;
    let step_schema = effect_facts
        .step_schemas()
        .get(&step_schema_id)
        .ok_or_else(|| EffectLoweringError::MissingStepSchema {
            root_fqn: root_fqn.to_string(),
            step_schema: step_schema_id.as_u32(),
        })?;
    if step_schema.complete_ty() != return_ty {
        return Err(
            EffectLoweringError::InvalidPlainLocalEffectControlContract {
                root_fqn: root_fqn.to_string(),
                detail: format!(
                    "local effect/control StepSchema s{} complete_ty=t{} 与 plain return_ty=t{} 不一致",
                    step_schema_id.as_u32(),
                    step_schema.complete_ty().as_u32(),
                    return_ty.as_u32(),
                ),
            },
        );
    }
    let step_type = step_types
        .iter()
        .find(|step_type| step_type.step_schema() == step_schema_id)
        .ok_or_else(|| EffectLoweringError::MissingStepSchema {
            root_fqn: root_fqn.to_string(),
            step_schema: step_schema_id.as_u32(),
        })?;
    let segmentation =
        build_callable_segmentation(root_fqn, types, body, body_facts, step_schema.complete_ty())?;
    let local_case_tags = step_schema
        .cases()
        .iter()
        .map(|case| case.case_tag())
        .collect::<Vec<_>>();
    let frame = build_callable_frame(FrameBuildInputs {
        root_fqn,
        body,
        _body_facts: body_facts,
        step_schema_id,
        step_schema,
        continuation_schemas: effect_facts.continuation_schemas(),
        resolved_outward_cases: &local_case_tags,
        impl_plan: crate::effect_facts::ImplPlan::CanonicalFull,
        state_graph: &segmentation.state_graph,
        boundary_map: &segmentation.boundary_map,
        types,
    })?;
    let boundary_map = materialize_boundary_map(BoundaryMaterializationInputs {
        root_fqn,
        owner_version_key: body_version_key,
        body,
        body_facts,
        step_type,
        state_graph: &frame.state_graph,
        frame_schema: &frame.frame_schema,
        boundary_map: &segmentation.boundary_map,
        continuation_object: continuation_object_id,
        step_types,
        types,
        nominal_direct_supertypes,
        cross_callable_continuation_provenance,
    })?;
    let state_graph = boundary_map.state_graph;
    let boundary_map = boundary_map.boundary_map;
    let builtins = types
        .builtins()
        .ok_or_else(|| EffectLoweringError::MissingBuiltinTypes {
            root_fqn: root_fqn.to_string(),
        })?;
    let frame = augment_frame_for_handle_dispatch(
        &frame.frame_schema,
        &boundary_map,
        &state_graph,
        builtins.any,
    );
    let frame_schema = frame.frame_schema;
    let continuation_captures = frame.continuation_captures;
    let resume_payload_bindings =
        materialize_resume_payload_bindings(root_fqn, &frame_schema, &boundary_map)?;
    let completion_payload_bindings = materialize_completion_payload_bindings(
        root_fqn,
        step_type,
        &state_graph,
        &frame_schema,
        types,
    )?;
    let frame_schema = frame_schema
        .with_resume_payload_bindings(resume_payload_bindings)
        .with_completion_payload_bindings(completion_payload_bindings);
    let source_statement_classifications = materialize_source_statement_classifications(
        root_fqn,
        body,
        &state_graph,
        &frame_schema,
        &boundary_map,
    )?;
    let resume_packings = resume_packing_ids_by_step
        .get(&step_schema_id)
        .cloned()
        .unwrap_or_default();
    let continuation_object =
        materialize_continuation_object(ContinuationObjectMaterializationInputs {
            continuation_object_id,
            owner_version_key: body_version_key.clone(),
            step_schema_id,
            step_schema,
            implemented_packings: &resume_packings,
            resume_packing_ids_by_group,
            captures: continuation_captures,
            effect_facts,
        })?;
    Ok(PlainLocalEffectControlBuildOutput {
        control: LateLoweredPlainLocalEffectControl::new(
            step_schema_id,
            state_graph,
            frame_schema,
            boundary_map,
            segmentation.resume_state_map,
            source_statement_classifications,
            continuation_object_id,
            resume_packings,
        ),
        continuation_object,
    })
}

fn plain_body_has_local_effect_control(body_facts: &crate::effect_facts::BodyEffectFacts) -> bool {
    body_facts.sites().values().any(|site| match site {
        SiteEffectFacts::Call(facts) => !facts.resolved_cases().is_empty(),
        SiteEffectFacts::ClassCtor(facts) => !facts.emitted_cases().is_empty(),
        SiteEffectFacts::Perform(_) | SiteEffectFacts::Resume(_) | SiteEffectFacts::Handle(_) => {
            true
        }
    })
}

fn discover_plain_local_effect_control_step_schema(
    root_fqn: &str,
    body_facts: &crate::effect_facts::BodyEffectFacts,
    effect_facts: &MaterializedEffectFacts,
) -> Result<StepSchemaId, EffectLoweringError> {
    if let Some(step_schema) = body_facts.local_control_step_schema() {
        return Ok(step_schema);
    }

    let mut candidates = BTreeSet::new();
    for site in body_facts.sites().values() {
        match site {
            SiteEffectFacts::ClassCtor(facts) => {
                if !facts.emitted_cases().is_empty() {
                    candidates.insert(facts.emitted_cases().schema());
                }
            }
            SiteEffectFacts::Perform(facts) => {
                push_continuation_owner_step_schema(
                    root_fqn,
                    facts.captured_cont_schema(),
                    effect_facts,
                    &mut candidates,
                )?;
            }
            SiteEffectFacts::Handle(facts) => {
                for arm in facts.arm_facts() {
                    push_continuation_owner_step_schema(
                        root_fqn,
                        arm.continuation_schema(),
                        effect_facts,
                        &mut candidates,
                    )?;
                }
            }
            SiteEffectFacts::Call(_) | SiteEffectFacts::Resume(_) => {}
        }
    }
    match candidates.len() {
        1 => Ok(*candidates.iter().next().expect("one candidate exists")),
        0 => Err(
            EffectLoweringError::InvalidPlainLocalEffectControlContract {
                root_fqn: root_fqn.to_string(),
                detail:
                    "plain body 含本地 effect/control，但 P4/P5 未发布可归属的 owner StepSchema"
                        .to_string(),
            },
        ),
        _ => Err(
            EffectLoweringError::InvalidPlainLocalEffectControlContract {
                root_fqn: root_fqn.to_string(),
                detail: format!(
                    "plain body 本地 effect/control 对应多个 owner StepSchema：{}",
                    candidates
                        .iter()
                        .map(|schema| format!("s{}", schema.as_u32()))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            },
        ),
    }
}

fn push_continuation_owner_step_schema(
    root_fqn: &str,
    continuation_schema: crate::effect_facts::ContinuationSchemaId,
    effect_facts: &MaterializedEffectFacts,
    candidates: &mut BTreeSet<StepSchemaId>,
) -> Result<(), EffectLoweringError> {
    let schema = effect_facts
        .continuation_schemas()
        .get(&continuation_schema)
        .ok_or_else(
            || EffectLoweringError::InvalidPlainLocalEffectControlContract {
                root_fqn: root_fqn.to_string(),
                detail: format!(
                    "本地 continuation schema k{} 缺少 authoritative schema contract",
                    continuation_schema.as_u32()
                ),
            },
        )?;
    candidates.insert(schema.out_step_schema());
    Ok(())
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
                continue;
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
