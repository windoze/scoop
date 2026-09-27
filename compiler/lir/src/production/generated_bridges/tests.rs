use scoop_identity::{
    CborIdentityRecord, ConeIdentity, DefinitionAtomRole, DefinitionAtomSubkey,
    GeneratedBridgeAtomId, GeneratedBridgeAtomKey, GeneratedBridgeAtomRoleKey,
    GeneratedBridgeUnitId, GeneratedBridgeUnitKey, NativeExternalContract,
    NativeExternalContractFingerprint, NativeExternalSymbolKey, NativeLibraryBinding,
    ObjectDefinitionAtomKey, ObjectDefinitionPlanId, ObjectDefinitionPlanKey,
    PendingIdentityValidation, PersistentNativeExternalSymbolId, SourceNativeSymbol,
    StrongDefinitionEntity, StrongDefinitionRole, ValidatedIdentityGraph,
};
use scoop_wire::{decode_canonical, encode};

use super::*;
use crate::{CanonicalLirFoundation, ConeLirFoundation};

#[test]
fn generated_bridge_plan_has_a_fixed_wire_vector_and_validates() {
    let fixture = fixture(true);
    assert_eq!(fixture.plan.units()[0].unit_authority(), &fixture.unit);
    assert_eq!(
        fixture.plan.units()[0].primary_atom_authority(),
        &fixture.bridge_atom
    );
    assert_eq!(
        hex(&encode(&fixture.plan).unwrap()),
        "81a401582065a6a97003171db803e4000a76035475b756279dcc38e3ea75a0bff21c2652ac02582013f35f95f19b6d94a4f4a29e3fbce3b718808bceaf354b351b598392b263848c03800480"
    );

    let decoded: DecodedGeneratedBridgePlanSetV1 =
        decode_canonical(&encode(&fixture.plan).unwrap()).unwrap();
    let mut identities = authorities(fixture.unit.id(), fixture.bridge_atom.id());
    assert_eq!(
        decoded.validate(&mut identities, &fixture.foundation),
        Ok(fixture.plan)
    );
}

#[test]
fn generated_bridge_reader_rejects_non_closed_products() {
    for bytes in [vec![0x81, 0xa3], vec![0x81, 0xa5]] {
        assert!(decode_canonical::<DecodedGeneratedBridgePlanSetV1>(&bytes,).is_err());
    }
}

#[test]
fn generated_bridge_plan_requires_primary_and_definition_plans() {
    let unit = unit_record();
    let mut no_primary = CanonicalLirFoundation::empty();
    no_primary.set_bridge_units(vec![unit.clone()]).unwrap();
    let no_primary = ConeLirFoundation::try_new(ConeIdentity::CORE, no_primary).unwrap();
    assert_eq!(
        GeneratedBridgePlanSetV1::from_foundation(&no_primary),
        Err(GeneratedBridgePlanBuildError::MissingPrimaryAtom(unit.id()))
    );

    let fixture = fixture(false);
    let expected = expected_definition_plan(fixture.bridge_atom.key()).id();
    assert_eq!(
        GeneratedBridgePlanSetV1::from_foundation(&fixture.foundation),
        Err(GeneratedBridgePlanBuildError::MissingDefinitionPlan {
            atom: fixture.bridge_atom.id(),
            expected,
        })
    );
}

#[test]
fn validation_rejects_atoms_moved_to_another_role_set() {
    let fixture = fixture(true);
    let bytes = encode(&fixture.plan).unwrap();
    let mut decoded: DecodedGeneratedBridgePlanSetV1 = decode_canonical(&bytes).unwrap();
    let primary_atom = decoded.units[0].primary_atom;
    decoded.units[0]
        .materialized_associated_atoms
        .push(primary_atom);
    let mut identities = authorities(fixture.unit.id(), fixture.bridge_atom.id());

    assert_eq!(
        decoded.validate(&mut identities, &fixture.foundation),
        Err(GeneratedBridgePlanValidationError::UnitMismatch { index: 0 })
    );
}

struct Fixture {
    unit: CborIdentityRecord<GeneratedBridgeUnitId, GeneratedBridgeUnitKey>,
    bridge_atom: CborIdentityRecord<GeneratedBridgeAtomId, GeneratedBridgeAtomKey>,
    foundation: ConeLirFoundation,
    plan: GeneratedBridgePlanSetV1,
}

fn fixture(include_definition: bool) -> Fixture {
    let unit = unit_record();
    let bridge_atom = CborIdentityRecord::from_key(GeneratedBridgeAtomKey::new(
        ConeIdentity::CORE,
        GeneratedBridgeAtomRoleKey::PrimaryEntry { unit: unit.id() },
    ))
    .unwrap();
    let mut foundation = CanonicalLirFoundation::empty();
    foundation.set_bridge_units(vec![unit.clone()]).unwrap();
    foundation
        .set_bridge_atoms(vec![bridge_atom.clone()])
        .unwrap();
    if include_definition {
        let definition_plan = expected_definition_plan(bridge_atom.key());
        let definition_atom = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
            definition_plan.id(),
            DefinitionAtomRole::Primary,
            DefinitionAtomSubkey::Singleton,
        ))
        .unwrap();
        foundation
            .set_definition_plans(vec![definition_plan])
            .unwrap();
        foundation
            .set_definition_atoms(vec![definition_atom])
            .unwrap();
    }
    let foundation = ConeLirFoundation::try_new(ConeIdentity::CORE, foundation).unwrap();
    let plan = if include_definition {
        GeneratedBridgePlanSetV1::from_foundation(&foundation).unwrap()
    } else {
        GeneratedBridgePlanSetV1 {
            producer: ConeIdentity::CORE,
            units: Vec::new(),
        }
    };
    Fixture {
        unit,
        bridge_atom,
        foundation,
        plan,
    }
}

fn unit_record() -> CborIdentityRecord<GeneratedBridgeUnitId, GeneratedBridgeUnitKey> {
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

fn expected_definition_plan(
    atom: &GeneratedBridgeAtomKey,
) -> CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey> {
    CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(
            ConeIdentity::CORE,
            StrongDefinitionEntity::generated_bridge_atom(atom).unwrap(),
            StrongDefinitionRole::GeneratedBridge,
        )
        .unwrap(),
    )
    .unwrap()
}

fn authorities(unit: GeneratedBridgeUnitId, atom: GeneratedBridgeAtomId) -> ValidatedIdentityGraph {
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    pending.register_authority(unit).unwrap();
    pending.register_authority(atom).unwrap();
    pending.finish().unwrap()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
