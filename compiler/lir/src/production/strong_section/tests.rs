use scoop_identity::{
    CborIdentityRecord, ConeCoordinate, ConeImageSupportRole, DefinitionAtomRole,
    DefinitionAtomSubkey, DigestNodeId, DigestNodeKey, DigestPatchIntentKey,
    DigestSemanticFieldRole, LinkageClass, ObjectDefinitionAtomId, ObjectDefinitionAtomKey,
    ObjectDefinitionPlanId, ObjectDefinitionPlanKey, PendingIdentityValidation,
    PersistentSymbolKey, PersistentSymbolRequest, PersistentSymbolRequestTable,
    StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_wire::{decode_canonical, encode};

use super::*;

mod complete_image;
mod digests;
use crate::{DigestNodeV1, LirTargetProfile};
pub(in crate::production) use complete_image::attach_image;
pub(in crate::production) use digests::without_image_input;

#[test]
fn strong_section_has_eight_closed_fields_and_round_trips() {
    let coordinate = ConeCoordinate::new("test", "strong-section", "0.0.0").unwrap();
    let (foundation, digests) = fixture(&coordinate);

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
        digests,
        registrations,
        EntryProductionSourceV1::Library,
        &[],
    )
    .unwrap();
    let encoded = encode(&section).unwrap();
    assert_eq!(encoded[0], 0xa8);

    let decoded: DecodedStrongProductionSectionV1 = decode_canonical(&encoded).unwrap();
    let mut identities = identities(&foundation);
    let validated = decoded
        .validate(
            coordinate,
            &[],
            LirTargetProfile::DARWIN_AARCH64,
            &foundation,
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
    for bytes in [vec![0xa7], vec![0xa9], vec![0xaa]] {
        assert!(decode_canonical::<DecodedStrongProductionSectionV1>(&bytes).is_err());
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
