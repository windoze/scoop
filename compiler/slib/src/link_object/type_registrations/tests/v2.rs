use super::*;
use crate::link_object::stackmap_normalization::verification::tests::support::semantic;
use scoop_lir::{
    RegistrationIdentitySurfaceV1, StrongTypeDescriptorSemanticPlanSetV2,
    StrongTypeRegistrationPlanSetV2,
};

fn plans() -> StrongTypeRegistrationPlanSetV2 {
    let input = semantic::inputs();

    let semantics = StrongTypeDescriptorSemanticPlanSetV2::from_module(&input.module).unwrap();
    let identities = RegistrationIdentitySurfaceV1::from_foundation(&input.foundation).unwrap();
    StrongTypeRegistrationPlanSetV2::new(
        input.module.meta.target_profile,
        &input.foundation,
        &identities,
        &semantics,
        &input.digest_plan,
    )
    .unwrap()
}

#[test]
fn v2_type_data_preserves_descriptor_layout_and_registration_digests() {
    let fixture = Fixture::new(Corruption::None);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let v1 = bind_type_registrations_v1(
        fixture.object_data(),
        fixture.type_registration_plan.clone(),
    );
    let v2 = bind_type_registrations_v1(fixture.object_data(), plans());
    assert!(!v2.plan().registrations().is_empty());
    let v1 = compute_strong_type_fingerprints_v1(v1, &fixture.canonical_shapes, &objects).unwrap();
    let v2 = compute_strong_type_fingerprints_v2(v2, &fixture.canonical_shapes, &objects).unwrap();
    assert_eq!(v1.fingerprints(), v2.fingerprints());
}
