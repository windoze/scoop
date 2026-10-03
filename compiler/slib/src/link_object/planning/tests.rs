use scoop_identity::{
    CborIdentityRecord, ConeIdentity, DefinitionAtomRole, DefinitionAtomSubkey,
    GeneratedBridgeAtomKey, GeneratedBridgeAtomRoleKey, GeneratedBridgeUnitId,
    GeneratedBridgeUnitKey, NativeExternalContract, NativeExternalContractFingerprint,
    NativeExternalSymbolKey, ObjectDefinitionAtomId, ObjectDefinitionAtomKey,
    ObjectDefinitionPlanId, ObjectDefinitionPlanKey, PersistentNativeExternalSymbolId,
    SourceNativeSymbol, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{CanonicalLirFoundation, ConeLirFoundation, ProducerUnitPartitionV1};

use super::*;

#[test]
fn assigns_every_plan_and_bridge_unit_to_exactly_one_member() {
    let fixture = fixture(true);
    let partition = ProducerUnitPartitionV1::from_foundation(&fixture.foundation).unwrap();
    let plan = PlannedLinkObjectMemberSetV1::new(
        &partition,
        vec![
            CanonicalScoopLirObjectUnitSetV1::new(vec![fixture.lir_plans[1]]).unwrap(),
            CanonicalScoopLirObjectUnitSetV1::new(vec![fixture.lir_plans[0]]).unwrap(),
        ],
        vec![CanonicalGeneratedBridgeObjectUnitSetV1::new(vec![fixture.bridge_unit]).unwrap()],
    )
    .unwrap();

    assert_eq!(plan.producer(), fixture.foundation.producer());
    assert_eq!(plan.scoop_lir_members().len(), 2);
    assert_eq!(plan.generated_bridge_members().len(), 1);
    assert_eq!(plan.definition_assignments().len(), 3);
    for lir_plan in fixture.lir_plans {
        assert!(plan.member_for_definition(lir_plan).is_some());
    }
    let bridge_member = plan
        .member_for_generated_bridge_unit(fixture.bridge_unit)
        .unwrap();
    assert_eq!(
        plan.member_for_definition(fixture.bridge_plan),
        Some(bridge_member)
    );
    assert!(
        plan.definition_assignments()
            .windows(2)
            .all(|pair| pair[0].definition_plan() < pair[1].definition_plan())
    );
}

#[test]
fn rejects_overlapping_and_incomplete_shards() {
    let fixture = fixture(true);
    let partition = ProducerUnitPartitionV1::from_foundation(&fixture.foundation).unwrap();

    let duplicate = PlannedLinkObjectMemberSetV1::new(
        &partition,
        vec![
            CanonicalScoopLirObjectUnitSetV1::new(fixture.lir_plans.to_vec()).unwrap(),
            CanonicalScoopLirObjectUnitSetV1::new(vec![fixture.lir_plans[0]]).unwrap(),
        ],
        vec![CanonicalGeneratedBridgeObjectUnitSetV1::new(vec![fixture.bridge_unit]).unwrap()],
    );
    assert!(matches!(
        duplicate,
        Err(LinkObjectMemberSetPlanError::DuplicateScoopLirDefinition(_))
    ));

    let missing_bridge = PlannedLinkObjectMemberSetV1::new(
        &partition,
        vec![CanonicalScoopLirObjectUnitSetV1::new(fixture.lir_plans.to_vec()).unwrap()],
        Vec::new(),
    );
    assert_eq!(
        missing_bridge.unwrap_err().to_string(),
        LinkObjectMemberSetPlanError::MissingGeneratedBridgeUnit(fixture.bridge_unit).to_string()
    );
}

#[test]
fn generated_members_are_absent_when_the_partition_has_no_bridge_units() {
    let fixture = fixture(false);
    let partition = ProducerUnitPartitionV1::from_foundation(&fixture.foundation).unwrap();
    let plan = PlannedLinkObjectMemberSetV1::new(
        &partition,
        vec![CanonicalScoopLirObjectUnitSetV1::new(fixture.lir_plans.to_vec()).unwrap()],
        Vec::new(),
    )
    .unwrap();

    assert!(plan.generated_bridge_members().is_empty());
    assert!(plan.generated_bridge_unit_assignments().is_empty());
}

struct Fixture {
    foundation: ConeLirFoundation,
    lir_plans: [ObjectDefinitionPlanId; 2],
    bridge_unit: GeneratedBridgeUnitId,
    bridge_plan: ObjectDefinitionPlanId,
}

fn fixture(include_bridge: bool) -> Fixture {
    let lir_records = [
        CborIdentityRecord::from_key(
            ObjectDefinitionPlanKey::strong(
                ConeIdentity::CORE,
                StrongDefinitionEntity::cone_image(ConeIdentity::CORE),
                StrongDefinitionRole::ImageDescriptor,
            )
            .unwrap(),
        )
        .unwrap(),
        CborIdentityRecord::from_key(
            ObjectDefinitionPlanKey::strong(
                ConeIdentity::CORE,
                StrongDefinitionEntity::root_entry(ConeIdentity::CORE),
                StrongDefinitionRole::RootEntryDescriptor,
            )
            .unwrap(),
        )
        .unwrap(),
    ];
    let lir_plans = [lir_records[0].id(), lir_records[1].id()];

    let native_symbol = NativeExternalSymbolKey::darwin_macho_external(
        &SourceNativeSymbol::new("native_bridge").unwrap(),
    )
    .unwrap();
    let native_contract = NativeExternalContract::c_function(
        scoop_identity::NativeLibraryBinding::DefaultNativeNamespace,
        scoop_identity::CanonicalCAbiFunctionSignature::cdecl(
            Vec::new(),
            scoop_identity::CanonicalCAbiReturn::Void,
        ),
    );
    let fingerprint = NativeExternalContractFingerprint::from_symbol_and_contract(
        PersistentNativeExternalSymbolId::from_key(&native_symbol).unwrap(),
        &native_contract,
    )
    .unwrap();
    let bridge_unit =
        CborIdentityRecord::from_key(GeneratedBridgeUnitKey::OutboundFunction(fingerprint))
            .unwrap();
    let bridge_atom = CborIdentityRecord::from_key(GeneratedBridgeAtomKey::new(
        ConeIdentity::CORE,
        GeneratedBridgeAtomRoleKey::PrimaryEntry {
            unit: bridge_unit.id(),
        },
    ))
    .unwrap();
    let bridge_definition = CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(
            ConeIdentity::CORE,
            StrongDefinitionEntity::generated_bridge_atom(bridge_atom.key()).unwrap(),
            StrongDefinitionRole::GeneratedBridge,
        )
        .unwrap(),
    )
    .unwrap();

    let mut definitions = lir_records.to_vec();
    let mut atoms = definitions
        .iter()
        .map(|record| definition_atom(record.id()))
        .collect::<Vec<_>>();
    let mut foundation = CanonicalLirFoundation::empty();
    if include_bridge {
        definitions.push(bridge_definition.clone());
        atoms.push(definition_atom(bridge_definition.id()));
        foundation
            .set_bridge_units(vec![bridge_unit.clone()])
            .unwrap();
        foundation.set_bridge_atoms(vec![bridge_atom]).unwrap();
    }
    foundation.set_definition_plans(definitions).unwrap();
    foundation.set_definition_atoms(atoms).unwrap();

    Fixture {
        foundation: ConeLirFoundation::try_new(ConeIdentity::CORE, foundation).unwrap(),
        lir_plans,
        bridge_unit: bridge_unit.id(),
        bridge_plan: bridge_definition.id(),
    }
}

fn definition_atom(
    plan: ObjectDefinitionPlanId,
) -> CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey> {
    CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        plan,
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap()
}
