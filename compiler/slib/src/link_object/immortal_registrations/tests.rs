use super::*;
use crate::link_object::RegistrationAbiV1;
use crate::link_object::stackmap_normalization::verification::tests::support::{
    Corruption, Fixture,
};

#[test]
fn computes_immortal_registration_abis_from_the_plan() {
    let fixture = Fixture::new(Corruption::None);
    let fingerprints = compute_strong_immortal_object_fingerprints_v1(
        fixture.immortal_registration_plan.clone(),
        &fixture.canonical_shapes,
    )
    .unwrap();
    assert_eq!(fingerprints.fingerprints().len(), 1);
    let actual = fingerprints.fingerprints()[0];
    let plan = fixture.immortal_registration_plan.registrations()[0];
    assert_eq!(actual.object(), plan.object());
    assert_eq!(actual.definition(), ImmortalObjectDefinitionAbiV1::Strong);
    assert_eq!(
        actual.registration(),
        RegistrationAbiV1::from_owner(
            plan.definition_owner(),
            scoop_lir::RegistrationTableV1::ImmortalObject,
        )
        .unwrap()
    );
}
