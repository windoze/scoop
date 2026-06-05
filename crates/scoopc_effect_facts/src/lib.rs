//! Effect/control fact product shared by LIR and backend-neutral consumers.
//!
//! This crate is intentionally data-only: it depends only on the
//! stage-independent base crates for identity and type/effect-row primitives. It
//! does not depend on the `scoopc` facade, MIR pass views, LIR, or backend ABI
//! types. The P4 stage publishes this product as `EffectFactsStageOutput =
//! { effect_facts }` from a read-only MIR handoff and records effect-owned
//! context here instead of writing derived types back into MIR or nesting the
//! upstream MIR stage output.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use scoopc_ids::StableEffectInstanceKey;

pub mod dump;
pub mod facts;
pub mod schema;
pub mod verify;

pub use facts::{
    BlockEffectFacts, BodyEffectFacts, CallSiteEffectFacts, CallSiteKind, CallSiteTarget,
    CallTargetMode, CallableAbiKind, CallableEffectFacts, CanonicalMirQuerySurface,
    ClassCtorSiteEffectFacts, EffectPrecision, EffectSnapshotBinding, HandleArmEffectFacts,
    HandleSiteEffectFacts, NestedHandleClassification, PerformSiteEffectFacts,
    ResumeSiteEffectFacts, SiteEffectFacts,
};
pub use schema::{
    CaseSet, CaseTag, ConcreteOpKey, ContinuationSchema, ContinuationSchemaId, EffectFamilyKey,
    ImplPlan, StepCaseFact, StepSchema, StepSchemaId,
};

/// Complete effect/control fact product published by the effect-facts stage.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EffectFacts {
    pub schema_version: scoopc_types::WireSchemaVersion,
    pub snapshot_binding: EffectSnapshotBinding,
    pub step_schemas: BTreeMap<StepSchemaId, StepSchema>,
    pub continuation_schemas: BTreeMap<ContinuationSchemaId, ContinuationSchema>,
    pub callables: BTreeMap<StableEffectInstanceKey, CallableEffectFacts>,
    pub bodies: BTreeMap<StableEffectInstanceKey, BodyEffectFacts>,
}

impl Default for EffectFacts {
    fn default() -> Self {
        Self {
            schema_version: scoopc_types::WIRE_SCHEMA_VERSION,
            snapshot_binding: EffectSnapshotBinding::default(),
            step_schemas: BTreeMap::new(),
            continuation_schemas: BTreeMap::new(),
            callables: BTreeMap::new(),
            bodies: BTreeMap::new(),
        }
    }
}

impl EffectFacts {
    /// Create an empty fact product for tests and staged construction.
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a fact product from already materialized fact groups.
    pub fn from_parts(
        snapshot_binding: EffectSnapshotBinding,
        step_schemas: BTreeMap<StepSchemaId, StepSchema>,
        continuation_schemas: BTreeMap<ContinuationSchemaId, ContinuationSchema>,
        callables: BTreeMap<StableEffectInstanceKey, CallableEffectFacts>,
        bodies: BTreeMap<StableEffectInstanceKey, BodyEffectFacts>,
    ) -> Self {
        Self {
            schema_version: scoopc_types::WIRE_SCHEMA_VERSION,
            snapshot_binding,
            step_schemas,
            continuation_schemas,
            callables,
            bodies,
        }
    }

    /// Return whether all fact groups are currently empty.
    pub fn is_empty(&self) -> bool {
        self.step_schemas.is_empty()
            && self.continuation_schemas.is_empty()
            && self.callables.is_empty()
            && self.bodies.is_empty()
    }

    /// Verify structural invariants before handing facts to later stages.
    pub fn verify(&self) -> verify::Result<()> {
        verify::verify_effect_facts(self)
    }

    /// Render a stable textual summary of the fact groups.
    pub fn dump(&self) -> String {
        dump::dump_effect_facts(self)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use scoopc_ids::StableEffectInstanceKey;
    use scoopc_types::{EffectRow, TypeKind, TypeStore, ValueTypeKind};

    use super::*;
    use crate::verify::VerifyError;

    #[test]
    fn empty_effect_facts_verify_and_dump_group_boundaries() {
        let facts = EffectFacts::new();

        assert!(facts.is_empty());
        assert!(facts.verify().is_ok());

        let dump = facts.dump();
        assert!(dump.contains("snapshot_binding: surface=PassView"));
        assert!(dump.contains("schemas: steps=0 continuations=0"));
        assert!(dump.contains("bodies=0"));
    }

    #[test]
    fn effect_facts_bincode_round_trip_preserves_schema_and_content() {
        let facts = EffectFacts::new();
        let bytes = bincode::serialize(&facts).expect("serialize effect facts");
        let decoded: EffectFacts = bincode::deserialize(&bytes).expect("deserialize effect facts");

        assert_eq!(decoded.schema_version, scoopc_types::WIRE_SCHEMA_VERSION);
        assert_eq!(decoded, facts);
    }

    #[test]
    fn effect_facts_bincode_round_trip_preserves_step_and_continuation_contracts() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let effect = types.intern(TypeKind::Ref(scoopc_types::RefTypeKind::Nominal(
            scoopc_types::NominalType {
                fqn: "app.Log".to_string(),
                args: vec![builtins.string],
                eff: None,
            },
        )));
        let invoke_tuple = types.ty_tuple(vec![builtins.string]);
        let resume_tuple = types.ty_tuple(vec![builtins.int]);
        let continuation_surface = types.intern(TypeKind::Ref(scoopc_types::RefTypeKind::Nominal(
            scoopc_types::NominalType {
                fqn: "app.Continuation".to_string(),
                args: vec![builtins.unit],
                eff: Some(EffectRow::new(vec![effect])),
            },
        )));
        let step_schema_id = StepSchemaId::new(1);
        let continuation_schema_id = ContinuationSchemaId::new(2);
        let case_tag = CaseTag::new(3);
        let callable = StableEffectInstanceKey::new("effect(instance(app.main))", "app.main");
        let op_key = ConcreteOpKey::new(
            callable.clone(),
            EffectFamilyKey::new("app.Log".to_string(), vec![builtins.string]),
        );

        let mut step_schemas = BTreeMap::new();
        step_schemas.insert(
            step_schema_id,
            StepSchema::new(
                invoke_tuple,
                builtins.unit,
                continuation_surface,
                vec![StepCaseFact::new(
                    case_tag,
                    op_key,
                    resume_tuple,
                    continuation_schema_id,
                )],
            ),
        );
        let mut continuation_schemas = BTreeMap::new();
        continuation_schemas.insert(
            continuation_schema_id,
            ContinuationSchema::new(
                resume_tuple,
                builtins.unit,
                step_schema_id,
                continuation_surface,
            ),
        );
        let mut callables = BTreeMap::new();
        callables.insert(
            callable.clone(),
            CallableEffectFacts::new(
                EffectRow::new(vec![effect]),
                CallableAbiKind::EffectStep,
                Some(invoke_tuple),
                Some(step_schema_id),
                CaseSet::new(step_schema_id, vec![case_tag]),
                true,
                ImplPlan::CanonicalFull,
            ),
        );
        let mut bodies = BTreeMap::new();
        bodies.insert(
            callable,
            BodyEffectFacts::with_local_control_step_schema(
                BTreeMap::new(),
                BTreeMap::new(),
                Some(step_schema_id),
            ),
        );
        let facts = EffectFacts::from_parts(
            EffectSnapshotBinding::default(),
            step_schemas,
            continuation_schemas,
            callables,
            bodies,
        );

        let bytes = bincode::serialize(&facts).expect("serialize populated effect facts");
        let decoded: EffectFacts =
            bincode::deserialize(&bytes).expect("deserialize populated effect facts");

        assert_eq!(decoded.schema_version, scoopc_types::WIRE_SCHEMA_VERSION);
        assert_eq!(decoded, facts);
    }

    #[test]
    fn verifier_rejects_missing_callable_step_schema() {
        let unit = unit_ty();
        let main_key = callable_key("app.main");
        let missing_schema = StepSchemaId::new(7);
        let mut facts = EffectFacts::new();
        facts.callables.insert(
            main_key,
            CallableEffectFacts::new(
                EffectRow::pure(),
                CallableAbiKind::EffectStep,
                Some(unit),
                Some(missing_schema),
                CaseSet::new(missing_schema, vec![CaseTag::new(0)]),
                false,
                ImplPlan::CanonicalFull,
            ),
        );

        assert_eq!(
            facts.verify().unwrap_err(),
            VerifyError::MissingStepSchema {
                context: "callable app.main body_step_schema".to_string(),
                schema: 7,
            }
        );
    }

    #[test]
    fn verifier_rejects_missing_plain_local_control_step_schema() {
        let unit = unit_ty();
        let main_key = callable_key("app.main");
        let step = StepSchemaId::new(0);
        let continuation = ContinuationSchemaId::new(0);
        let case = CaseTag::new(0);

        let mut facts = EffectFacts::new();
        facts.step_schemas.insert(
            step,
            StepSchema::new(
                unit,
                unit,
                unit,
                vec![StepCaseFact::new(
                    case,
                    ConcreteOpKey::new(
                        callable_key("app.Ping.hit"),
                        EffectFamilyKey::new("app.Ping".to_string(), Vec::new()),
                    ),
                    unit,
                    continuation,
                )],
            ),
        );
        facts.continuation_schemas.insert(
            continuation,
            ContinuationSchema::new(unit, unit, step, unit),
        );
        facts.callables.insert(
            main_key.clone(),
            CallableEffectFacts::new(
                EffectRow::pure(),
                CallableAbiKind::Plain,
                None,
                None,
                CaseSet::new(step, Vec::new()),
                false,
                ImplPlan::NoOutward,
            ),
        );

        let mut sites = BTreeMap::new();
        sites.insert(
            scoopc_ids::SiteId::from_raw(0),
            SiteEffectFacts::Perform(PerformSiteEffectFacts::new(case, unit, continuation)),
        );
        facts
            .bodies
            .insert(main_key, BodyEffectFacts::new(BTreeMap::new(), sites));

        assert_eq!(
            facts.verify().unwrap_err(),
            VerifyError::MissingLocalControlStepSchema {
                body: "app.main".to_string(),
            }
        );
    }

    #[test]
    fn verifier_rejects_empty_call_site_candidate_set() {
        let unit = unit_ty();
        let main_key = callable_key("app.main");
        let step = StepSchemaId::new(0);

        let mut facts = EffectFacts::new();
        facts.callables.insert(
            main_key.clone(),
            CallableEffectFacts::new(
                EffectRow::pure(),
                CallableAbiKind::Plain,
                None,
                None,
                CaseSet::new(step, Vec::new()),
                false,
                ImplPlan::NoOutward,
            ),
        );

        let mut sites = BTreeMap::new();
        sites.insert(
            scoopc_ids::SiteId::from_raw(0),
            SiteEffectFacts::Call(CallSiteEffectFacts::new_with_abi(
                CallSiteKind::FunValue,
                CallSiteTarget::CandidateSet(Vec::new()),
                CallableAbiKind::Plain,
                unit,
                None,
                CaseSet::new(step, Vec::new()),
                EffectPrecision::Precise,
            )),
        );
        facts
            .bodies
            .insert(main_key, BodyEffectFacts::new(BTreeMap::new(), sites));

        assert_eq!(
            facts.verify().unwrap_err(),
            VerifyError::EmptyCallSiteCandidateSet {
                context: "body app.main site0".to_string(),
            }
        );
    }

    #[test]
    fn verifier_rejects_call_site_abi_schema_mismatch() {
        let unit = unit_ty();
        let main_key = callable_key("app.main");
        let step = StepSchemaId::new(0);
        let continuation = ContinuationSchemaId::new(0);
        let case = CaseTag::new(0);

        let mut facts = EffectFacts::new();
        facts.step_schemas.insert(
            step,
            StepSchema::new(
                unit,
                unit,
                unit,
                vec![StepCaseFact::new(
                    case,
                    ConcreteOpKey::new(
                        callable_key("app.Ping.hit"),
                        EffectFamilyKey::new("app.Ping".to_string(), Vec::new()),
                    ),
                    unit,
                    continuation,
                )],
            ),
        );
        facts.continuation_schemas.insert(
            continuation,
            ContinuationSchema::new(unit, unit, step, unit),
        );
        facts.callables.insert(
            main_key.clone(),
            CallableEffectFacts::new(
                EffectRow::pure(),
                CallableAbiKind::Plain,
                None,
                None,
                CaseSet::new(step, Vec::new()),
                false,
                ImplPlan::NoOutward,
            ),
        );

        let mut sites = BTreeMap::new();
        sites.insert(
            scoopc_ids::SiteId::from_raw(0),
            SiteEffectFacts::Call(CallSiteEffectFacts::new_with_abi(
                CallSiteKind::FunValue,
                CallSiteTarget::DynamicFallback,
                CallableAbiKind::Plain,
                unit,
                None,
                CaseSet::new(step, vec![case]),
                EffectPrecision::Precise,
            )),
        );
        facts.bodies.insert(
            main_key,
            BodyEffectFacts::with_local_control_step_schema(BTreeMap::new(), sites, Some(step)),
        );

        assert_eq!(
            facts.verify().unwrap_err(),
            VerifyError::CallSiteAbiSchemaMismatch {
                context: "body app.main site0".to_string(),
            }
        );
    }

    #[test]
    fn verifier_accepts_complete_callable_body_and_schema_graph() {
        let unit = unit_ty();
        let main_key = callable_key("app.main");
        let op_key = callable_key("app.Ping.hit");
        let step = StepSchemaId::new(0);
        let continuation = ContinuationSchemaId::new(0);
        let case = CaseTag::new(0);

        let mut step_schemas = BTreeMap::new();
        step_schemas.insert(
            step,
            StepSchema::new(
                unit,
                unit,
                unit,
                vec![StepCaseFact::new(
                    case,
                    ConcreteOpKey::new(
                        op_key,
                        EffectFamilyKey::new("app.Ping".to_string(), Vec::new()),
                    ),
                    unit,
                    continuation,
                )],
            ),
        );

        let mut continuation_schemas = BTreeMap::new();
        continuation_schemas.insert(
            continuation,
            ContinuationSchema::new(unit, unit, step, unit),
        );

        let mut callables = BTreeMap::new();
        callables.insert(
            main_key.clone(),
            CallableEffectFacts::new(
                EffectRow::pure(),
                CallableAbiKind::EffectStep,
                Some(unit),
                Some(step),
                CaseSet::new(step, vec![case]),
                false,
                ImplPlan::CanonicalFull,
            ),
        );

        let mut bodies = BTreeMap::new();
        bodies.insert(
            main_key,
            BodyEffectFacts::new(BTreeMap::new(), BTreeMap::new()),
        );

        let facts = EffectFacts::from_parts(
            EffectSnapshotBinding::new(
                CanonicalMirQuerySurface::PassView,
                1,
                vec!["app.main".to_string()],
            ),
            step_schemas,
            continuation_schemas,
            callables,
            bodies,
        );

        assert!(facts.verify().is_ok());
        assert!(facts.dump().contains("callable=app.main abi=EffectStep"));
    }

    fn callable_key(path: &str) -> StableEffectInstanceKey {
        StableEffectInstanceKey::new(format!("instance({path})"), path)
    }

    fn unit_ty() -> scoopc_types::TypeId {
        let mut types = TypeStore::new();
        types.intern(TypeKind::Value(ValueTypeKind::Unit))
    }
}
