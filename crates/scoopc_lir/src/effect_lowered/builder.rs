use std::collections::HashMap;
use std::path::Path;

use scoopc_ids::{LirCallableHash, LirCallableId};
use scoopc_lir_facts::LirCallableRef;
use scoopc_mir_facts::MirFacts;

use crate::effect_facts::{
    CallableAbiKind, ConcreteOpKey, MaterializedEffectFacts, SiteEffectFacts, StepSchemaId,
};
use crate::mir::placeholder_inventory::validate_body_for_lir_lift;
use crate::mir::{
    BasicBlockId, Body, FunDecl, Item, MaterializedMir, MaterializedMirPassView, Rvalue,
    StatementKind, build_body_labels_for_dump,
};
use crate::ty::{TypeId, TypeStore};

use super::EffectLoweringError;
use super::frame::{FrameBuildInputs, augment_frame_for_handle_dispatch, build_callable_frame};
use super::ir::{
    ContinuationObjectId, LateLoweredBodyVersionKey, LateLoweredBoundaryMap,
    LateLoweredCallSiteMaterializedKind, LateLoweredCallSiteMaterializedMetadata,
    LateLoweredCallable, LateLoweredClassCtorDelegation, LateLoweredClassCtorInitBody,
    LateLoweredClassCtorInitStep, LateLoweredClassCtorParam,
    LateLoweredClassCtorSourceCallContract, LateLoweredClassCtorSuperCall, LateLoweredFrameSchema,
    LateLoweredPlainBodySlice, LateLoweredPlainCallSite, LateLoweredPlainCallable,
    LateLoweredPlainLocalEffectControl, LateLoweredProgram, LateLoweredResumeStateMap,
    LateLoweredStateGraph, class_ctor_source as source,
};
use super::lift::LirLiftContext;
use super::materialize::{
    BoundaryMaterializationInputs, ContinuationObjectMaterializationInputs,
    ContinuationRouteOwnerPlan, NominalDirectSupertypeIndex, StepMaterialization,
    build_cross_callable_continuation_provenance, materialize_boundary_map,
    materialize_completion_payload_bindings, materialize_continuation_object,
    materialize_dynamic_invoke_entry, materialize_resume_payload_bindings,
    materialize_source_statement_classifications, materialize_step_and_resume_interfaces,
};
use super::segment::build_callable_segmentation;

/// 把 canonical MIR pass query、MIR-owned facts 与 P4 facts 组装成独立 `LateLoweredProgram`。
pub struct LateLoweredProgramBuilder<'a> {
    pass_view: MaterializedMirPassView<'a>,
    effect_facts: &'a MaterializedEffectFacts,
    mir_facts: &'a MirFacts,
    types: &'a TypeStore,
    nominal_direct_supertypes: NominalDirectSupertypeIndex,
}

impl<'a> LateLoweredProgramBuilder<'a> {
    pub fn from_canonical_inputs(
        pass_view: MaterializedMirPassView<'a>,
        effect_facts: &'a MaterializedEffectFacts,
        types: &'a TypeStore,
        mir_facts: &'a MirFacts,
    ) -> Self {
        Self {
            nominal_direct_supertypes: nominal_direct_supertypes_from_mir_facts(mir_facts),
            pass_view,
            effect_facts,
            mir_facts,
            types,
        }
    }

    pub fn build(self) -> Result<LateLoweredProgram, EffectLoweringError> {
        let pass_view = self.pass_view;
        let effect_facts = self.effect_facts;
        let mir_facts = self.mir_facts;
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
        let materialized = pass_view.materialized();
        let callable_ids = planned_lir_callable_ids(&pass_view, effect_facts);
        let callable_refs = planned_lir_callable_refs(&callable_ids, mir_facts);
        let concrete_ops = concrete_ops_by_fqn(effect_facts);
        let mut stable_instance_keys = materialized.stable_instance_keys().clone();
        let mut dump_body_labels =
            HashMap::<LateLoweredBodyVersionKey, crate::mir::BodyLabels>::new();

        let mut continuation_objects = Vec::with_capacity(effect_facts.callable_facts().len());
        let mut callables = Vec::with_capacity(effect_facts.callable_facts().len());

        for family in pass_view.instances() {
            let root_fqn = family.root_fqn().to_string();
            let lift = LirLiftContext::new(&root_fqn, &callable_ids, &callable_refs, &concrete_ops);
            let source_kind = callable_source_kind(materialized, &root_fqn);
            let Some(callable_facts) = effect_facts.callable_facts().get(family.key()) else {
                if family.root_body().is_some() {
                    return Err(EffectLoweringError::MissingCallableFacts {
                        root_fqn: root_fqn.clone(),
                    });
                }
                continue;
            };
            let stable_instance_key = materialized
                .authoritative_stable_instance_key(family.key())
                .ok_or_else(|| EffectLoweringError::MissingStableInstanceKey {
                    root_fqn: root_fqn.clone(),
                })?;
            stable_instance_keys.insert(family.key().clone(), stable_instance_key.clone());
            let body_version_key = LateLoweredBodyVersionKey::new(
                family.key().clone(),
                callable_facts.declared_row().clone(),
                callable_facts.impl_plan(),
                callable_facts.needs_reentry(),
            );
            let pass_source_body = family
                .summary()
                .body_known
                .then(|| family.root_body())
                .flatten();
            let materialized_signature = find_materialized_fun(pass_view.materialized(), &root_fqn);
            let root_source_body = pass_source_body;
            if let Some(body) = root_source_body.and_then(|fun| fun.body.as_ref()) {
                validate_mir_body_for_lir_lift(&root_fqn, body)?;
                dump_body_labels.insert(
                    body_version_key.clone(),
                    build_body_labels_for_dump(&root_fqn, body, types),
                );
            }

            if matches!(callable_facts.call_abi_kind(), CallableAbiKind::Plain) {
                let source_fun = root_source_body;
                let fun = source_fun.or(materialized_signature).ok_or_else(|| {
                    EffectLoweringError::MissingPlainCallableSignature {
                        root_fqn: root_fqn.clone(),
                    }
                })?;
                let declaration_only_signature;
                let plain_fun = if source_fun.is_some() {
                    fun
                } else {
                    let mut signature = fun.clone();
                    signature.body = None;
                    declaration_only_signature = signature;
                    &declaration_only_signature
                };
                let plain = build_plain_callable_abi(PlainCallableBuildInputs {
                    root_fqn: &root_fqn,
                    body_version_key: &body_version_key,
                    fun: plain_fun,
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
                    lift: &lift,
                })?;
                let executable_body = plain_fun.body.as_ref().map(|body| {
                    if let Some(control) = plain.callable.local_effect_control() {
                        lift.lift_control_body(
                            plain_fun,
                            body,
                            super::instruction::LirExecutableBodyFlavor::PlainLocalEffectControl,
                            control.state_graph(),
                        )
                    } else {
                        lift.lift_plain_body(
                            plain_fun,
                            body,
                            super::instruction::LirExecutableBodyFlavor::Plain,
                        )
                    }
                });
                if let Some(object) = plain.continuation_object {
                    continuation_objects.push(object);
                }
                let mut callable = LateLoweredCallable::new_plain(
                    root_fqn,
                    stable_instance_key,
                    body_version_key,
                    callable_facts.resolved_outward_cases().tags().to_vec(),
                    plain.callable,
                )
                .with_source_kind(source_kind)
                .with_source_callable(plain_fun);
                if let Some(executable_body) = executable_body {
                    callable = callable.with_executable_body(executable_body);
                }
                callables.push(callable);
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
                match root_source_body.and_then(|fun| fun.body.as_ref()) {
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
                            &lift,
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
            let boundary_map = match root_source_body.and_then(|fun| fun.body.as_ref()) {
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
                match root_source_body.and_then(|fun| fun.body.as_ref()) {
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
            let mut callable = LateLoweredCallable::new(
                family.root_fqn().to_string(),
                stable_instance_key,
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
            .with_source_kind(source_kind)
            .with_source_statement_classifications(source_statement_classifications);
            if let Some(fun) = root_source_body.or(materialized_signature) {
                callable = callable.with_source_callable(fun);
                if let Some(body) = fun.body.as_ref() {
                    let executable_body = lift.lift_control_body(
                        fun,
                        body,
                        super::instruction::LirExecutableBodyFlavor::EffectStep,
                        callable.state_graph(),
                    );
                    callable = callable.with_executable_body(executable_body);
                }
            }
            callables.push(callable);
        }

        let backend_contracts = materialized.backend_contracts();
        let class_init_payloads = backend_contracts.class_init_payloads().collect::<Vec<_>>();
        let class_ctor_init_bodies = build_class_ctor_init_bodies(
            class_init_payloads.iter(),
            &backend_contracts.ctor_call_sites,
            types,
        );
        let source_class_ctor_calls = build_source_class_ctor_calls(
            &class_init_payloads,
            &class_ctor_init_bodies,
            &backend_contracts.ctor_call_sites,
            &backend_contracts.top_level_immutable_values,
            &backend_contracts.top_level_vars,
            &backend_contracts.object_inits,
            types,
        );
        let program =
            LateLoweredProgram::new(step_types, resume_packings, continuation_objects, callables)
                .with_class_ctor_init_bodies(class_ctor_init_bodies)
                .with_source_class_ctor_calls(source_class_ctor_calls)
                .with_stable_instance_keys(stable_instance_keys);
        let dump_type_texts = collect_program_dump_type_texts(&program, types);
        Ok(program.with_dump_metadata(dump_type_texts, dump_body_labels))
    }
}

fn find_materialized_fun<'a>(materialized: &'a MaterializedMir, fqn: &str) -> Option<&'a FunDecl> {
    materialized.file.items.iter().find_map(|item| match item {
        Item::Fun(fun) if fun.fqn == fqn => Some(fun),
        _ => None,
    })
}

fn validate_mir_body_for_lir_lift(root_fqn: &str, body: &Body) -> Result<(), EffectLoweringError> {
    validate_body_for_lir_lift(root_fqn, body).map_err(|error| {
        EffectLoweringError::InvalidMirForLirLift {
            root_fqn: root_fqn.to_string(),
            detail: error.to_string(),
        }
    })
}

fn planned_lir_callable_ids(
    pass_view: &MaterializedMirPassView<'_>,
    effect_facts: &MaterializedEffectFacts,
) -> HashMap<String, LirCallableId> {
    let mut ids = HashMap::new();
    let mut next = 0u32;
    for family in pass_view.instances() {
        if effect_facts.callable_facts().get(family.key()).is_none() {
            continue;
        }
        ids.insert(family.root_fqn().to_string(), LirCallableId::from_raw(next));
        next += 1;
    }
    ids
}

fn planned_lir_callable_refs(
    callable_ids: &HashMap<String, LirCallableId>,
    mir_facts: &MirFacts,
) -> HashMap<String, LirCallableRef> {
    let mut refs = callable_ids
        .iter()
        .map(|(root_fqn, id)| (root_fqn.clone(), LirCallableRef::Local(*id)))
        .collect::<HashMap<_, _>>();
    for signature in &mir_facts.backend.source_signatures {
        if refs.contains_key(&signature.fqn) {
            continue;
        }
        if let Some(target_key) = &signature.target_callable_key {
            refs.insert(
                signature.fqn.clone(),
                LirCallableRef::ExternalHash(LirCallableHash::from_stable_key(target_key)),
            );
        }
    }
    refs
}

fn concrete_ops_by_fqn(effect_facts: &MaterializedEffectFacts) -> HashMap<String, ConcreteOpKey> {
    let mut out = HashMap::new();
    for schema in effect_facts.step_schemas().values() {
        for case in schema.cases() {
            out.insert(
                case.concrete_op_key().instance_key().template.fqn.clone(),
                case.concrete_op_key().clone(),
            );
        }
    }
    out
}

fn callable_source_kind(
    materialized: &MaterializedMir,
    root_fqn: &str,
) -> scoopc_lir_facts::LirCallableSourceKind {
    let base_fqn = callable_source_base_fqn(root_fqn);
    if base_fqn.contains(".$lambda") || callable_owner_is_nominal_or_object(materialized, base_fqn)
    {
        scoopc_lir_facts::LirCallableSourceKind::MemberOrSynthetic
    } else {
        scoopc_lir_facts::LirCallableSourceKind::TopLevel
    }
}

fn callable_source_base_fqn(root_fqn: &str) -> &str {
    let base = root_fqn
        .rsplit_once("::<")
        .map(|(base, _)| base)
        .unwrap_or(root_fqn);
    base.split_once("$overload$")
        .map(|(base, _)| base)
        .unwrap_or(base)
}

fn callable_owner_is_nominal_or_object(materialized: &MaterializedMir, root_fqn: &str) -> bool {
    let Some((owner_fqn, _name)) = root_fqn.rsplit_once('.') else {
        return false;
    };
    materialized.file.items.iter().any(|item| match item {
        Item::Metadata(crate::mir::MetadataRoot::Nominal(metadata)) => metadata.fqn == owner_fqn,
        Item::Metadata(crate::mir::MetadataRoot::Object(metadata)) => metadata.fqn == owner_fqn,
        _ => false,
    })
}

pub fn build_class_ctor_init_bodies<'a>(
    classes: impl Iterator<Item = &'a source::MonoClassInit> + Clone,
    ctor_call_sites: &crate::mir::source_payload::CtorCallSiteIndex,
    types: &TypeStore,
) -> Vec<LateLoweredClassCtorInitBody> {
    let mut bodies = Vec::new();
    let class_index = classes.clone().collect::<Vec<_>>();
    for class in &class_index {
        if class.ctors.is_empty() {
            bodies.push(build_class_ctor_init_body(
                &class_index,
                class,
                None,
                ctor_call_sites,
                types,
            ));
            continue;
        }
        for ctor in &class.ctors {
            bodies.push(build_class_ctor_init_body(
                &class_index,
                class,
                Some(ctor),
                ctor_call_sites,
                types,
            ));
        }
    }
    bodies
}

fn build_source_class_ctor_calls(
    class_init_payloads: &[source::MonoClassInit],
    class_ctor_init_bodies: &[LateLoweredClassCtorInitBody],
    ctor_call_sites: &crate::mir::source_payload::CtorCallSiteIndex,
    top_level_immutable_values: &crate::mir::source_payload::TopLevelImmutableValueIndex,
    top_level_vars: &crate::mir::source_payload::TopLevelVarIndex,
    object_inits: &crate::mir::source_payload::ObjectInitIndex,
    types: &TypeStore,
) -> Vec<LateLoweredClassCtorSourceCallContract> {
    let class_index = class_init_payloads.iter().collect::<Vec<_>>();
    let mut out = class_ctor_init_bodies
        .iter()
        .flat_map(|body| body.source_ctor_calls().iter().cloned())
        .collect::<Vec<_>>();

    for value in top_level_immutable_values.values() {
        if let Some(init) = &value.init {
            collect_class_ctor_source_call_contracts_from_expr(
                value.source_path.as_path(),
                init,
                ctor_call_sites,
                types,
                &class_index,
                Some(value.ty),
                &mut out,
            );
        }
    }
    for value in top_level_vars.values() {
        if let Some(init) = &value.init {
            collect_class_ctor_source_call_contracts_from_expr(
                value.source_path.as_path(),
                init,
                ctor_call_sites,
                types,
                &class_index,
                Some(value.ty),
                &mut out,
            );
        }
    }
    for object in object_inits.values() {
        for step in &object.steps {
            match step {
                crate::mir::source_payload::ObjectInitStep::PropertyInit { name, init } => {
                    collect_class_ctor_source_call_contracts_from_expr(
                        object.source_path.as_path(),
                        init,
                        ctor_call_sites,
                        types,
                        &class_index,
                        object.properties.get(name).map(|property| property.ty),
                        &mut out,
                    );
                }
                crate::mir::source_payload::ObjectInitStep::InitBlock { block } => {
                    collect_class_ctor_source_call_contracts_from_block(
                        object.source_path.as_path(),
                        block,
                        ctor_call_sites,
                        types,
                        &class_index,
                        &mut out,
                    );
                }
            }
        }
    }

    out.sort_by(|lhs, rhs| {
        lhs.source_path()
            .cmp(rhs.source_path())
            .then(lhs.call_span().start.cmp(&rhs.call_span().start))
            .then(lhs.call_span().end.cmp(&rhs.call_span().end))
    });
    out.dedup_by(|lhs, rhs| {
        lhs.source_path() == rhs.source_path() && lhs.call_span() == rhs.call_span()
    });
    out
}

fn build_class_ctor_init_body(
    class_index: &[&source::MonoClassInit],
    class: &source::MonoClassInit,
    ctor: Option<&source::ClassCtor<crate::ty::MonoTypeId>>,
    ctor_call_sites: &crate::mir::source_payload::CtorCallSiteIndex,
    types: &TypeStore,
) -> LateLoweredClassCtorInitBody {
    let ctor_span = ctor.map(|ctor| ctor.span);
    let key = class_ctor_init_key(&class.fqn, ctor_span);
    let ctor_kind = ctor
        .map(|ctor| lir_class_ctor_kind(ctor.kind))
        .unwrap_or(scoopc_lir_facts::LirClassCtorKind::Primary);
    let params = ctor
        .map(|ctor| {
            ctor.params
                .iter()
                .map(LateLoweredClassCtorParam::new)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let delegation = ctor.and_then(|ctor| {
        ctor.delegation
            .as_ref()
            .map(|delegation| build_class_ctor_delegation(class, ctor.span, delegation))
    });
    let implicit_super = if delegation.is_none() {
        build_implicit_super_call(class)
    } else {
        None
    };
    let mut steps = Vec::new();
    if delegation.as_ref().is_none_or(|delegation| {
        delegation.kind() != scoopc_lir_facts::LirClassCtorDelegationKind::This
    }) {
        for (param_index, param) in params.iter().enumerate() {
            if let Some(field_fqn) = param.property_field_fqn() {
                steps.push(LateLoweredClassCtorInitStep::PropertyParamAssignment {
                    param_index,
                    field_fqn: field_fqn.to_string(),
                    span: param.decl_span(),
                });
            }
        }
        for step in &class.steps {
            match step {
                source::ClassInitStep::PropertyInit { field_fqn, init } => {
                    steps.push(LateLoweredClassCtorInitStep::PropertyInitializer {
                        field_fqn: field_fqn.clone(),
                        init: init.clone(),
                    });
                }
                source::ClassInitStep::InitBlock { block } => {
                    steps.push(LateLoweredClassCtorInitStep::InitBlock {
                        block: block.clone(),
                    });
                }
            }
        }
    }
    if let Some(body) = ctor.and_then(|ctor| ctor.body.as_ref()) {
        steps.push(LateLoweredClassCtorInitStep::SecondaryBody {
            block: body.clone(),
        });
    }
    let source_ctor_calls = collect_class_ctor_source_call_contracts(
        class_index,
        class,
        class.source_path.as_path(),
        ctor,
        implicit_super.as_ref(),
        delegation.as_ref(),
        &steps,
        ctor_call_sites,
        types,
    );
    LateLoweredClassCtorInitBody::new(
        key,
        class.fqn.clone(),
        class.source_path.clone(),
        class.this_id,
        ctor_kind,
        ctor_span,
        params,
        implicit_super,
        delegation,
        steps,
        source_ctor_calls,
    )
}

#[allow(clippy::too_many_arguments)]
fn collect_class_ctor_source_call_contracts(
    class_index: &[&source::MonoClassInit],
    class: &source::MonoClassInit,
    source_path: &Path,
    ctor: Option<&source::ClassCtor<crate::ty::MonoTypeId>>,
    implicit_super: Option<&LateLoweredClassCtorSuperCall>,
    delegation: Option<&LateLoweredClassCtorDelegation>,
    steps: &[LateLoweredClassCtorInitStep],
    ctor_call_sites: &crate::mir::source_payload::CtorCallSiteIndex,
    types: &TypeStore,
) -> Vec<LateLoweredClassCtorSourceCallContract> {
    let mut out = Vec::new();
    if let Some(ctor) = ctor {
        for param in &ctor.params {
            if let Some(default_value) = &param.default_value {
                collect_class_ctor_source_call_contracts_from_expr(
                    source_path,
                    default_value,
                    ctor_call_sites,
                    types,
                    class_index,
                    Some(param.ty.inner()),
                    &mut out,
                );
            }
        }
    }
    if let Some(super_call) = implicit_super {
        for arg in super_call.args() {
            collect_class_ctor_source_call_contracts_from_arg(
                source_path,
                arg,
                ctor_call_sites,
                types,
                class_index,
                None,
                &mut out,
            );
        }
    }
    if let Some(delegation) = delegation {
        for arg in delegation.args() {
            collect_class_ctor_source_call_contracts_from_arg(
                source_path,
                arg,
                ctor_call_sites,
                types,
                class_index,
                None,
                &mut out,
            );
        }
    }
    for step in steps {
        match step {
            LateLoweredClassCtorInitStep::PropertyParamAssignment { .. } => {}
            LateLoweredClassCtorInitStep::PropertyInitializer { field_fqn, init } => {
                collect_class_ctor_source_call_contracts_from_expr(
                    source_path,
                    init,
                    ctor_call_sites,
                    types,
                    class_index,
                    class
                        .fields
                        .iter()
                        .find(|field| field.fqn == *field_fqn)
                        .map(|field| field.ty.inner()),
                    &mut out,
                );
            }
            LateLoweredClassCtorInitStep::InitBlock { block }
            | LateLoweredClassCtorInitStep::SecondaryBody { block } => {
                collect_class_ctor_source_call_contracts_from_block(
                    source_path,
                    block,
                    ctor_call_sites,
                    types,
                    class_index,
                    &mut out,
                );
            }
        }
    }
    out
}

fn collect_class_ctor_source_call_contracts_from_block(
    source_path: &Path,
    block: &source::Block,
    ctor_call_sites: &crate::mir::source_payload::CtorCallSiteIndex,
    types: &TypeStore,
    class_index: &[&source::MonoClassInit],
    out: &mut Vec<LateLoweredClassCtorSourceCallContract>,
) {
    for stmt in &block.stmts {
        collect_class_ctor_source_call_contracts_from_stmt(
            source_path,
            stmt,
            ctor_call_sites,
            types,
            class_index,
            out,
        );
    }
}

fn collect_class_ctor_source_call_contracts_from_stmt(
    source_path: &Path,
    stmt: &crate::mir::source_payload::Stmt,
    ctor_call_sites: &crate::mir::source_payload::CtorCallSiteIndex,
    types: &TypeStore,
    class_index: &[&source::MonoClassInit],
    out: &mut Vec<LateLoweredClassCtorSourceCallContract>,
) {
    use crate::mir::source_payload::StmtKind;

    match &stmt.kind {
        StmtKind::Empty
        | StmtKind::Break { .. }
        | StmtKind::Continue { .. }
        | StmtKind::Todo(_) => {}
        StmtKind::Expr(expr) => {
            collect_class_ctor_source_call_contracts_from_expr(
                source_path,
                expr,
                ctor_call_sites,
                types,
                class_index,
                None,
                out,
            );
        }
        StmtKind::Val(decl) => {
            if let Some(init) = &decl.init {
                collect_class_ctor_source_call_contracts_from_expr(
                    source_path,
                    init,
                    ctor_call_sites,
                    types,
                    class_index,
                    Some(decl.ty),
                    out,
                );
            }
        }
        StmtKind::Assign { lhs, rhs, .. } => {
            collect_class_ctor_source_call_contracts_from_expr(
                source_path,
                lhs,
                ctor_call_sites,
                types,
                class_index,
                None,
                out,
            );
            collect_class_ctor_source_call_contracts_from_expr(
                source_path,
                rhs,
                ctor_call_sites,
                types,
                class_index,
                None,
                out,
            );
        }
        StmtKind::While { cond, body } => {
            collect_class_ctor_source_call_contracts_from_expr(
                source_path,
                cond,
                ctor_call_sites,
                types,
                class_index,
                None,
                out,
            );
            collect_class_ctor_source_call_contracts_from_block(
                source_path,
                body,
                ctor_call_sites,
                types,
                class_index,
                out,
            );
        }
        StmtKind::Return { value } => {
            if let Some(value) = value {
                collect_class_ctor_source_call_contracts_from_expr(
                    source_path,
                    value,
                    ctor_call_sites,
                    types,
                    class_index,
                    None,
                    out,
                );
            }
        }
    }
}

fn collect_class_ctor_source_call_contracts_from_arg(
    source_path: &Path,
    arg: &source::CallArg,
    ctor_call_sites: &crate::mir::source_payload::CtorCallSiteIndex,
    types: &TypeStore,
    class_index: &[&source::MonoClassInit],
    expected_ty: Option<TypeId>,
    out: &mut Vec<LateLoweredClassCtorSourceCallContract>,
) {
    match arg {
        crate::mir::source_payload::CallArg::Positional(expr)
        | crate::mir::source_payload::CallArg::Named { value: expr, .. } => {
            collect_class_ctor_source_call_contracts_from_expr(
                source_path,
                expr,
                ctor_call_sites,
                types,
                class_index,
                expected_ty,
                out,
            );
        }
    }
}

struct ClassCtorSourceSelection {
    ctor_span: Option<crate::span::Span>,
    arg_mapping: Vec<Option<usize>>,
    arg_expected: Vec<Option<TypeId>>,
}

fn ctor_source_contract_selection(
    class_index: &[&source::MonoClassInit],
    types: &TypeStore,
    result_ty: TypeId,
    args: &[source::CallArg],
    published: Option<&crate::mir::source_payload::CtorCallInfo>,
) -> Option<ClassCtorSourceSelection> {
    if let Some(call) = published {
        let arg_expected =
            class_ctor_call_arg_expected_tys(class_index, types, result_ty, call, args.len());
        return Some(ClassCtorSourceSelection {
            ctor_span: call.ctor_span,
            arg_mapping: call.arg_mapping.clone(),
            arg_expected,
        });
    }
    let target_class_fqn = types.display(result_ty).to_string();
    let class = class_index
        .iter()
        .copied()
        .find(|class| class.fqn == target_class_fqn)?;
    if class.ctors.is_empty() {
        return args.is_empty().then_some(ClassCtorSourceSelection {
            ctor_span: None,
            arg_mapping: Vec::new(),
            arg_expected: Vec::new(),
        });
    }
    let mut matches = class
        .ctors
        .iter()
        .filter_map(|ctor| derive_ctor_arg_mapping(ctor, args).map(|mapping| (ctor, mapping)))
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return None;
    }
    let (ctor, arg_mapping) = matches.remove(0);
    let mut arg_expected = vec![None; args.len()];
    for (param_idx, arg_idx) in arg_mapping.iter().copied().enumerate() {
        let Some(arg_idx) = arg_idx else {
            continue;
        };
        if let (Some(slot), Some(param)) =
            (arg_expected.get_mut(arg_idx), ctor.params.get(param_idx))
        {
            *slot = Some(param.ty.inner());
        }
    }
    Some(ClassCtorSourceSelection {
        ctor_span: Some(ctor.span),
        arg_mapping,
        arg_expected,
    })
}

fn derive_ctor_arg_mapping(
    ctor: &source::ClassCtor<crate::ty::MonoTypeId>,
    args: &[source::CallArg],
) -> Option<Vec<Option<usize>>> {
    let mut mapping = vec![None; ctor.params.len()];
    let mut next_positional = 0usize;
    for (arg_idx, arg) in args.iter().enumerate() {
        let param_idx = match arg {
            crate::mir::source_payload::CallArg::Positional(_) => {
                while mapping.get(next_positional).is_some_and(Option::is_some) {
                    next_positional += 1;
                }
                let idx = next_positional;
                next_positional += 1;
                idx
            }
            crate::mir::source_payload::CallArg::Named { name, .. } => {
                ctor.params.iter().position(|param| param.name == *name)?
            }
        };
        let slot = mapping.get_mut(param_idx)?;
        if slot.is_some() {
            return None;
        }
        *slot = Some(arg_idx);
    }
    if mapping
        .iter()
        .enumerate()
        .any(|(idx, arg)| arg.is_none() && !ctor.params[idx].has_default)
    {
        return None;
    }
    Some(mapping)
}

fn class_ctor_call_arg_expected_tys(
    class_index: &[&source::MonoClassInit],
    types: &TypeStore,
    result_ty: TypeId,
    call: &crate::mir::source_payload::CtorCallInfo,
    arg_count: usize,
) -> Vec<Option<TypeId>> {
    let mut expected = vec![None; arg_count];
    let target_class_fqn = types.display(result_ty).to_string();
    let Some(class) = class_index
        .iter()
        .copied()
        .find(|class| class.fqn == target_class_fqn)
    else {
        return expected;
    };
    let selected_ctor = call
        .ctor_span
        .and_then(|span| class.ctors.iter().find(|ctor| ctor.span == span))
        .or_else(|| {
            if call.ctor_span.is_none() && class.ctors.len() == 1 {
                class.ctors.first()
            } else {
                None
            }
        });
    let Some(ctor) = selected_ctor else {
        return expected;
    };
    for (param_idx, arg_idx) in call.arg_mapping.iter().copied().enumerate() {
        let Some(arg_idx) = arg_idx else {
            continue;
        };
        if let (Some(slot), Some(param)) = (expected.get_mut(arg_idx), ctor.params.get(param_idx)) {
            *slot = Some(param.ty.inner());
        }
    }
    expected
}

fn collect_class_ctor_source_call_contracts_from_expr(
    source_path: &Path,
    expr: &source::Expr,
    ctor_call_sites: &crate::mir::source_payload::CtorCallSiteIndex,
    types: &TypeStore,
    class_index: &[&source::MonoClassInit],
    expected_ty: Option<TypeId>,
    out: &mut Vec<LateLoweredClassCtorSourceCallContract>,
) {
    use crate::mir::source_payload::{ExprKind, InterpolatedStringPart};

    if let ExprKind::Call { args, .. } = &expr.kind {
        let site = crate::mir::source_payload::CallSite::new(source_path.to_path_buf(), expr.span);
        let result_ty = expected_ty.unwrap_or(expr.ty);
        if let Some(selection) = ctor_source_contract_selection(
            class_index,
            types,
            result_ty,
            args,
            ctor_call_sites.get(&site),
        ) && !out.iter().any(|contract| contract.call_span() == expr.span)
        {
            let target_class_fqn = types.display(result_ty).to_string();
            out.push(LateLoweredClassCtorSourceCallContract::new(
                source_path.to_path_buf(),
                expr.span,
                target_class_fqn.clone(),
                class_ctor_init_key(&target_class_fqn, selection.ctor_span),
                result_ty,
                selection.arg_mapping,
            ));
        }
    }

    match &expr.kind {
        ExprKind::Missing
        | ExprKind::Literal(_)
        | ExprKind::VarRef(_)
        | ExprKind::UnresolvedIdent { .. }
        | ExprKind::ClassLiteral(_)
        | ExprKind::Todo(_) => {}
        ExprKind::StructLit { fields, .. } => {
            for field in fields {
                collect_class_ctor_source_call_contracts_from_expr(
                    source_path,
                    &field.value,
                    ctor_call_sites,
                    types,
                    class_index,
                    None,
                    out,
                );
            }
        }
        ExprKind::TupleLit { elements } => {
            for element in elements {
                collect_class_ctor_source_call_contracts_from_expr(
                    source_path,
                    element,
                    ctor_call_sites,
                    types,
                    class_index,
                    None,
                    out,
                );
            }
        }
        ExprKind::InterpolatedString { parts, .. } => {
            for part in parts {
                if let InterpolatedStringPart::Expr { expr } = part {
                    collect_class_ctor_source_call_contracts_from_expr(
                        source_path,
                        expr,
                        ctor_call_sites,
                        types,
                        class_index,
                        None,
                        out,
                    );
                }
            }
        }
        ExprKind::Unary { expr, .. }
        | ExprKind::TypeCheck { expr, .. }
        | ExprKind::Cast { expr, .. } => {
            collect_class_ctor_source_call_contracts_from_expr(
                source_path,
                expr,
                ctor_call_sites,
                types,
                class_index,
                None,
                out,
            );
        }
        ExprKind::Binary { lhs, rhs, .. } => {
            collect_class_ctor_source_call_contracts_from_expr(
                source_path,
                lhs,
                ctor_call_sites,
                types,
                class_index,
                None,
                out,
            );
            collect_class_ctor_source_call_contracts_from_expr(
                source_path,
                rhs,
                ctor_call_sites,
                types,
                class_index,
                None,
                out,
            );
        }
        ExprKind::Block(block) => {
            collect_class_ctor_source_call_contracts_from_block(
                source_path,
                block,
                ctor_call_sites,
                types,
                class_index,
                out,
            );
        }
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_class_ctor_source_call_contracts_from_expr(
                source_path,
                cond,
                ctor_call_sites,
                types,
                class_index,
                None,
                out,
            );
            collect_class_ctor_source_call_contracts_from_expr(
                source_path,
                then_branch,
                ctor_call_sites,
                types,
                class_index,
                None,
                out,
            );
            if let Some(else_branch) = else_branch {
                collect_class_ctor_source_call_contracts_from_expr(
                    source_path,
                    else_branch,
                    ctor_call_sites,
                    types,
                    class_index,
                    None,
                    out,
                );
            }
        }
        ExprKind::When { subject, arms } => {
            collect_class_ctor_source_call_contracts_from_expr(
                source_path,
                subject,
                ctor_call_sites,
                types,
                class_index,
                None,
                out,
            );
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_class_ctor_source_call_contracts_from_expr(
                        source_path,
                        guard,
                        ctor_call_sites,
                        types,
                        class_index,
                        None,
                        out,
                    );
                }
                collect_class_ctor_source_call_contracts_from_expr(
                    source_path,
                    &arm.body,
                    ctor_call_sites,
                    types,
                    class_index,
                    None,
                    out,
                );
            }
        }
        ExprKind::MemberAccess { receiver, .. } => {
            collect_class_ctor_source_call_contracts_from_expr(
                source_path,
                receiver,
                ctor_call_sites,
                types,
                class_index,
                None,
                out,
            );
        }
        ExprKind::Call { callee, args } => {
            collect_class_ctor_source_call_contracts_from_expr(
                source_path,
                callee,
                ctor_call_sites,
                types,
                class_index,
                None,
                out,
            );
            let site =
                crate::mir::source_payload::CallSite::new(source_path.to_path_buf(), expr.span);
            let arg_expected = ctor_source_contract_selection(
                class_index,
                types,
                expected_ty.unwrap_or(expr.ty),
                args,
                ctor_call_sites.get(&site),
            )
            .map(|selection| selection.arg_expected)
            .unwrap_or_else(|| vec![None; args.len()]);
            for (arg_idx, arg) in args.iter().enumerate() {
                collect_class_ctor_source_call_contracts_from_arg(
                    source_path,
                    arg,
                    ctor_call_sites,
                    types,
                    class_index,
                    arg_expected.get(arg_idx).copied().flatten(),
                    out,
                );
            }
        }
        ExprKind::Perform { args, .. } => {
            for arg in args {
                collect_class_ctor_source_call_contracts_from_arg(
                    source_path,
                    arg,
                    ctor_call_sites,
                    types,
                    class_index,
                    None,
                    out,
                );
            }
        }
        ExprKind::Handle(handle) => {
            collect_class_ctor_source_call_contracts_from_block(
                source_path,
                &handle.body,
                ctor_call_sites,
                types,
                class_index,
                out,
            );
            for arm in &handle.arms {
                collect_class_ctor_source_call_contracts_from_expr(
                    source_path,
                    &arm.body,
                    ctor_call_sites,
                    types,
                    class_index,
                    None,
                    out,
                );
            }
            if let Some(finally) = &handle.finally {
                collect_class_ctor_source_call_contracts_from_block(
                    source_path,
                    finally,
                    ctor_call_sites,
                    types,
                    class_index,
                    out,
                );
            }
        }
        ExprKind::Closure(closure) => {
            collect_class_ctor_source_call_contracts_from_expr(
                source_path,
                &closure.body,
                ctor_call_sites,
                types,
                class_index,
                None,
                out,
            );
        }
    }
}

fn build_implicit_super_call(
    class: &source::MonoClassInit,
) -> Option<LateLoweredClassCtorSuperCall> {
    let super_fqn = class.super_class_fqn.as_ref()?;
    let target_span = class
        .super_ctor_call
        .as_ref()
        .and_then(|call| call.ctor_span);
    Some(LateLoweredClassCtorSuperCall::new(
        class_ctor_init_key(super_fqn, target_span),
        super_fqn.clone(),
        class.super_ctor_call.clone(),
        class.super_ctor_args.clone(),
        class.super_ctor_args_span,
    ))
}

fn build_class_ctor_delegation(
    class: &source::MonoClassInit,
    _current_ctor_span: crate::span::Span,
    delegation: &source::ClassCtorDelegation,
) -> LateLoweredClassCtorDelegation {
    let kind = match delegation.kind {
        source::ClassCtorDelegationKind::This => scoopc_lir_facts::LirClassCtorDelegationKind::This,
        source::ClassCtorDelegationKind::Super => {
            scoopc_lir_facts::LirClassCtorDelegationKind::Super
        }
    };
    let class_fqn = delegation
        .call
        .as_ref()
        .map(|call| call.class_fqn.clone())
        .or_else(|| match delegation.kind {
            source::ClassCtorDelegationKind::This => Some(class.fqn.clone()),
            source::ClassCtorDelegationKind::Super => class.super_class_fqn.clone(),
        })
        .unwrap_or_else(|| class.fqn.clone());
    let target_span = delegation.call.as_ref().and_then(|call| call.ctor_span);
    LateLoweredClassCtorDelegation::new(
        kind,
        class_ctor_init_key(&class_fqn, target_span),
        class_fqn,
        delegation.call.clone(),
        delegation.args.clone(),
        delegation.span,
    )
}

fn class_ctor_init_key(
    class_fqn: &str,
    ctor_span: Option<crate::span::Span>,
) -> scoopc_lir_facts::LirClassCtorInitKey {
    scoopc_lir_facts::LirClassCtorInitKey::for_ctor(
        class_fqn,
        ctor_span.map(|span| (span.start, span.end)),
    )
}

fn lir_class_ctor_kind(kind: source::ClassCtorKind) -> scoopc_lir_facts::LirClassCtorKind {
    match kind {
        source::ClassCtorKind::Primary => scoopc_lir_facts::LirClassCtorKind::Primary,
        source::ClassCtorKind::Secondary => scoopc_lir_facts::LirClassCtorKind::Secondary,
    }
}

fn nominal_direct_supertypes_from_mir_facts(mir_facts: &MirFacts) -> NominalDirectSupertypeIndex {
    mir_facts
        .metadata
        .nominal_direct_supertypes
        .iter()
        .map(|fact| (fact.fqn.clone(), fact.direct_supertypes.clone()))
        .collect()
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
    lift: &'a LirLiftContext<'a>,
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
        lift,
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
                lift,
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
    lift: &'a LirLiftContext<'a>,
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
        lift,
        return_ty,
    } = inputs;
    let step_schema_id =
        require_plain_local_effect_control_step_schema(root_fqn, body_facts, effect_facts)?;
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
    let segmentation = build_callable_segmentation(
        root_fqn,
        types,
        body,
        body_facts,
        step_schema.complete_ty(),
        lift,
    )?;
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

fn require_plain_local_effect_control_step_schema(
    root_fqn: &str,
    body_facts: &crate::effect_facts::BodyEffectFacts,
    _effect_facts: &MaterializedEffectFacts,
) -> Result<StepSchemaId, EffectLoweringError> {
    if let Some(step_schema) = body_facts.local_control_step_schema() {
        return Ok(step_schema);
    }

    Err(
        EffectLoweringError::InvalidPlainLocalEffectControlContract {
            root_fqn: root_fqn.to_string(),
            detail: "plain body 含本地 effect/control，但 P4 未发布 local_control_step_schema"
                .to_string(),
        },
    )
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
                value:
                    Rvalue::Call {
                        site_id,
                        kind,
                        args,
                        ..
                    },
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
                call_site_materialized_metadata(body, kind, args.len()),
                call_facts.clone(),
            ));
        }
    }
    Ok(call_sites)
}

fn call_site_materialized_metadata(
    body: &Body,
    kind: &crate::mir::CallKind,
    arg_count: usize,
) -> LateLoweredCallSiteMaterializedMetadata {
    LateLoweredCallSiteMaterializedMetadata::new(
        call_site_materialized_kind(kind),
        arg_count,
        call_carrier_source_ty(body, kind),
    )
}

fn call_site_materialized_kind(kind: &crate::mir::CallKind) -> LateLoweredCallSiteMaterializedKind {
    match kind {
        crate::mir::CallKind::Direct { .. } => LateLoweredCallSiteMaterializedKind::Direct,
        crate::mir::CallKind::Closure { .. } => LateLoweredCallSiteMaterializedKind::Closure,
        crate::mir::CallKind::FunValue { .. } => LateLoweredCallSiteMaterializedKind::FunValue,
        crate::mir::CallKind::FunPtr { .. } => LateLoweredCallSiteMaterializedKind::FunPtr,
        crate::mir::CallKind::Virtual { dispatch, .. } => {
            LateLoweredCallSiteMaterializedKind::Virtual {
                owner_fqn: dispatch.owner_fqn.clone(),
                member_name: dispatch.member_name.clone(),
                member_fqn: dispatch.member_fqn.clone(),
                receiver_ty: dispatch.receiver_ty,
            }
        }
        crate::mir::CallKind::Interface { dispatch, .. } => {
            LateLoweredCallSiteMaterializedKind::Interface {
                owner_fqn: dispatch.owner_fqn.clone(),
                member_name: dispatch.member_name.clone(),
                member_fqn: dispatch.member_fqn.clone(),
                receiver_ty: dispatch.receiver_ty,
            }
        }
        crate::mir::CallKind::Resume { .. } => LateLoweredCallSiteMaterializedKind::Resume,
    }
}

fn call_carrier_source_ty(body: &Body, kind: &crate::mir::CallKind) -> Option<crate::ty::TypeId> {
    match kind {
        crate::mir::CallKind::Closure { callee, .. }
        | crate::mir::CallKind::FunValue { callee }
        | crate::mir::CallKind::FunPtr { callee } => operand_source_ty(body, callee),
        crate::mir::CallKind::Virtual { receiver, dispatch }
        | crate::mir::CallKind::Interface { receiver, dispatch } => {
            operand_source_ty(body, receiver).or(Some(dispatch.receiver_ty))
        }
        crate::mir::CallKind::Resume { continuation, .. } => operand_source_ty(body, continuation),
        crate::mir::CallKind::Direct { .. } => None,
    }
}

fn operand_source_ty(body: &Body, operand: &crate::mir::Operand) -> Option<crate::ty::TypeId> {
    match operand {
        crate::mir::Operand::Local(local) => {
            body.locals.get(local.as_u32() as usize).map(|decl| decl.ty)
        }
        crate::mir::Operand::Const(_) => None,
    }
}

fn collect_program_dump_type_texts(
    program: &LateLoweredProgram,
    types: &TypeStore,
) -> HashMap<crate::ty::TypeId, String> {
    let mut out = HashMap::new();

    for step_type in program.step_types() {
        record_step_type_types(&mut out, types, step_type);
    }
    for interface in program.resume_packings() {
        record_effect_family_key_types(&mut out, types, interface.effect_family());
        for method in interface.methods() {
            record_resume_method_types(&mut out, types, method);
        }
    }
    for object in program.continuation_objects() {
        record_body_version_key_types(&mut out, types, object.owner_version_key());
        record_type_text(&mut out, types, object.continuation_obj_ty());
        for surface_resume in object.surface_resumes() {
            record_surface_resume_types(&mut out, types, surface_resume);
        }
        for method in object.methods() {
            record_continuation_method_types(&mut out, types, method);
        }
    }
    for entry in program.surface_resume_dispatch_inventory() {
        let contract = entry.contract();
        record_type_text(&mut out, types, contract.resume_tuple_ty());
        record_type_text(&mut out, types, contract.answer_ty());
        for projection in entry.wrapper_projections() {
            record_type_text(&mut out, types, projection.complete().owner_answer_ty());
            record_type_text(&mut out, types, projection.complete().wrapper_answer_ty());
            record_surface_resume_wrapper_complete_payload_source_types(
                &mut out,
                types,
                projection.complete().payload_source(),
            );
            for case in projection.outward_cases() {
                record_type_text(&mut out, types, case.owner_payload_tuple_ty());
                record_type_text(&mut out, types, case.wrapper_payload_tuple_ty());
            }
        }
    }
    for callable in program.callables() {
        record_body_version_key_types(&mut out, types, callable.body_version_key());
        if let Some(plain) = callable.plain_abi() {
            record_type_text(&mut out, types, plain.function_ty());
            for &param in plain.param_tys() {
                record_type_text(&mut out, types, param);
            }
            record_type_text(&mut out, types, plain.return_ty());
            for call_site in plain.call_sites() {
                record_call_site_facts_types(&mut out, types, call_site.facts());
            }
        }
        if let Some(effect_step) = callable.effect_step_abi() {
            record_type_text(
                &mut out,
                types,
                effect_step.dynamic_invoke_entry().invoke_args_tuple_ty(),
            );
            record_state_graph_types(&mut out, types, effect_step.state_graph());
            record_frame_schema_types(&mut out, types, effect_step.frame_schema());
            for boundary in effect_step.boundary_map().entries() {
                if let Some(lowering) = boundary.lowering() {
                    record_boundary_lowering_types(&mut out, types, lowering);
                }
            }
        }
        if let Some(local) = callable.plain_local_effect_control() {
            record_state_graph_types(&mut out, types, local.state_graph());
            record_frame_schema_types(&mut out, types, local.frame_schema());
            for boundary in local.boundary_map().entries() {
                if let Some(lowering) = boundary.lowering() {
                    record_boundary_lowering_types(&mut out, types, lowering);
                }
            }
        }
    }

    out
}

fn record_type_text(
    out: &mut HashMap<crate::ty::TypeId, String>,
    types: &TypeStore,
    ty: crate::ty::TypeId,
) {
    out.entry(ty)
        .or_insert_with(|| normalize_display_text(types.display(ty).to_string()));
}

fn normalize_display_text(text: String) -> String {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let Some(workspace_root) = manifest_dir.parent().and_then(Path::parent) else {
        return text;
    };
    let prefix = format!("{}/", workspace_root.display());
    text.replace(&prefix, "")
}

fn record_effect_row_types(
    out: &mut HashMap<crate::ty::TypeId, String>,
    types: &TypeStore,
    row: &crate::ty::EffectRow,
) {
    for &term in &row.terms {
        record_type_text(out, types, term);
    }
}

fn record_instance_key_types(
    out: &mut HashMap<crate::ty::TypeId, String>,
    types: &TypeStore,
    key: &crate::mir::InstanceKey,
) {
    for &ty in &key.type_args {
        record_type_text(out, types, ty);
    }
    for row in &key.eff_args {
        record_effect_row_types(out, types, row);
    }
}

fn record_effect_family_key_types(
    out: &mut HashMap<crate::ty::TypeId, String>,
    types: &TypeStore,
    key: &crate::effect_facts::EffectFamilyKey,
) {
    for &ty in key.type_args() {
        record_type_text(out, types, ty);
    }
}

fn record_concrete_op_key_types(
    out: &mut HashMap<crate::ty::TypeId, String>,
    types: &TypeStore,
    key: &crate::effect_facts::ConcreteOpKey,
) {
    record_instance_key_types(out, types, key.instance_key());
    record_effect_family_key_types(out, types, key.effect_family());
}

fn record_body_version_key_types(
    out: &mut HashMap<crate::ty::TypeId, String>,
    types: &TypeStore,
    key: &LateLoweredBodyVersionKey,
) {
    record_instance_key_types(out, types, key.surface_instance());
    record_effect_row_types(out, types, key.allowed_row());
}

fn record_step_type_types(
    out: &mut HashMap<crate::ty::TypeId, String>,
    types: &TypeStore,
    step_type: &crate::effect_lowered::ir::LateLoweredStepType,
) {
    record_type_text(out, types, step_type.invoke_args_tuple_ty());
    record_type_text(out, types, step_type.complete_ty());
    record_type_text(out, types, step_type.continuation_obj_ty());
    for case in step_type.cases() {
        record_step_case_types(out, types, case);
    }
}

fn record_step_case_types(
    out: &mut HashMap<crate::ty::TypeId, String>,
    types: &TypeStore,
    case: &crate::effect_lowered::ir::LateLoweredStepCase,
) {
    record_concrete_op_key_types(out, types, case.concrete_op_key());
    record_type_text(out, types, case.payload_tuple_ty());
    record_continuation_contract_types(out, types, case.continuation_contract());
}

fn record_resume_method_types(
    out: &mut HashMap<crate::ty::TypeId, String>,
    types: &TypeStore,
    method: &crate::effect_lowered::ir::LateLoweredResumeMethod,
) {
    record_concrete_op_key_types(out, types, method.concrete_op_key());
    record_type_text(out, types, method.resume_tuple_ty());
    record_type_text(out, types, method.answer_ty());
    record_type_text(out, types, method.surface_ty());
}

fn record_surface_resume_types(
    out: &mut HashMap<crate::ty::TypeId, String>,
    types: &TypeStore,
    surface_resume: &crate::effect_lowered::ir::LateLoweredContinuationSurfaceResume,
) {
    record_concrete_op_key_types(out, types, surface_resume.concrete_op_key());
    record_type_text(out, types, surface_resume.resume_tuple_ty());
    record_type_text(out, types, surface_resume.answer_ty());
    record_type_text(out, types, surface_resume.surface_ty());
}

fn record_continuation_method_types(
    out: &mut HashMap<crate::ty::TypeId, String>,
    types: &TypeStore,
    method: &crate::effect_lowered::ir::LateLoweredContinuationMethod,
) {
    record_concrete_op_key_types(out, types, method.concrete_op_key());
    record_type_text(out, types, method.resume_tuple_ty());
    record_type_text(out, types, method.answer_ty());
    record_type_text(out, types, method.surface_ty());
}

fn record_continuation_contract_types(
    out: &mut HashMap<crate::ty::TypeId, String>,
    types: &TypeStore,
    contract: crate::effect_lowered::ir::LateLoweredContinuationContract,
) {
    record_type_text(out, types, contract.resume_tuple_ty());
    record_type_text(out, types, contract.answer_ty());
    record_type_text(out, types, contract.surface_ty());
}

fn record_call_target_types(
    out: &mut HashMap<crate::ty::TypeId, String>,
    types: &TypeStore,
    target: &crate::effect_facts::CallSiteTarget,
) {
    match target {
        crate::effect_facts::CallSiteTarget::KnownInstance(instance) => {
            record_instance_key_types(out, types, instance);
        }
        crate::effect_facts::CallSiteTarget::CandidateSet(instances) => {
            for instance in instances {
                record_instance_key_types(out, types, instance);
            }
        }
        crate::effect_facts::CallSiteTarget::BodylessDirect { .. } => {}
        crate::effect_facts::CallSiteTarget::DynamicFallback => {}
    }
}

fn record_call_site_facts_types(
    out: &mut HashMap<crate::ty::TypeId, String>,
    types: &TypeStore,
    facts: &crate::effect_facts::CallSiteEffectFacts,
) {
    record_call_target_types(out, types, facts.target());
    record_type_text(out, types, facts.invoke_args_tuple_ty());
}

fn record_state_graph_types(
    out: &mut HashMap<crate::ty::TypeId, String>,
    types: &TypeStore,
    state_graph: &LateLoweredStateGraph,
) {
    for state in state_graph.states() {
        if let crate::effect_lowered::ir::LateLoweredStateTerminator::HandleDispatch {
            contract,
            ..
        } = state.terminator()
        {
            record_handle_dispatch_contract_types(out, types, contract);
        }
        if let crate::effect_lowered::ir::LateLoweredStateTerminator::Return {
            payload_source,
            ..
        } = state.terminator()
        {
            record_completion_payload_source_types(out, types, payload_source);
        }
        if let crate::effect_lowered::ir::LateLoweredStateTerminator::LocalRuntimeError {
            payload_tuple_ty,
            ..
        } = state.terminator()
        {
            record_type_text(out, types, *payload_tuple_ty);
        }
    }
}

fn record_frame_schema_types(
    out: &mut HashMap<crate::ty::TypeId, String>,
    types: &TypeStore,
    frame_schema: &LateLoweredFrameSchema,
) {
    for slot in frame_schema.slots() {
        record_type_text(out, types, slot.ty());
    }
    for binding in frame_schema.completion_payload_bindings() {
        record_completion_payload_source_types(out, types, binding.payload_source());
    }
}

fn record_handle_dispatch_contract_types(
    out: &mut HashMap<crate::ty::TypeId, String>,
    types: &TypeStore,
    contract: &crate::effect_lowered::ir::LateLoweredHandleDispatchContract,
) {
    if let Some(source) = contract.body_completion_payload_source() {
        record_completion_payload_source_types(out, types, source);
    }
    for arm in contract.handled_arms() {
        record_type_text(out, types, arm.payload_tuple_ty());
        record_completion_payload_source_types(out, types, arm.completion_payload_source());
    }
    for transport in contract.pending_payload_transports() {
        record_type_text(out, types, transport.payload_tuple_ty());
    }
    for emission in contract.outward_emissions() {
        record_step_case_emission_types(out, types, emission);
    }
}

fn record_boundary_lowering_types(
    out: &mut HashMap<crate::ty::TypeId, String>,
    types: &TypeStore,
    lowering: &crate::effect_lowered::ir::LateLoweredBoundaryLowering,
) {
    match lowering {
        crate::effect_lowered::ir::LateLoweredBoundaryLowering::Call(lowering) => {
            record_call_site_facts_types(out, types, lowering.facts());
            if let Some(source) = lowering.operand_contract().carrier_source() {
                record_operand_source_types(out, types, source);
            }
            for source in lowering.operand_contract().arg_sources() {
                record_operand_source_types(out, types, source);
            }
            if let Some(runtime_error_case) = lowering.consumed_runtime_error_case() {
                record_concrete_op_key_types(
                    out,
                    types,
                    runtime_error_case.input_concrete_op_key(),
                );
                record_type_text(out, types, runtime_error_case.payload_tuple_ty());
            }
            record_step_dispatch_plan_types(out, types, lowering.dispatch());
            for composition in lowering.continuation_compositions() {
                record_continuation_contract_types(
                    out,
                    types,
                    composition.callee_continuation_contract(),
                );
                record_continuation_contract_types(
                    out,
                    types,
                    composition.caller_continuation_contract(),
                );
                record_type_text(out, types, composition.caller_result_ty());
            }
        }
        crate::effect_lowered::ir::LateLoweredBoundaryLowering::ClassCtor(lowering) => {
            for emission in lowering.emitted_steps() {
                record_step_case_emission_types(out, types, emission);
            }
        }
        crate::effect_lowered::ir::LateLoweredBoundaryLowering::Perform(lowering) => {
            record_type_text(out, types, lowering.facts().payload_tuple_ty());
            for source in lowering.operand_contract().payload_sources() {
                record_operand_source_types(out, types, source);
            }
            record_step_case_emission_types(out, types, lowering.emitted_step());
        }
        crate::effect_lowered::ir::LateLoweredBoundaryLowering::Resume(lowering) => {
            record_type_text(out, types, lowering.facts().resume_tuple_ty());
            record_type_text(out, types, lowering.facts().answer_ty());
            record_operand_source_types(
                out,
                types,
                lowering.operand_contract().continuation_source(),
            );
            for source in lowering.operand_contract().arg_sources() {
                record_operand_source_types(out, types, source);
            }
            record_step_dispatch_plan_types(out, types, lowering.dispatch());
            for composition in lowering.continuation_compositions() {
                record_continuation_contract_types(
                    out,
                    types,
                    composition.callee_continuation_contract(),
                );
                record_continuation_contract_types(
                    out,
                    types,
                    composition.caller_continuation_contract(),
                );
                record_type_text(out, types, composition.caller_result_ty());
            }
        }
        crate::effect_lowered::ir::LateLoweredBoundaryLowering::RuntimeError(lowering) => {
            record_step_case_emission_types(out, types, lowering.emitted_step());
        }
        crate::effect_lowered::ir::LateLoweredBoundaryLowering::Handle(lowering) => {
            record_type_text(out, types, lowering.facts().result_ty());
            for arm in lowering.facts().arm_facts() {
                record_type_text(out, types, arm.payload_tuple_ty());
            }
            for emission in lowering.outward_emissions() {
                record_step_case_emission_types(out, types, emission);
            }
        }
    }
}

fn record_step_dispatch_plan_types(
    out: &mut HashMap<crate::ty::TypeId, String>,
    types: &TypeStore,
    dispatch: &crate::effect_lowered::ir::LateLoweredStepDispatchPlan,
) {
    record_type_text(out, types, dispatch.complete().answer_ty());
    for forwarding in dispatch.outward_cases() {
        record_concrete_op_key_types(out, types, forwarding.input_concrete_op_key());
        record_step_case_emission_types(out, types, forwarding.emission());
    }
}

fn record_step_case_emission_types(
    out: &mut HashMap<crate::ty::TypeId, String>,
    types: &TypeStore,
    emission: &crate::effect_lowered::ir::LateLoweredStepCaseEmission,
) {
    record_concrete_op_key_types(out, types, emission.concrete_op_key());
    record_type_text(out, types, emission.payload_tuple_ty());
    record_continuation_contract_types(out, types, emission.continuation_contract());
}

fn record_operand_source_types(
    out: &mut HashMap<crate::ty::TypeId, String>,
    types: &TypeStore,
    source: &crate::effect_lowered::ir::LateLoweredOperandSource,
) {
    record_type_text(out, types, source.source_ty());
}

fn record_completion_payload_source_types(
    out: &mut HashMap<crate::ty::TypeId, String>,
    types: &TypeStore,
    source: &crate::effect_lowered::ir::LateLoweredCompletionPayloadSource,
) {
    match source {
        crate::effect_lowered::ir::LateLoweredCompletionPayloadSource::Unit { complete_ty } => {
            record_type_text(out, types, *complete_ty);
        }
        crate::effect_lowered::ir::LateLoweredCompletionPayloadSource::Operand(source) => {
            record_operand_source_types(out, types, source);
        }
    }
}

fn record_surface_resume_wrapper_complete_payload_source_types(
    out: &mut HashMap<crate::ty::TypeId, String>,
    types: &TypeStore,
    source: &crate::effect_lowered::ir::LateLoweredSurfaceResumeWrapperCompletePayloadSource,
) {
    match source {
        crate::effect_lowered::ir::LateLoweredSurfaceResumeWrapperCompletePayloadSource::OwnerComplete { answer_ty } => {
            record_type_text(out, types, *answer_ty);
        }
        crate::effect_lowered::ir::LateLoweredSurfaceResumeWrapperCompletePayloadSource::WrapperPayload(source) => {
            record_completion_payload_source_types(out, types, source);
        }
    }
}
