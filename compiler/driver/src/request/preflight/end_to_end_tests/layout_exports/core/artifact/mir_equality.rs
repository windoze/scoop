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
    foundation: &mir::CanonicalMirFoundation,
    strong: &mir::StrongCallableBridgeSurfaceV1,
    section: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
) {
    let replay = Replay {
        source,
        foundation,
        strong,
        section,
    };
    replay.validate(section.callables()).unwrap();
    if !name.starts_with("shared-equality-") {
        return;
    }
    dump::check(&replay, name);
    mutations::check(&replay);

    replay.validate(section.callables()).unwrap();
}

struct Replay<'a> {
    source: hir::CheckedSharedTypeFoundationV1<'a>,
    foundation: &'a mir::CanonicalMirFoundation,
    strong: &'a mir::StrongCallableBridgeSurfaceV1,
    section: &'a mir::CrossConeMirTypeBridgeSectionV1<'a>,
}

impl Replay<'_> {
    fn validate(&self, callables: &mir::CanonicalMirCallableBindingsV1) -> Result<(), Error> {
        scoop_slib::validate_shared_mir_equality(self.source, &[], self.strong, callables)
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
            .validate(&mut identities, self.foundation, self.section.types())
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
