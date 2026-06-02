use std::collections::BTreeMap;

use scoopc_effect_facts as published;
use scoopc_ids::{BodyBlockId, StableEffectInstanceKey};
use thiserror::Error;

use crate::mir::{InstanceKey, MaterializedMirPassView};

use super::{
    BlockEffectFacts, BodyEffectFacts, CallSiteEffectFacts, CallSiteKind, CallSiteTarget,
    CallableAbiKind, CallableEffectFacts, CanonicalMirQuerySurface, ClassCtorSiteEffectFacts,
    EffectPrecision, HandleArmEffectFacts, HandleSiteEffectFacts, MaterializedEffectFacts,
    MirSnapshotBinding, NestedHandleClassification, PerformSiteEffectFacts, ResumeSiteEffectFacts,
    SiteEffectFacts,
};

/// Error raised while adapting the current monolithic materialized facts into
/// the independent `scoopc_effect_facts` data product.
#[derive(Debug, Error)]
pub enum EffectFactsProductError {
    #[error("effect facts reference an instance without a stable key: {instance}")]
    MissingStableInstanceKey { instance: String },

    #[error(transparent)]
    Verify(#[from] published::verify::VerifyError),
}

impl MaterializedEffectFacts {
    /// Convert the current materialized effect facts into the independent fact crate product.
    ///
    /// The adapter is the P4-T01 compatibility seam: production still builds the
    /// legacy MIR-keyed structure, while later tasks can move construction to the
    /// published stable-key product without changing the fact crate boundary.
    pub fn to_published_effect_facts(
        &self,
        pass_view: MaterializedMirPassView<'_>,
    ) -> Result<published::EffectFacts, EffectFactsProductError> {
        let mut step_schemas = BTreeMap::new();
        for (schema_id, schema) in self.step_schemas() {
            step_schemas.insert(map_step_schema_id(*schema_id), map_step_schema(schema));
        }

        let mut continuation_schemas = BTreeMap::new();
        for (schema_id, schema) in self.continuation_schemas() {
            continuation_schemas.insert(
                map_continuation_schema_id(*schema_id),
                published::ContinuationSchema::new(
                    schema.resume_tuple_ty(),
                    schema.answer_ty(),
                    map_step_schema_id(schema.out_step_schema()),
                    schema.surface_ty(),
                ),
            );
        }

        let mut callables = BTreeMap::new();
        for (instance, callable) in self.callable_facts() {
            let stable_key = stable_key_for_instance(&pass_view, instance).ok_or_else(|| {
                EffectFactsProductError::MissingStableInstanceKey {
                    instance: instance.template.fqn.clone(),
                }
            })?;
            callables.insert(stable_key, map_callable_facts(callable));
        }

        let mut bodies = BTreeMap::new();
        for (instance, body) in self.bodies() {
            let stable_key = stable_key_for_instance(&pass_view, instance).ok_or_else(|| {
                EffectFactsProductError::MissingStableInstanceKey {
                    instance: instance.template.fqn.clone(),
                }
            })?;
            bodies.insert(stable_key, map_body_facts(body, &pass_view)?);
        }

        let facts = published::EffectFacts::from_parts(
            map_snapshot_binding(self.snapshot_binding()),
            step_schemas,
            continuation_schemas,
            callables,
            bodies,
        );
        facts.verify()?;
        Ok(facts)
    }
}

fn map_snapshot_binding(binding: &MirSnapshotBinding) -> published::EffectSnapshotBinding {
    published::EffectSnapshotBinding::new(
        match binding.query_surface() {
            CanonicalMirQuerySurface::PassView => published::CanonicalMirQuerySurface::PassView,
        },
        binding.instance_count(),
        binding.canonical_body_fqns().to_vec(),
    )
}

fn map_step_schema(schema: &super::StepSchema) -> published::StepSchema {
    published::StepSchema::new(
        schema.invoke_args_tuple_ty(),
        schema.complete_ty(),
        schema.continuation_obj_ty(),
        schema.cases().iter().map(map_step_case).collect(),
    )
}

fn map_step_case(case: &super::StepCaseFact) -> published::StepCaseFact {
    published::StepCaseFact::new(
        map_case_tag(case.case_tag()),
        published::ConcreteOpKey::new(
            StableEffectInstanceKey::from_symbol_key(case.concrete_op_key().stable_instance_key()),
            published::EffectFamilyKey::new(
                case.concrete_op_key()
                    .effect_family()
                    .effect_fqn()
                    .to_string(),
                case.concrete_op_key().effect_family().type_args().to_vec(),
            ),
        ),
        case.payload_tuple_ty(),
        map_continuation_schema_id(case.continuation_schema()),
    )
}

fn map_callable_facts(callable: &CallableEffectFacts) -> published::CallableEffectFacts {
    published::CallableEffectFacts::new(
        callable.declared_row().clone(),
        map_callable_abi(callable.call_abi_kind()),
        callable.invoke_args_tuple_ty_opt(),
        callable.body_step_schema().map(map_step_schema_id),
        map_case_set(callable.resolved_outward_cases()),
        callable.needs_reentry(),
        map_impl_plan(callable.impl_plan()),
    )
}

fn map_body_facts(
    body: &BodyEffectFacts,
    pass_view: &MaterializedMirPassView<'_>,
) -> Result<published::BodyEffectFacts, EffectFactsProductError> {
    let blocks = body
        .blocks()
        .iter()
        .map(|(block_id, block)| {
            (
                BodyBlockId::from_raw(block_id.as_u32()),
                map_block_facts(block),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut sites = BTreeMap::new();
    for (site_id, site) in body.sites() {
        sites.insert(*site_id, map_site_facts(site, pass_view)?);
    }
    Ok(published::BodyEffectFacts::with_local_control_step_schema(
        blocks,
        sites,
        body.local_control_step_schema().map(map_step_schema_id),
    ))
}

fn map_block_facts(block: &BlockEffectFacts) -> published::BlockEffectFacts {
    published::BlockEffectFacts::new(
        map_case_set(block.ambient_cases()),
        map_case_set(block.outward_cases()),
        block.has_suspend_boundary(),
        block.has_handle_boundary(),
    )
}

fn map_site_facts(
    site: &SiteEffectFacts,
    pass_view: &MaterializedMirPassView<'_>,
) -> Result<published::SiteEffectFacts, EffectFactsProductError> {
    match site {
        SiteEffectFacts::Call(call) => Ok(published::SiteEffectFacts::Call(map_call_site(
            call, pass_view,
        )?)),
        SiteEffectFacts::ClassCtor(class_ctor) => Ok(published::SiteEffectFacts::ClassCtor(
            map_class_ctor_site(class_ctor),
        )),
        SiteEffectFacts::Perform(perform) => Ok(published::SiteEffectFacts::Perform(
            map_perform_site(perform),
        )),
        SiteEffectFacts::Resume(resume) => {
            Ok(published::SiteEffectFacts::Resume(map_resume_site(resume)))
        }
        SiteEffectFacts::Handle(handle) => {
            Ok(published::SiteEffectFacts::Handle(map_handle_site(handle)))
        }
    }
}

fn map_call_site(
    call: &CallSiteEffectFacts,
    pass_view: &MaterializedMirPassView<'_>,
) -> Result<published::CallSiteEffectFacts, EffectFactsProductError> {
    Ok(published::CallSiteEffectFacts::new_with_abi(
        map_call_site_kind(call.kind()),
        map_call_site_target(call.target(), pass_view)?,
        map_callable_abi(call.callee_abi_kind()),
        call.invoke_args_tuple_ty(),
        call.callee_step_schema().map(map_step_schema_id),
        map_case_set(call.resolved_cases()),
        map_effect_precision(call.precision())?,
    ))
}

fn map_class_ctor_site(
    class_ctor: &ClassCtorSiteEffectFacts,
) -> published::ClassCtorSiteEffectFacts {
    published::ClassCtorSiteEffectFacts::new(map_case_set(class_ctor.emitted_cases()))
}

fn map_perform_site(perform: &PerformSiteEffectFacts) -> published::PerformSiteEffectFacts {
    published::PerformSiteEffectFacts::new(
        map_case_tag(perform.emitted_case()),
        perform.payload_tuple_ty(),
        map_continuation_schema_id(perform.captured_cont_schema()),
    )
}

fn map_resume_site(resume: &ResumeSiteEffectFacts) -> published::ResumeSiteEffectFacts {
    published::ResumeSiteEffectFacts::new(
        map_continuation_schema_id(resume.continuation_schema()),
        resume.resume_tuple_ty(),
        resume.answer_ty(),
        map_step_schema_id(resume.out_step_schema()),
        map_case_set(resume.resolved_cases()),
    )
}

fn map_handle_site(handle: &HandleSiteEffectFacts) -> published::HandleSiteEffectFacts {
    published::HandleSiteEffectFacts::new(
        handle.result_ty(),
        map_case_set(handle.handled_cases()),
        map_case_set(handle.body_outward_cases()),
        handle.arm_facts().iter().map(map_handle_arm).collect(),
        map_case_set(handle.finally_outward_cases()),
        map_nested_handle_classification(handle.nested_handle_classification()),
    )
}

fn map_handle_arm(arm: &HandleArmEffectFacts) -> published::HandleArmEffectFacts {
    published::HandleArmEffectFacts::new(
        map_case_tag(arm.handled_case()),
        arm.payload_tuple_ty(),
        map_continuation_schema_id(arm.continuation_schema()),
        map_case_set(arm.arm_outward_cases()),
    )
}

fn map_call_site_target(
    target: &CallSiteTarget,
    pass_view: &MaterializedMirPassView<'_>,
) -> Result<published::CallSiteTarget, EffectFactsProductError> {
    match target {
        CallSiteTarget::KnownInstance(instance) => {
            let key = stable_key_for_instance(pass_view, instance).ok_or_else(|| {
                EffectFactsProductError::MissingStableInstanceKey {
                    instance: instance.template.fqn.clone(),
                }
            })?;
            Ok(published::CallSiteTarget::KnownInstance(key))
        }
        CallSiteTarget::CandidateSet(instances) => {
            let stable_keys = instances
                .iter()
                .map(|instance| {
                    stable_key_for_instance(pass_view, instance).ok_or_else(|| {
                        EffectFactsProductError::MissingStableInstanceKey {
                            instance: instance.template.fqn.clone(),
                        }
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(published::CallSiteTarget::CandidateSet(stable_keys))
        }
        CallSiteTarget::DynamicFallback => Ok(published::CallSiteTarget::DynamicFallback),
    }
}

fn stable_key_for_instance(
    pass_view: &MaterializedMirPassView<'_>,
    instance: &InstanceKey,
) -> Option<StableEffectInstanceKey> {
    pass_view
        .materialized()
        .stable_instance_key(instance)
        .map(StableEffectInstanceKey::from_symbol_key)
}

fn map_case_set(case_set: &super::CaseSet) -> published::CaseSet {
    published::CaseSet::new(
        map_step_schema_id(case_set.schema()),
        case_set.tags().iter().copied().map(map_case_tag).collect(),
    )
}

fn map_step_schema_id(id: super::StepSchemaId) -> published::StepSchemaId {
    published::StepSchemaId::new(id.as_u32())
}

fn map_continuation_schema_id(id: super::ContinuationSchemaId) -> published::ContinuationSchemaId {
    published::ContinuationSchemaId::new(id.as_u32())
}

fn map_case_tag(tag: super::CaseTag) -> published::CaseTag {
    published::CaseTag::new(tag.as_u32())
}

fn map_impl_plan(plan: super::ImplPlan) -> published::ImplPlan {
    match plan {
        super::ImplPlan::NoOutward => published::ImplPlan::NoOutward,
        super::ImplPlan::SingleCase(tag) => published::ImplPlan::SingleCase(map_case_tag(tag)),
        super::ImplPlan::CanonicalFull => published::ImplPlan::CanonicalFull,
    }
}

fn map_callable_abi(kind: CallableAbiKind) -> published::CallableAbiKind {
    match kind {
        CallableAbiKind::Plain => published::CallableAbiKind::Plain,
        CallableAbiKind::EffectStep => published::CallableAbiKind::EffectStep,
    }
}

fn map_effect_precision(
    precision: EffectPrecision,
) -> Result<published::EffectPrecision, EffectFactsProductError> {
    match precision {
        EffectPrecision::Precise => Ok(published::EffectPrecision::Precise),
        EffectPrecision::Widened => Ok(published::EffectPrecision::Widened),
        EffectPrecision::SignatureFallback => Err(EffectFactsProductError::Verify(
            published::verify::VerifyError::InvalidCallSiteFallbackPrecision {
                context: "effect facts product adapter".to_string(),
            },
        )),
    }
}

fn map_call_site_kind(kind: CallSiteKind) -> published::CallSiteKind {
    match kind {
        CallSiteKind::Direct => published::CallSiteKind::Direct,
        CallSiteKind::Closure => published::CallSiteKind::Closure,
        CallSiteKind::FunValue => published::CallSiteKind::FunValue,
        CallSiteKind::FunPtr => published::CallSiteKind::FunPtr,
        CallSiteKind::Virtual => published::CallSiteKind::Virtual,
        CallSiteKind::Interface => published::CallSiteKind::Interface,
    }
}

fn map_nested_handle_classification(
    classification: NestedHandleClassification,
) -> published::NestedHandleClassification {
    match classification {
        NestedHandleClassification::SelfContained => {
            published::NestedHandleClassification::SelfContained
        }
        NestedHandleClassification::MaySuspendOutward => {
            published::NestedHandleClassification::MaySuspendOutward
        }
    }
}
