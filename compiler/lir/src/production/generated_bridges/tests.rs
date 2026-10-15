use scoop_identity::{
    CborIdentityRecord, ConeIdentity, DefinitionAtomRole, DefinitionAtomSubkey,
    GeneratedBridgeAtomId, GeneratedBridgeAtomKey, GeneratedBridgeAtomRoleKey,
    GeneratedBridgeUnitId, GeneratedBridgeUnitKey, NativeExternalContract,
    NativeExternalContractFingerprint, NativeExternalSymbolKey, NativeLibraryBinding,
    ObjectDefinitionAtomKey, ObjectDefinitionPlanId, ObjectDefinitionPlanKey,
    PersistentNativeExternalSymbolId, SourceNativeSymbol, StrongDefinitionEntity,
    StrongDefinitionRole,
};
use scoop_wire::decode_canonical;

use super::*;
use crate::{CanonicalLirFoundation, ConeLirFoundation};

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

struct Fixture {
    bridge_atom: CborIdentityRecord<GeneratedBridgeAtomId, GeneratedBridgeAtomKey>,
    foundation: ConeLirFoundation,
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
    Fixture {
        bridge_atom,
        foundation,
    }
}

fn unit_record() -> CborIdentityRecord<GeneratedBridgeUnitId, GeneratedBridgeUnitKey> {
    let symbol = NativeExternalSymbolKey::for_target(
        scoop_identity::TargetProfileWireId::new(crate::target_fixtures::DARWIN_AARCH64_ID),
        scoop_identity::NativeSymbolNormalization::MachOExternalUnderscore,
        &SourceNativeSymbol::new("native").unwrap(),
    )
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
    CborIdentityRecord::from_key(GeneratedBridgeUnitKey::OutboundFunction(
        fingerprint,
        scoop_identity::CResultAdaptation::Direct,
    ))
    .unwrap()
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
