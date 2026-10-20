use scoop_identity::{
    CborIdentityRecord, ConeCoordinate, ConeImageSupportRole, DefinitionAtomRole,
    DefinitionAtomSubkey, DigestNodeKey, LinkageClass, ObjectDefinitionAtomId,
    ObjectDefinitionAtomKey, ObjectDefinitionPlanId, ObjectDefinitionPlanKey, PersistentSymbolKey,
    PersistentSymbolRequest, PersistentSymbolRequestTable, StrongDefinitionEntity,
    StrongDefinitionRole,
};
use scoop_wire::{decode_canonical, encode};

use super::*;

mod complete_image;
use crate::DigestNodeV1;
pub(in crate::production) use complete_image::attach_image;

#[test]
fn strong_section_has_ten_closed_fields_and_round_trips() {
    let coordinate = ConeCoordinate::new("test", "strong-section", "0.0.0").unwrap();
    let (foundation, digests) = fixture(&coordinate);

    let registrations = StrongRegistrationProductionSurfaceV1::empty(
        crate::target_fixtures::DARWIN_AARCH64,
        &foundation,
        &digests,
    )
    .unwrap();
    let section = ConeProductionSectionV1::new(
        coordinate.clone(),
        &[],
        &foundation,
        digests,
        registrations,
        EntryProductionSourceV1::Library,
        &[],
        crate::canonical_callable::tests::fixture_definitions(&foundation),
        crate::CanonicalShapeAbisV1::new(Vec::new(), &foundation).unwrap(),
    )
    .unwrap();
    let encoded = encode(&section).unwrap();
    assert_eq!(encoded[0], 0xaa);

    let decoded: DecodedConeProductionSectionV1 = decode_canonical(&encoded).unwrap();
    assert_eq!(encode(&decoded).unwrap(), encoded);
}

#[test]
fn strong_section_reader_rejects_old_or_extended_top_level_shapes() {
    for bytes in [vec![0xa7], vec![0xa8], vec![0xa9], vec![0xab]] {
        assert!(decode_canonical::<DecodedConeProductionSectionV1>(&bytes).is_err());
    }
}

fn fixture(coordinate: &ConeCoordinate) -> (ConeLirFoundation, DigestFinalizationPlanV1) {
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
    let foundation = ConeLirFoundation::try_new(producer, canonical).unwrap();

    let image_key = DigestNodeKey::runtime_image(producer);

    let image = DigestNodeV1::new(image_key, Vec::new()).unwrap();
    let digests = DigestFinalizationPlanV1::new(vec![image], &foundation).unwrap();
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

mod v2;
