use super::*;
use scoop_wire::{decode_canonical, encode};

#[test]
fn release_policy_preserves_required_binders_and_rejects_invalid_scope() {
    let shape = NominalSourceShapeV1::Class(crate::NominalSourceFieldsV1::try_new(vec![]).unwrap());
    let binder = ReleaseValueBinderV1 { depth: 0, index: 0 };
    let policy = NominalReleasePolicyV1::SynchronousGcFree {
        requirements: vec![binder],
    };
    let restored: NominalReleasePolicyV1 = decode_canonical(&encode(&policy).unwrap()).unwrap();
    assert_eq!(restored, policy);
    restored
        .validate(&shape, NominalInheritanceModalityV1::Final, 1)
        .unwrap();
    for (requirements, position) in [
        (vec![ReleaseValueBinderV1 { depth: 1, index: 0 }], 0),
        (vec![ReleaseValueBinderV1 { depth: 0, index: 1 }], 0),
        (vec![binder, binder], 1),
    ] {
        assert_eq!(
            NominalReleasePolicyV1::SynchronousGcFree { requirements }.validate(
                &shape,
                NominalInheritanceModalityV1::Final,
                1
            ),
            Err(NominalInterfaceRecordBuildError::ReleaseConditionBinder { position })
        );
    }
    for (shape, modality) in [
        (shape, NominalInheritanceModalityV1::Open),
        (
            NominalSourceShapeV1::Interface,
            NominalInheritanceModalityV1::Interface,
        ),
    ] {
        assert_eq!(
            policy.validate(&shape, modality, 1),
            Err(NominalInterfaceRecordBuildError::ReleaseOwner)
        );
    }
}

#[test]
fn release_policy_reader_requires_the_tag_and_complete_product() {
    for bytes in [
        &[0xa1, 0, 3][..],
        &[0xa1, 0, 2][..],
        &[0xa2, 0, 1, 1, 0x80][..],
        &[0xa2, 0, 2, 1, 0x81, 0x81, 0][..],
    ] {
        assert!(decode_canonical::<NominalReleasePolicyV1>(bytes).is_err());
    }
}
