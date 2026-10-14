use super::*;

#[test]
fn registration_lookup_keeps_count_and_target_owner_checks() {
    for (corruption, kind) in [
        (
            Corruption::RegistrationCount,
            ConeImageRelocationFailureV1::Count,
        ),
        (
            Corruption::RegistrationTarget,
            ConeImageRelocationFailureV1::TargetOwner,
        ),
    ] {
        let fixture = fixture(corruption, true);
        assert_eq!(
            verify_cone_image_v1(
                fixture.patch_sites,
                fixture.plan,
                &[ScoopLirObjectCandidateV1::new(
                    fixture.member,
                    &fixture.bytes
                )],
            ),
            Err(ConeImageValidationError::RelocationMismatch {
                role: ConeImageAtomRoleV1::TypeRegistrations,
                index: 0,
                kind,
            }),
        );
    }
}
