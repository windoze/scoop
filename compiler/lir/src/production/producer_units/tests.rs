use scoop_identity::{
    CborIdentityRecord, ConeIdentity, DefinitionAtomRole, DefinitionAtomSubkey,
    GeneratedBridgeAtomKey, GeneratedBridgeAtomRoleKey, GeneratedBridgeUnitKey,
    NativeExternalContract, NativeExternalContractFingerprint, NativeExternalSymbolKey,
    ObjectDefinitionAtomKey, ObjectDefinitionPlanKey, PersistentNativeExternalSymbolId,
    SourceNativeSymbol, StrongDefinitionEntity, StrongDefinitionRole,
};

use super::*;
use crate::{CanonicalLirFoundation, ConeLirFoundation};

#[test]
fn partitions_every_definition_by_its_actual_producer() {
    let (foundation, lir_plan, bridge_plan, bridge_unit) = fixture(true);

    let partition = StrongProducerUnitPartitionV1::from_foundation(&foundation).unwrap();

    assert_eq!(partition.producer(), ConeIdentity::CORE);
    assert_eq!(partition.scoop_lir_definition_plans(), &[lir_plan]);
    assert_eq!(partition.generated_bridge_units().len(), 1);
    assert_eq!(partition.generated_bridge_units()[0].unit(), bridge_unit);
    assert_eq!(
        partition.generated_bridge_units()[0].definition_plans(),
        &[bridge_plan]
    );
    assert_eq!(partition.definition_plan_count(), 2);
}

#[test]
fn bridge_only_foundation_has_an_explicit_empty_lir_partition() {
    let (foundation, _, bridge_plan, bridge_unit) = fixture(false);

    let partition = StrongProducerUnitPartitionV1::from_foundation(&foundation).unwrap();

    assert!(partition.scoop_lir_definition_plans().is_empty());
    assert_eq!(partition.generated_bridge_units().len(), 1);
    assert_eq!(partition.generated_bridge_units()[0].unit(), bridge_unit);
    assert_eq!(
        partition.generated_bridge_units()[0].definition_plans(),
        &[bridge_plan]
    );
    assert_eq!(partition.definition_plan_count(), 1);
}

fn fixture(
    include_lir_plan: bool,
) -> (
    ConeLirFoundation,
    scoop_identity::ObjectDefinitionPlanId,
    scoop_identity::ObjectDefinitionPlanId,
    GeneratedBridgeUnitId,
) {
    let native_symbol =
        NativeExternalSymbolKey::darwin_macho_external(&SourceNativeSymbol::new("native").unwrap())
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
    let bridge_definition_atom = definition_atom(bridge_definition.id());

    let lir_definition = CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(
            ConeIdentity::CORE,
            StrongDefinitionEntity::cone_image(ConeIdentity::CORE),
            StrongDefinitionRole::ImageDescriptor,
        )
        .unwrap(),
    )
    .unwrap();
    let lir_definition_atom = definition_atom(lir_definition.id());

    let mut foundation = CanonicalLirFoundation::empty();
    foundation
        .set_bridge_units(vec![bridge_unit.clone()])
        .unwrap();
    foundation.set_bridge_atoms(vec![bridge_atom]).unwrap();
    let mut definitions = vec![bridge_definition.clone()];
    let mut atoms = vec![bridge_definition_atom];
    if include_lir_plan {
        definitions.push(lir_definition.clone());
        atoms.push(lir_definition_atom);
    }
    foundation.set_definition_plans(definitions).unwrap();
    foundation.set_definition_atoms(atoms).unwrap();

    (
        ConeLirFoundation::try_new(ConeIdentity::CORE, foundation).unwrap(),
        lir_definition.id(),
        bridge_definition.id(),
        bridge_unit.id(),
    )
}

fn definition_atom(
    plan: scoop_identity::ObjectDefinitionPlanId,
) -> scoop_identity::CborIdentityRecord<
    scoop_identity::ObjectDefinitionAtomId,
    ObjectDefinitionAtomKey,
> {
    CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        plan,
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap()
}
