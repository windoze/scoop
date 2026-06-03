use std::collections::{BTreeMap, HashMap};

use scoopc_ids::{BodyBlockId, StableEffectInstanceKey};
use scoopc_types::TypeStore;
use thiserror::Error;

use crate::mir::{BasicBlockId, InstanceKey, MaterializedMirPassView};
use crate::stable_id::StableInstanceKey;

pub mod dump;
pub mod facts;
pub mod schema;

pub use facts::{
    BlockEffectFacts, BodyEffectFacts, CallSiteEffectFacts, CallSiteKind, CallSiteTarget,
    CallTargetMode, CallableAbiKind, CallableEffectFacts, CanonicalMirQuerySurface,
    ClassCtorSiteEffectFacts, EffectOwnedTypeContext, EffectPrecision, HandleArmEffectFacts,
    HandleSiteEffectFacts, MaterializedEffectFacts, MirSnapshotBinding, NestedHandleClassification,
    PerformSiteEffectFacts, ResumeSiteEffectFacts, SiteEffectFacts,
};
pub use schema::{
    CaseSet, CaseTag, ConcreteOpKey, ContinuationSchema, ContinuationSchemaId, EffectFamilyKey,
    ImplPlan, StepCaseFact, StepSchema, StepSchemaId,
};

#[derive(Debug, Error)]
pub enum EffectFactsImportError {
    #[error(
        "published effect facts reference stable instance key `{key}` that is absent from the MIR snapshot"
    )]
    MissingStableInstance { key: String },
}

impl MaterializedEffectFacts {
    pub fn from_published_effect_facts(
        pass_view: MaterializedMirPassView<'_>,
        published: &scoopc_effect_facts::EffectFacts,
        types: &TypeStore,
    ) -> Result<Self, EffectFactsImportError> {
        let snapshot_binding = MirSnapshotBinding::from_pass_view(&pass_view);
        let stable_instances = stable_instance_index(&pass_view);
        let step_schemas = published
            .step_schemas
            .iter()
            .map(|(id, schema)| {
                Ok((
                    map_step_schema_id(*id),
                    map_step_schema(schema, &stable_instances)?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>, EffectFactsImportError>>()?;
        let continuation_schemas = published
            .continuation_schemas
            .iter()
            .map(|(id, schema)| {
                (
                    map_continuation_schema_id(*id),
                    map_continuation_schema(schema),
                )
            })
            .collect();
        let callable_facts = published
            .callables
            .iter()
            .map(|(key, facts)| {
                let instance =
                    instance_for_stable_key(key, &stable_instances).ok_or_else(|| {
                        EffectFactsImportError::MissingStableInstance {
                            key: key.as_str().to_string(),
                        }
                    })?;
                Ok((instance.clone(), map_callable_facts(facts)))
            })
            .collect::<Result<HashMap<_, _>, EffectFactsImportError>>()?;
        let bodies = published
            .bodies
            .iter()
            .map(|(key, body)| {
                let instance =
                    instance_for_stable_key(key, &stable_instances).ok_or_else(|| {
                        EffectFactsImportError::MissingStableInstance {
                            key: key.as_str().to_string(),
                        }
                    })?;
                Ok((instance.clone(), map_body_facts(body, &stable_instances)?))
            })
            .collect::<Result<HashMap<_, _>, EffectFactsImportError>>()?;

        Ok(Self::new(
            EffectOwnedTypeContext::from_mir_types(types),
            snapshot_binding,
            step_schemas,
            continuation_schemas,
            callable_facts,
            bodies,
        ))
    }
}

fn stable_instance_index(
    pass_view: &MaterializedMirPassView<'_>,
) -> HashMap<StableEffectInstanceKey, (InstanceKey, StableInstanceKey)> {
    pass_view
        .materialized()
        .stable_instance_keys()
        .iter()
        .map(|(instance, stable)| {
            (
                StableEffectInstanceKey::from_symbol_key(stable),
                (instance.clone(), stable.clone()),
            )
        })
        .collect()
}

fn instance_for_stable_key<'a>(
    key: &StableEffectInstanceKey,
    stable_instances: &'a HashMap<StableEffectInstanceKey, (InstanceKey, StableInstanceKey)>,
) -> Option<&'a InstanceKey> {
    stable_instances.get(key).map(|(instance, _)| instance)
}

fn stable_instance_for_stable_key<'a>(
    key: &StableEffectInstanceKey,
    stable_instances: &'a HashMap<StableEffectInstanceKey, (InstanceKey, StableInstanceKey)>,
) -> Option<(&'a InstanceKey, &'a StableInstanceKey)> {
    stable_instances
        .get(key)
        .map(|(instance, stable)| (instance, stable))
}

fn map_step_schema(
    schema: &scoopc_effect_facts::StepSchema,
    stable_instances: &HashMap<StableEffectInstanceKey, (InstanceKey, StableInstanceKey)>,
) -> Result<StepSchema, EffectFactsImportError> {
    Ok(StepSchema::new(
        schema.invoke_args_tuple_ty(),
        schema.complete_ty(),
        schema.continuation_obj_ty(),
        schema
            .cases()
            .iter()
            .map(|case| map_step_case(case, stable_instances))
            .collect::<Result<Vec<_>, _>>()?,
    ))
}

fn map_step_case(
    case: &scoopc_effect_facts::StepCaseFact,
    stable_instances: &HashMap<StableEffectInstanceKey, (InstanceKey, StableInstanceKey)>,
) -> Result<StepCaseFact, EffectFactsImportError> {
    Ok(StepCaseFact::new(
        map_case_tag(case.case_tag()),
        map_concrete_op_key(case.concrete_op_key(), stable_instances)?,
        case.payload_tuple_ty(),
        map_continuation_schema_id(case.continuation_schema()),
    ))
}

fn map_concrete_op_key(
    key: &scoopc_effect_facts::ConcreteOpKey,
    stable_instances: &HashMap<StableEffectInstanceKey, (InstanceKey, StableInstanceKey)>,
) -> Result<ConcreteOpKey, EffectFactsImportError> {
    let (instance, stable) =
        stable_instance_for_stable_key(key.stable_instance_key(), stable_instances).ok_or_else(
            || EffectFactsImportError::MissingStableInstance {
                key: key.stable_instance_key().as_str().to_string(),
            },
        )?;
    Ok(ConcreteOpKey::new(
        instance.clone(),
        stable.clone(),
        map_effect_family_key(key.effect_family()),
    ))
}

fn map_effect_family_key(key: &scoopc_effect_facts::EffectFamilyKey) -> EffectFamilyKey {
    EffectFamilyKey::new(key.effect_fqn().to_string(), key.type_args().to_vec())
}

fn map_continuation_schema(schema: &scoopc_effect_facts::ContinuationSchema) -> ContinuationSchema {
    ContinuationSchema::new(
        schema.resume_tuple_ty(),
        schema.answer_ty(),
        map_step_schema_id(schema.out_step_schema()),
        schema.surface_ty(),
    )
}

fn map_callable_facts(facts: &scoopc_effect_facts::CallableEffectFacts) -> CallableEffectFacts {
    CallableEffectFacts::new(
        facts.declared_row().clone(),
        map_callable_abi(facts.call_abi_kind()),
        facts.invoke_args_tuple_ty_opt(),
        facts.body_step_schema().map(map_step_schema_id),
        map_case_set(facts.resolved_outward_cases()),
        facts.needs_reentry(),
        map_impl_plan(facts.impl_plan()),
    )
}

fn map_body_facts(
    body: &scoopc_effect_facts::BodyEffectFacts,
    stable_instances: &HashMap<StableEffectInstanceKey, (InstanceKey, StableInstanceKey)>,
) -> Result<BodyEffectFacts, EffectFactsImportError> {
    let blocks = body
        .blocks()
        .iter()
        .map(|(block_id, block)| (map_body_block_id(*block_id), map_block_facts(block)))
        .collect();
    let sites = body
        .sites()
        .iter()
        .map(|(site_id, site)| Ok((*site_id, map_site_facts(site, stable_instances)?)))
        .collect::<Result<BTreeMap<_, _>, EffectFactsImportError>>()?;
    Ok(BodyEffectFacts::with_solver_facts(
        blocks,
        sites,
        body.local_control_step_schema().map(map_step_schema_id),
        facts::BodyEffectSolverFacts::default(),
    ))
}

fn map_block_facts(block: &scoopc_effect_facts::BlockEffectFacts) -> BlockEffectFacts {
    BlockEffectFacts::new(
        map_case_set(block.ambient_cases()),
        map_case_set(block.outward_cases()),
        block.has_suspend_boundary(),
        block.has_handle_boundary(),
    )
}

fn map_site_facts(
    site: &scoopc_effect_facts::SiteEffectFacts,
    stable_instances: &HashMap<StableEffectInstanceKey, (InstanceKey, StableInstanceKey)>,
) -> Result<SiteEffectFacts, EffectFactsImportError> {
    match site {
        scoopc_effect_facts::SiteEffectFacts::Call(call) => Ok(SiteEffectFacts::Call(
            map_call_site(call, stable_instances)?,
        )),
        scoopc_effect_facts::SiteEffectFacts::ClassCtor(class_ctor) => {
            Ok(SiteEffectFacts::ClassCtor(ClassCtorSiteEffectFacts::new(
                map_case_set(class_ctor.emitted_cases()),
            )))
        }
        scoopc_effect_facts::SiteEffectFacts::Perform(perform) => {
            Ok(SiteEffectFacts::Perform(PerformSiteEffectFacts::new(
                map_case_tag(perform.emitted_case()),
                perform.payload_tuple_ty(),
                map_continuation_schema_id(perform.captured_cont_schema()),
            )))
        }
        scoopc_effect_facts::SiteEffectFacts::Resume(resume) => {
            Ok(SiteEffectFacts::Resume(ResumeSiteEffectFacts::new(
                map_continuation_schema_id(resume.continuation_schema()),
                resume.resume_tuple_ty(),
                resume.answer_ty(),
                map_step_schema_id(resume.out_step_schema()),
                map_case_set(resume.resolved_cases()),
            )))
        }
        scoopc_effect_facts::SiteEffectFacts::Handle(handle) => {
            Ok(SiteEffectFacts::Handle(HandleSiteEffectFacts::new(
                handle.result_ty(),
                map_case_set(handle.handled_cases()),
                map_case_set(handle.body_outward_cases()),
                handle.arm_facts().iter().map(map_handle_arm).collect(),
                map_case_set(handle.finally_outward_cases()),
                map_nested_handle_classification(handle.nested_handle_classification()),
            )))
        }
    }
}

fn map_call_site(
    call: &scoopc_effect_facts::CallSiteEffectFacts,
    stable_instances: &HashMap<StableEffectInstanceKey, (InstanceKey, StableInstanceKey)>,
) -> Result<CallSiteEffectFacts, EffectFactsImportError> {
    Ok(CallSiteEffectFacts::new_with_abi(
        map_call_site_kind(call.kind()),
        map_call_site_target(call.target(), stable_instances)?,
        map_callable_abi(call.callee_abi_kind()),
        call.invoke_args_tuple_ty(),
        call.callee_step_schema().map(map_step_schema_id),
        map_case_set(call.resolved_cases()),
        map_effect_precision(call.precision()),
    ))
}

fn map_call_site_target(
    target: &scoopc_effect_facts::CallSiteTarget,
    stable_instances: &HashMap<StableEffectInstanceKey, (InstanceKey, StableInstanceKey)>,
) -> Result<CallSiteTarget, EffectFactsImportError> {
    match target {
        scoopc_effect_facts::CallSiteTarget::KnownInstance(key) => {
            instance_for_stable_key(key, stable_instances)
                .cloned()
                .map(CallSiteTarget::KnownInstance)
                .ok_or_else(|| EffectFactsImportError::MissingStableInstance {
                    key: key.as_str().to_string(),
                })
        }
        scoopc_effect_facts::CallSiteTarget::CandidateSet(keys) => {
            let instances = keys
                .iter()
                .map(|key| {
                    instance_for_stable_key(key, stable_instances)
                        .cloned()
                        .ok_or_else(|| EffectFactsImportError::MissingStableInstance {
                            key: key.as_str().to_string(),
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            if instances.is_empty() {
                return Err(EffectFactsImportError::MissingStableInstance {
                    key: "<empty candidate set>".to_string(),
                });
            }
            Ok(CallSiteTarget::CandidateSet(instances))
        }
        scoopc_effect_facts::CallSiteTarget::BodylessDirect { fqn } => {
            Ok(CallSiteTarget::BodylessDirect { fqn: fqn.clone() })
        }
        scoopc_effect_facts::CallSiteTarget::DynamicFallback => Ok(CallSiteTarget::DynamicFallback),
    }
}

fn map_handle_arm(arm: &scoopc_effect_facts::HandleArmEffectFacts) -> HandleArmEffectFacts {
    HandleArmEffectFacts::new(
        map_case_tag(arm.handled_case()),
        arm.payload_tuple_ty(),
        map_continuation_schema_id(arm.continuation_schema()),
        map_case_set(arm.arm_outward_cases()),
    )
}

fn map_case_set(case_set: &scoopc_effect_facts::CaseSet) -> CaseSet {
    CaseSet::new(
        map_step_schema_id(case_set.schema()),
        case_set.tags().iter().copied().map(map_case_tag).collect(),
    )
}

fn map_step_schema_id(id: scoopc_effect_facts::StepSchemaId) -> StepSchemaId {
    StepSchemaId::new(id.as_u32())
}

fn map_continuation_schema_id(
    id: scoopc_effect_facts::ContinuationSchemaId,
) -> ContinuationSchemaId {
    ContinuationSchemaId::new(id.as_u32())
}

fn map_case_tag(tag: scoopc_effect_facts::CaseTag) -> CaseTag {
    CaseTag::new(tag.as_u32())
}

fn map_body_block_id(id: BodyBlockId) -> BasicBlockId {
    BasicBlockId::from_raw(id.as_u32())
}

fn map_callable_abi(kind: scoopc_effect_facts::CallableAbiKind) -> CallableAbiKind {
    match kind {
        scoopc_effect_facts::CallableAbiKind::Plain => CallableAbiKind::Plain,
        scoopc_effect_facts::CallableAbiKind::EffectStep => CallableAbiKind::EffectStep,
    }
}

fn map_call_site_kind(kind: scoopc_effect_facts::CallSiteKind) -> CallSiteKind {
    match kind {
        scoopc_effect_facts::CallSiteKind::Direct => CallSiteKind::Direct,
        scoopc_effect_facts::CallSiteKind::Closure => CallSiteKind::Closure,
        scoopc_effect_facts::CallSiteKind::FunValue => CallSiteKind::FunValue,
        scoopc_effect_facts::CallSiteKind::FunPtr => CallSiteKind::FunPtr,
        scoopc_effect_facts::CallSiteKind::Virtual => CallSiteKind::Virtual,
        scoopc_effect_facts::CallSiteKind::Interface => CallSiteKind::Interface,
    }
}

fn map_effect_precision(precision: scoopc_effect_facts::EffectPrecision) -> EffectPrecision {
    match precision {
        scoopc_effect_facts::EffectPrecision::Precise => EffectPrecision::Precise,
        scoopc_effect_facts::EffectPrecision::Widened => EffectPrecision::Widened,
        scoopc_effect_facts::EffectPrecision::SignatureFallback => {
            EffectPrecision::SignatureFallback
        }
    }
}

fn map_impl_plan(plan: scoopc_effect_facts::ImplPlan) -> ImplPlan {
    match plan {
        scoopc_effect_facts::ImplPlan::NoOutward => ImplPlan::NoOutward,
        scoopc_effect_facts::ImplPlan::SingleCase(tag) => ImplPlan::SingleCase(map_case_tag(tag)),
        scoopc_effect_facts::ImplPlan::CanonicalFull => ImplPlan::CanonicalFull,
    }
}

fn map_nested_handle_classification(
    classification: scoopc_effect_facts::NestedHandleClassification,
) -> NestedHandleClassification {
    match classification {
        scoopc_effect_facts::NestedHandleClassification::SelfContained => {
            NestedHandleClassification::SelfContained
        }
        scoopc_effect_facts::NestedHandleClassification::MaySuspendOutward => {
            NestedHandleClassification::MaySuspendOutward
        }
    }
}
