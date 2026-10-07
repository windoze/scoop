use super::*;
use crate::link_object::stackmap_normalization::verification::tests::support::semantic;
use scoop_lir::{
    RegistrationIdentitySurfaceV1, StrongTypeDescriptorSemanticPlanSetV2,
    StrongTypeRegistrationPlanSetV2,
};

fn plans(corruption: Corruption) -> StrongTypeRegistrationPlanSetV2 {
    let input = semantic::inputs(corruption);

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
fn nonempty_v2_type_proofs_preserve_descriptor_layout_and_registration_digests() {
    let fixture = Fixture::new(Corruption::None);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let v1 = verify_strong_type_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.type_registration_plan.clone(),
        &objects,
    )
    .unwrap();
    let v2 = verify_strong_type_registrations_v2(
        fixture.verified_patch_sites(),
        plans(Corruption::None),
        &objects,
    )
    .unwrap();
    assert!(!v2.registrations().is_empty());
    assert_eq!(v1.registrations(), v2.registrations());
    let v1 = compute_strong_type_fingerprints_v1(
        v1,
        &fixture.canonical_shapes,
        fixture.undefined_requirements(),
        &objects,
    )
    .unwrap();
    let v2 = compute_strong_type_fingerprints_v2(
        v2,
        &fixture.canonical_shapes,
        fixture.undefined_requirements(),
        &objects,
    )
    .unwrap();
    assert_eq!(v1.fingerprints(), v2.fingerprints());
}

#[test]
fn v2_rechecks_registration_relocations_diagnostic_bytes_and_patch_sites() {
    for corruption in [
        Corruption::TypeRegistrationMagic,
        Corruption::TypeDescriptorRelocationTarget,
        Corruption::TypeDescriptorDiagnosticBytes,
        Corruption::TypeDescriptorDiagnosticRelocationTarget,
    ] {
        let fixture = Fixture::new(corruption);
        let objects = [ScoopLirObjectCandidateV1::new(
            fixture.member,
            &fixture.object_bytes,
        )];
        let old = verify_strong_type_registrations_v1(
            fixture.verified_patch_sites(),
            fixture.type_registration_plan.clone(),
            &objects,
        )
        .unwrap_err();
        let new = verify_strong_type_registrations_v2(
            fixture.verified_patch_sites(),
            plans(corruption),
            &objects,
        )
        .unwrap_err();
        assert_eq!(old, new);
    }
}

#[test]
fn v2_rejects_descriptor_shape_corruption_before_fingerprinting() {
    let fixture = Fixture::new(Corruption::TypeDescriptorScalar);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let registrations = verify_strong_type_registrations_v2(
        fixture.verified_patch_sites(),
        plans(Corruption::TypeDescriptorScalar),
        &objects,
    )
    .unwrap();
    assert!(matches!(
        compute_strong_type_fingerprints_v2(
            registrations,
            &fixture.canonical_shapes,
            fixture.undefined_requirements(),
            &objects
        ),
        Err(StrongTypeFingerprintError::Dependencies(
            StrongTypeDependencyFingerprintError::DescriptorByteMismatch {
                offset_within_descriptor: 16,
                ..
            }
        ))
    ));
}
