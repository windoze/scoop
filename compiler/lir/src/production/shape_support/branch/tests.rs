use scoop_identity::{ConeIdentity, DigestNodeKey, ValidatedIdentityGraph};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

use super::*;
use crate::{
    CanonicalLirFoundation, DigestNodeV1, StrongDigestFinalizationPlanV1,
    StrongRegistrationIdentitySurfaceV1,
};

#[test]
fn branches_have_fixed_wire_and_follow_the_producer() {
    let (core, core_registrations) = foundation(ConeIdentity::CORE);
    assert_eq!(
        encode(
            &CoreShapeSupportPlanV1::new(std::iter::empty(), &core, &core_registrations,).unwrap()
        )
        .unwrap(),
        b"\xa2\x00\x02\x01\x80"
    );

    let (ordinary, ordinary_registrations) = foundation(ConeIdentity::SINGLE_FILE);
    assert_eq!(
        CoreShapeSupportPlanV1::new(std::iter::empty(), &ordinary, &ordinary_registrations,)
            .unwrap(),
        CoreShapeSupportPlanV1::NotCore
    );
    assert_eq!(
        encode(&CoreShapeSupportPlanV1::NotCore).unwrap(),
        b"\xa1\x00\x01"
    );
}

#[test]
fn reader_rejects_the_wrong_branch_without_promoting_it() {
    let (core, registrations) = foundation(ConeIdentity::CORE);
    let decoded: DecodedCoreShapeSupportPlanV1 = decode_canonical(
        &encode(&CoreShapeSupportPlanV1::NotCore).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
    let mut identities = empty_identity_graph();
    assert!(matches!(
        decoded.validate(std::iter::empty(), &mut identities, &core, &registrations,),
        Err(CoreShapeSupportPlanValidationError::BranchMismatch)
    ));
}

#[test]
fn decoder_rejects_unknown_and_non_closed_sum_shapes() {
    for bytes in [b"\xa1\x00\x03".as_slice(), b"\xa2\x00\x01\x01\x80"] {
        assert!(
            decode_canonical::<DecodedCoreShapeSupportPlanV1>(bytes, DecodeLimits::default())
                .is_err()
        );
    }
}

fn foundation(
    producer: ConeIdentity,
) -> (OdrFreeLirFoundation, StrongRegistrationIdentitySurfaceV1) {
    let foundation =
        OdrFreeLirFoundation::try_new(producer, CanonicalLirFoundation::empty()).unwrap();
    let digests = StrongDigestFinalizationPlanV1::new(
        vec![
            DigestNodeV1::new(
                DigestNodeKey::runtime_image(producer),
                Vec::new(),
                Vec::new(),
            )
            .unwrap(),
        ],
        &foundation,
    )
    .unwrap();
    let registrations =
        StrongRegistrationIdentitySurfaceV1::from_foundation(&foundation, &digests).unwrap();
    (foundation, registrations)
}

fn empty_identity_graph() -> ValidatedIdentityGraph {
    scoop_identity::PendingIdentityValidation::new()
        .finish()
        .unwrap()
}
