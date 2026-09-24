use scoop_identity::{
    CborIdentityRecord, ConeCoordinate, ConeImageSupportRole, DefinitionAtomRole,
    DefinitionAtomSubkey, DigestNodeId, DigestNodeKey, DigestPatchIntentKey,
    DigestSemanticFieldRole, LinkageClass, ObjectDefinitionAtomId, ObjectDefinitionAtomKey,
    ObjectDefinitionPlanId, ObjectDefinitionPlanKey, PendingIdentityValidation,
    PersistentSymbolKey, PersistentSymbolRequest, PersistentSymbolRequestTable,
    StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

use super::*;

mod complete_image;
mod digests;
mod initialization;
use crate::{DigestNodeV1, LirTargetProfile, StrongExternalLirBridgeSurfaceV1};
pub(in crate::production) use complete_image::attach_image;
pub(in crate::production) use digests::without_image_input;

#[test]
fn strong_section_has_ten_closed_fields_and_rebuilds_from_authority() {
    let coordinate = ConeCoordinate::new("test", "strong-section", "0.0.0").unwrap();
    let (foundation, digests) = fixture(&coordinate);
    let external =
        StrongExternalLirBridgeSurfaceV1::try_new(coordinate.identity().unwrap(), Vec::new())
            .unwrap();
    let registrations = StrongRegistrationProductionSurfaceV1::empty(
        LirTargetProfile::DARWIN_AARCH64,
        &foundation,
        &digests,
    )
    .unwrap();
    let section = StrongProductionSectionV1::new(
        coordinate.clone(),
        &[],
        &foundation,
        external.clone(),
        digests,
        registrations,
        EntryProductionSourceV1::Library,
        &[],
        None,
    )
    .unwrap();
    let encoded = encode(&section).unwrap();
    assert_eq!(encoded[0], 0xaa);
    assert_initialization_field(&encoded);

    let decoded: DecodedStrongProductionSectionV1 =
        decode_canonical(&encoded, DecodeLimits::default()).unwrap();
    let mut identities = identities(&foundation);
    let validated = decoded
        .validate(
            coordinate,
            &[],
            LirTargetProfile::DARWIN_AARCH64,
            &foundation,
            &external,
            EntryProductionSourceV1::Library,
            &[],
            &mut identities,
        )
        .unwrap();

    assert_eq!(validated, section);
    assert_eq!(encode(&validated).unwrap(), encoded);
}

#[test]
fn strong_section_reader_rejects_old_or_extended_top_level_shapes() {
    for bytes in [vec![0xa9], vec![0xab]] {
        assert!(
            decode_canonical::<DecodedStrongProductionSectionV1>(&bytes, DecodeLimits::default())
                .is_err()
        );
    }
}

fn assert_initialization_field(encoded: &[u8]) {
    assert!(encoded.ends_with(&[11, 0x80, 12, 0x80]));
    let mut retired = vec![0xaa, 1, 0x80];
    retired.extend_from_slice(&encoded[1..encoded.len() - 2]);
    for error in [
        decode_canonical::<DecodedStrongProductionSectionV1>(&retired, DecodeLimits::default())
            .unwrap_err(),
        decode_canonical::<DecodedStrongProductionSectionV2>(&retired, DecodeLimits::default())
            .unwrap_err(),
    ] {
        assert_eq!(
            error.kind(),
            &scoop_wire::WireErrorKind::UnexpectedField {
                expected: 2,
                actual: 1,
            }
        );
    }
    for payload in [&[10, 0x80][..], &[10, 0xa1, 0, 1][..]] {
        let mut retired = encoded[..encoded.len() - 4].to_vec();
        retired.extend_from_slice(payload);
        retired.extend_from_slice(&[12, 0x80]);
        let v1 =
            decode_canonical::<DecodedStrongProductionSectionV1>(&retired, DecodeLimits::default())
                .unwrap_err();
        let v2 =
            decode_canonical::<DecodedStrongProductionSectionV2>(&retired, DecodeLimits::default())
                .unwrap_err();
        for error in [v1, v2] {
            assert_eq!(
                error.kind(),
                &scoop_wire::WireErrorKind::UnexpectedField {
                    expected: 11,
                    actual: 10,
                }
            );
        }
    }
    for payload in [&[11, 0xa1, 0, 1][..], &[11, 0x82][..]] {
        let mut invalid = encoded[..encoded.len() - 4].to_vec();
        invalid.extend_from_slice(payload);
        invalid.extend_from_slice(&[12, 0x80]);
        assert!(
            decode_canonical::<DecodedStrongProductionSectionV1>(&invalid, DecodeLimits::default())
                .is_err()
        );
        assert!(
            decode_canonical::<DecodedStrongProductionSectionV2>(&invalid, DecodeLimits::default())
                .is_err()
        );
    }
}

fn fixture(coordinate: &ConeCoordinate) -> (OdrFreeLirFoundation, StrongDigestFinalizationPlanV1) {
    let producer = coordinate.identity().unwrap();
    let definition = CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(
            producer,
            StrongDefinitionEntity::cone_image(producer),
            StrongDefinitionRole::ImageDescriptor,
        )
        .unwrap(),
    )
    .unwrap();
    let definition_id = definition.id();
    let atoms = image_atoms(definition_id);
    let symbol = PersistentSymbolRequest::new(
        PersistentSymbolKey::ImageDescriptor(producer),
        LinkageClass::ConeStrong,
    )
    .unwrap();
    let mut canonical = crate::CanonicalLirFoundation::empty();
    canonical.set_definition_plans(vec![definition]).unwrap();
    canonical.set_definition_atoms(atoms).unwrap();
    canonical.set_symbol_requests(PersistentSymbolRequestTable::new(vec![symbol]).unwrap());
    let foundation = OdrFreeLirFoundation::try_new(producer, canonical).unwrap();

    let image_key = DigestNodeKey::runtime_image(producer);
    let image_id = DigestNodeId::from_key(&image_key).unwrap();
    let patch = DigestPatchIntentKey::new(
        image_id,
        definition_id,
        DefinitionAtomRole::Primary,
        DigestSemanticFieldRole::RuntimeImage,
    );
    let image = DigestNodeV1::new(image_key, Vec::new(), vec![patch]).unwrap();
    let digests = StrongDigestFinalizationPlanV1::new(vec![image], &foundation).unwrap();
    (foundation, digests)
}

fn image_atoms(
    plan: ObjectDefinitionPlanId,
) -> Vec<CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>> {
    let mut keys = vec![ObjectDefinitionAtomKey::new(
        plan,
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    )];
    keys.extend(
        [
            (
                DefinitionAtomRole::AddressTakenConstant,
                ConeImageSupportRole::CoordinateGroup,
            ),
            (
                DefinitionAtomRole::AddressTakenConstant,
                ConeImageSupportRole::CoordinateName,
            ),
            (
                DefinitionAtomRole::AddressTakenConstant,
                ConeImageSupportRole::CoordinateVersion,
            ),
            (
                DefinitionAtomRole::RuntimeRecord,
                ConeImageSupportRole::Dependencies,
            ),
            (
                DefinitionAtomRole::RuntimeRecord,
                ConeImageSupportRole::StaticStorages,
            ),
            (
                DefinitionAtomRole::RuntimeRecord,
                ConeImageSupportRole::ImmortalObjects,
            ),
            (
                DefinitionAtomRole::RuntimeRecord,
                ConeImageSupportRole::InitializationUnits,
            ),
            (
                DefinitionAtomRole::RuntimeRecord,
                ConeImageSupportRole::TypeRegistrations,
            ),
            (
                DefinitionAtomRole::RuntimeRecord,
                ConeImageSupportRole::Safepoints,
            ),
            (
                DefinitionAtomRole::RuntimeRecord,
                ConeImageSupportRole::Callables,
            ),
            (
                DefinitionAtomRole::AddressTakenConstant,
                ConeImageSupportRole::ArrayBoundsMessage,
            ),
            (
                DefinitionAtomRole::AddressTakenConstant,
                ConeImageSupportRole::ArraySizeOverflowMessage,
            ),
        ]
        .map(|(role, support)| {
            ObjectDefinitionAtomKey::new(
                plan,
                role,
                DefinitionAtomSubkey::ConeImageSupport(support),
            )
        }),
    );
    keys.into_iter()
        .map(|key| CborIdentityRecord::from_key(key).unwrap())
        .collect()
}

fn identities(foundation: &OdrFreeLirFoundation) -> scoop_identity::ValidatedIdentityGraph {
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(foundation.producer()).unwrap();
    for definition in foundation.definition_plans() {
        pending.register_authority(definition.id()).unwrap();
    }
    pending.finish().unwrap()
}

mod v2;
