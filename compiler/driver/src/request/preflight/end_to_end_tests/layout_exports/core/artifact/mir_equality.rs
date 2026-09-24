use super::*;
use scoop_identity::{
    CallableOwner, GeneratedCallableKey, PersistentGeneratedCallableId,
    StrongCallableDefinitionOwner,
};
use scoop_slib::SharedMirEqualityValidationError as Error;

mod dump;
mod mutations;

pub(super) fn check(
    name: &str,
    source: hir::CheckedSharedTypeFoundationV1<'_>,
    foundation: &mir::OdrFreeMirFoundation,
    strong: &mir::StrongCallableBridgeSurfaceV1,
    section: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
) {
    let replay = Replay {
        source,
        foundation,
        strong,
        section,
    };
    replay.validate(section.callables(), &mut meter()).unwrap();
    if !name.starts_with("shared-equality-") {
        return;
    }
    dump::check(&replay, name);
    mutations::check(&replay);
    let mut measured = meter();
    replay.validate(section.callables(), &mut measured).unwrap();
    let mut shared = scoop_wire::BudgetMeter::new(DecodeLimits {
        validation_work_units: measured.usage().validation_work_units,
        ..DecodeLimits::default()
    });
    replay.validate(section.callables(), &mut shared).unwrap();
    assert!(matches!(
        replay.validate(section.callables(), &mut shared),
        Err(Error::Resource(_))
    ));
    for limits in [
        DecodeLimits {
            validation_work_units: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            logical_heap_bytes: 0,
            ..DecodeLimits::default()
        },
    ] {
        assert!(matches!(
            replay.validate(
                section.callables(),
                &mut scoop_wire::BudgetMeter::new(limits)
            ),
            Err(Error::Resource(_))
        ));
    }
}

struct Replay<'a> {
    source: hir::CheckedSharedTypeFoundationV1<'a>,
    foundation: &'a mir::OdrFreeMirFoundation,
    strong: &'a mir::StrongCallableBridgeSurfaceV1,
    section: &'a mir::CrossConeMirTypeBridgeSectionV1<'a>,
}

impl Replay<'_> {
    fn validate(
        &self,
        callables: &mir::CanonicalMirCallableBindingsV1,
        meter: &mut scoop_wire::BudgetMeter,
    ) -> Result<(), Error> {
        scoop_slib::validate_shared_mir_equality(self.source, &[], self.strong, callables, meter)
    }

    fn resolve(
        &self,
        records: Vec<mir::ParamFreeMirCallableBindingV1>,
    ) -> mir::CanonicalMirCallableBindingsV1 {
        let bindings = mir::CanonicalMirCallableBindingsV1::try_new(records).unwrap();
        let decoded: mir::DecodedCanonicalMirCallableBindingsV1 = decoded(&bindings);
        let mut pending = scoop_identity::PendingIdentityValidation::new();
        pending
            .register_external_graph_authorities(self.source.metadata().identities)
            .unwrap();
        let mut identities = pending.finish().unwrap();
        decoded
            .validate(
                &mut identities,
                self.foundation,
                self.section.types(),
                &mut meter(),
            )
            .expect("the mutation preserves MIR-local identity, signature and type validation")
    }
}

fn callable(binding: &mir::ParamFreeMirCallableBindingV1) -> Option<PersistentGeneratedCallableId> {
    match binding.origin() {
        mir::MirCallableOriginV1::Generated {
            callable,
            role: GeneratedCallableKey::DerivedEquality { .. },
        } => Some(*callable),
        _ => None,
    }
}
