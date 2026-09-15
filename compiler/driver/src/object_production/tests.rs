use scoop_identity::{
    CborIdentityRecord, ConeIdentity, DefinitionAtomRole, DefinitionAtomSubkey,
    ObjectDefinitionAtomKey, ObjectDefinitionPlanId, ObjectDefinitionPlanKey,
    StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{CanonicalLirFoundation, OdrFreeLirFoundation};

use super::*;

#[test]
fn binds_object_bytes_to_member_ids_only_through_their_canonical_units() {
    let image = definition(
        StrongDefinitionEntity::cone_image(ConeIdentity::CORE),
        StrongDefinitionRole::ImageDescriptor,
    );
    let entry = definition(
        StrongDefinitionEntity::root_entry(ConeIdentity::CORE),
        StrongDefinitionRole::RootEntryDescriptor,
    );
    let foundation = foundation(&[image.clone(), entry.clone()]);
    let producer_units =
        StrongProducerUnitPartitionV1::from_odr_free_foundation(&foundation).unwrap();
    let sources = vec![
        UnboundScoopLirObject {
            units: vec![entry.id()],
            bytes: b"entry".to_vec(),
            digest_patches: Vec::new(),
        },
        UnboundScoopLirObject {
            units: vec![image.id()],
            bytes: b"image".to_vec(),
            digest_patches: Vec::new(),
        },
    ];

    let planned = plan_objects(&producer_units, sources, Vec::new()).unwrap();

    assert_eq!(planned.members.len(), 2);
    assert!(
        planned
            .members
            .windows(2)
            .all(|pair| pair[0].plan().member_id() < pair[1].plan().member_id())
    );
    for member in &planned.members {
        let unit = member.plan().units().units()[0];
        let expected = if unit == image.id() {
            b"image".as_slice()
        } else if unit == entry.id() {
            b"entry".as_slice()
        } else {
            panic!("unexpected planned definition {unit}");
        };
        assert_eq!(member.bytes(), expected);
        assert_eq!(
            planned.member_plan.member_for_definition(unit),
            Some(member.plan().member_id())
        );
    }
}

#[test]
fn rejects_one_physical_object_whose_units_resolve_to_different_members() {
    let image = definition(
        StrongDefinitionEntity::cone_image(ConeIdentity::CORE),
        StrongDefinitionRole::ImageDescriptor,
    );
    let entry = definition(
        StrongDefinitionEntity::root_entry(ConeIdentity::CORE),
        StrongDefinitionRole::RootEntryDescriptor,
    );
    let foundation = foundation(&[image.clone(), entry.clone()]);
    let producer_units =
        StrongProducerUnitPartitionV1::from_odr_free_foundation(&foundation).unwrap();
    let member_plan = PlannedLinkObjectMemberSetV1::new(
        &producer_units,
        vec![
            CanonicalScoopLirObjectUnitSetV1::new(vec![image.id()]).unwrap(),
            CanonicalScoopLirObjectUnitSetV1::new(vec![entry.id()]).unwrap(),
        ],
        Vec::new(),
    )
    .unwrap();

    assert!(matches!(
        member_for_units(&member_plan, &[image.id(), entry.id()]),
        Err(ScoopLirObjectProductionError::SplitObjectUnits(_))
    ));
}

fn definition(
    entity: StrongDefinitionEntity,
    role: StrongDefinitionRole,
) -> CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey> {
    CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(ConeIdentity::CORE, entity, role).unwrap(),
    )
    .unwrap()
}

fn foundation(
    definitions: &[CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey>],
) -> OdrFreeLirFoundation {
    let mut canonical = CanonicalLirFoundation::empty();
    canonical
        .set_definition_plans(definitions.to_vec())
        .unwrap();
    canonical
        .set_definition_atoms(
            definitions
                .iter()
                .map(|definition| {
                    CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
                        definition.id(),
                        DefinitionAtomRole::Primary,
                        DefinitionAtomSubkey::Singleton,
                    ))
                    .unwrap()
                })
                .collect(),
        )
        .unwrap();
    OdrFreeLirFoundation::try_new(ConeIdentity::CORE, canonical).unwrap()
}
