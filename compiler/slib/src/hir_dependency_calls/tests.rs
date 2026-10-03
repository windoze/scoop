use scoop_hir::{CanonicalHirDependencyCallSitesV1, HirDependencyCallSiteV1};
use scoop_identity::{Effect, ExactCallableSignature};

use super::*;

mod interface;
mod support;
use support::Fixture;

#[test]
fn every_actual_call_requires_its_own_strong_mir_root() {
    let fixture = Fixture::new();
    let interface = fixture.interface(vec![fixture.site(0, vec![])]);
    validate_calls(&interface, &fixture.strong, &fixture.bridge).unwrap();
    let absent = StrongCallableBridgeSurfaceV1::try_new(vec![]).unwrap();
    assert!(matches!(
        validate_calls(&interface, &absent, &fixture.bridge),
        Err(CrossConeMirClosureRelationError::CallRoot { .. })
    ));
}

#[test]
fn later_calls_to_one_target_cannot_hide_incompatible_logical_signatures() {
    let fixture = Fixture::new();
    let sites = vec![fixture.site(0, vec![]), fixture.site(1, vec![fixture.unit])];
    assert!(
        matches!(validate_calls(&fixture.interface(sites), &fixture.strong, &fixture.bridge), Err(CrossConeMirClosureRelationError::CallSignature { position, .. }) if position.expression_index == 1)
    );
}

#[test]
fn logical_receiver_and_each_unit_argument_survive_the_mir_join() {
    let mut fixture = Fixture::new();
    fixture.set_signature(ExactCallableSignature::new(
        Effect::Ordinary,
        Some(fixture.unit),
        vec![fixture.unit, fixture.unit],
        fixture.unit,
    ));
    let interface = fixture.interface(vec![fixture.site(0, vec![fixture.unit; 3])]);
    validate_calls(&interface, &fixture.strong, &fixture.bridge).unwrap();
    let erased = fixture.interface(vec![fixture.site(0, vec![fixture.unit; 2])]);
    assert!(matches!(
        validate_calls(&erased, &fixture.strong, &fixture.bridge),
        Err(CrossConeMirClosureRelationError::CallSignature { .. })
    ));
}

fn validate_calls(
    interface: &CrossConeHirInterfaceSectionV1,
    strong: &StrongCallableBridgeSurfaceV1,
    bridge: &CrossConeMirBridgeSectionV1,
) -> Result<(), CrossConeMirClosureRelationError> {
    let foundation = CanonicalMirFoundation::empty();
    let identities = scoop_identity::PendingIdentityValidation::new()
        .finish()
        .unwrap();
    validate_executable_hir_calls(interface, strong, bridge, None, &foundation, &identities)
}
