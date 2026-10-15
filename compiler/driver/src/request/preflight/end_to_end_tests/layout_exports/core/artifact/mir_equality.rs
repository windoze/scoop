use super::*;
use scoop_identity::{CallableOwner, GeneratedCallableKey, PersistentGeneratedCallableId};

mod applications;

pub(super) fn check(
    name: &str,
    source: hir::SharedTypeMetadataV1<'_>,
    strong: &mir::StrongCallableBridgeSurfaceV1,
    section: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
) {
    let replay = Fixture {
        source,
        strong,
        section,
    };
    if !name.starts_with("shared-equality-") {
        return;
    }
    applications::check(&replay, name);
}

struct Fixture<'a> {
    source: hir::SharedTypeMetadataV1<'a>,
    strong: &'a mir::StrongCallableBridgeSurfaceV1,
    section: &'a mir::CrossConeMirTypeBridgeSectionV1<'a>,
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
