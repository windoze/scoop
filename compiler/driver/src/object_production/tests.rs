use scoop_identity::{
    CborIdentityRecord, ConeIdentity, DefinitionAtomRole, DefinitionAtomSubkey,
    GeneratedBridgeAtomId, GeneratedBridgeAtomKey, GeneratedBridgeAtomRoleKey,
    GeneratedBridgeUnitId, GeneratedBridgeUnitKey, NativeExternalContract,
    NativeExternalContractFingerprint, NativeExternalSymbolKey, NativeLibraryBinding,
    ObjectDefinitionAtomKey, ObjectDefinitionPlanId, ObjectDefinitionPlanKey,
    PersistentNativeExternalSymbolId, SourceNativeSymbol, StrongDefinitionEntity,
    StrongDefinitionRole,
};
use scoop_lir::{CanonicalLirFoundation, ConeLirFoundation};

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
    let producer_units = ProducerUnitPartitionV1::from_foundation(&foundation).unwrap();
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

    assert_eq!(planned.scoop_lir_members.len(), 2);
    assert!(
        planned
            .scoop_lir_members
            .windows(2)
            .all(|pair| pair[0].plan.member_id() < pair[1].plan.member_id())
    );
    for member in &planned.scoop_lir_members {
        let unit = member.plan.units().units()[0];
        let expected = if unit == image.id() {
            b"image".as_slice()
        } else if unit == entry.id() {
            b"entry".as_slice()
        } else {
            panic!("unexpected planned definition {unit}");
        };
        assert_eq!(member.bytes, expected);
        assert_eq!(
            planned.member_plan.member_for_definition(unit),
            Some(member.plan.member_id())
        );
    }
}

#[test]
fn binds_generated_c_bytes_from_the_actual_singleton_unit_set() {
    let image = definition(
        StrongDefinitionEntity::cone_image(ConeIdentity::CORE),
        StrongDefinitionRole::ImageDescriptor,
    );
    let unit = bridge_unit();
    let bridge_atom = bridge_primary_atom(unit.id());
    let bridge_definition = definition(
        StrongDefinitionEntity::generated_bridge_atom(bridge_atom.key()).unwrap(),
        StrongDefinitionRole::GeneratedBridge,
    );
    let foundation = foundation_with_bridges(
        &[image.clone(), bridge_definition],
        vec![unit.clone()],
        vec![bridge_atom],
    );
    let producer_units = ProducerUnitPartitionV1::from_foundation(&foundation).unwrap();

    let planned = plan_objects(
        &producer_units,
        vec![UnboundScoopLirObject {
            units: vec![image.id()],
            bytes: b"image".to_vec(),
            digest_patches: Vec::new(),
        }],
        vec![UnboundGeneratedCBridgeObject {
            unit: unit.id(),
            bytes: b"bridge".to_vec(),
        }],
    )
    .unwrap();

    let [bridge] = planned.generated_c_bridge_members.as_slice() else {
        panic!("one generated bridge unit must produce one planned member")
    };
    assert_eq!(bridge.plan.units().units(), &[unit.id()]);
    assert_eq!(bridge.bytes, b"bridge");
    assert_eq!(
        planned
            .member_plan
            .member_for_generated_bridge_unit(unit.id()),
        Some(bridge.plan.member_id())
    );
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
    let producer_units = ProducerUnitPartitionV1::from_foundation(&foundation).unwrap();
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
        scoop_member_for_units(&member_plan, &[image.id(), entry.id()]),
        Err(BuiltinObjectProductionError::SplitObjectUnits(_))
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
) -> ConeLirFoundation {
    foundation_with_bridges(definitions, Vec::new(), Vec::new())
}

fn foundation_with_bridges(
    definitions: &[CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey>],
    bridge_units: Vec<CborIdentityRecord<GeneratedBridgeUnitId, GeneratedBridgeUnitKey>>,
    bridge_atoms: Vec<CborIdentityRecord<GeneratedBridgeAtomId, GeneratedBridgeAtomKey>>,
) -> ConeLirFoundation {
    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_bridge_units(bridge_units).unwrap();
    canonical.set_bridge_atoms(bridge_atoms).unwrap();
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
    ConeLirFoundation::try_new(ConeIdentity::CORE, canonical).unwrap()
}

fn bridge_unit() -> CborIdentityRecord<GeneratedBridgeUnitId, GeneratedBridgeUnitKey> {
    let symbol =
        NativeExternalSymbolKey::darwin_macho_external(&SourceNativeSymbol::new("native").unwrap())
            .unwrap();
    let contract = NativeExternalContract::c_function(
        NativeLibraryBinding::DefaultNativeNamespace,
        scoop_identity::CanonicalCAbiFunctionSignature::cdecl(
            Vec::new(),
            scoop_identity::CanonicalCAbiReturn::Void,
        ),
    );
    let fingerprint = NativeExternalContractFingerprint::from_symbol_and_contract(
        PersistentNativeExternalSymbolId::from_key(&symbol).unwrap(),
        &contract,
    )
    .unwrap();
    CborIdentityRecord::from_key(GeneratedBridgeUnitKey::OutboundFunction(fingerprint)).unwrap()
}

fn bridge_primary_atom(
    unit: GeneratedBridgeUnitId,
) -> CborIdentityRecord<GeneratedBridgeAtomId, GeneratedBridgeAtomKey> {
    CborIdentityRecord::from_key(GeneratedBridgeAtomKey::new(
        ConeIdentity::CORE,
        GeneratedBridgeAtomRoleKey::PrimaryEntry { unit },
    ))
    .unwrap()
}
