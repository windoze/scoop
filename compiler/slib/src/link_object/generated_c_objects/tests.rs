use object::macho;
use scoop_identity::{
    CborIdentityRecord, ConeIdentity, DefinitionAtomRole, DefinitionAtomSubkey,
    GeneratedBridgeAtomKey, GeneratedBridgeAtomRoleKey, GeneratedBridgeUnitKey, LinkageClass,
    NativeExternalContract, NativeExternalContractFingerprint, NativeExternalSymbolKey,
    ObjectDefinitionAtomId, ObjectDefinitionAtomKey, ObjectDefinitionPlanId,
    ObjectDefinitionPlanKey, PersistentNativeExternalSymbolId, PersistentSymbolRequest,
    PersistentSymbolRequestTable, SourceNativeSymbol, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{
    AppleClangCompilerIdentityV1, CBridgeProductionSetV1, CBridgeToolchainProfileV1,
    CanonicalLirFoundation, ConeLirFoundation, DarwinCBridgeDeploymentContractV1,
    DarwinPackedVersionV1, GeneratedBridgePlanSetV1, ProducerUnitPartitionV1,
};

use super::*;
use crate::{
    CanonicalGeneratedBridgeObjectUnitSetV1, CanonicalScoopLirObjectUnitSetV1,
    PlannedLinkObjectMemberSetV1,
};

const MINIMUM_OS: u32 = 0x000d_0100;
const SDK: u32 = 0x000e_0200;

#[test]
fn reads_generated_member_sections() {
    let fixture = fixture(Some("native_bridge"));
    let bridge_plan = GeneratedBridgePlanSetV1::from_foundation(&fixture.foundation).unwrap();
    let member_plan = member_plan(&fixture, &bridge_plan);
    let profile = profile("clang-2100.1.1.101", MINIMUM_OS, SDK);
    let production = CBridgeProductionSetV1::from_generated_bridge_plan(&bridge_plan, &profile);
    let bytes = object(MINIMUM_OS, SDK);
    let bridge_member = member_plan.generated_bridge_members()[0].member_id();
    let objects = [GeneratedCBridgeObjectCandidateV1::new(
        bridge_member,
        &bytes,
    )];

    let verified = read_generated_c_objects_v1(production.clone(), &profile, &objects).unwrap();

    assert_eq!(verified.production(), &production);
    assert_eq!(verified.members().len(), 1);
    assert_eq!(verified.members()[0].member(), bridge_member);
    assert_eq!(
        verified.members()[0].sections().envelope().target(),
        scoop_target_data::DARWIN_AARCH64_ID
    );
}

#[test]
fn reads_an_empty_generated_object_set() {
    let fixture = fixture(None);
    let bridge_plan = GeneratedBridgePlanSetV1::from_foundation(&fixture.foundation).unwrap();
    let profile = profile("clang-2100.1.1.101", MINIMUM_OS, SDK);
    let production = CBridgeProductionSetV1::from_generated_bridge_plan(&bridge_plan, &profile);

    let verified = read_generated_c_objects_v1(production.clone(), &profile, &[]).unwrap();

    assert_eq!(verified.production(), &CBridgeProductionSetV1::NotUsed);
    assert!(verified.members().is_empty());
}

pub(in crate::link_object) struct Fixture {
    pub(in crate::link_object) foundation: ConeLirFoundation,
    pub(in crate::link_object) lir_plan: ObjectDefinitionPlanId,
}

pub(in crate::link_object) fn fixture(native_name: Option<&str>) -> Fixture {
    let lir_definition = CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(
            ConeIdentity::CORE,
            StrongDefinitionEntity::cone_image(ConeIdentity::CORE),
            StrongDefinitionRole::ImageDescriptor,
        )
        .unwrap(),
    )
    .unwrap();
    let lir_plan = lir_definition.id();
    let mut definitions = vec![lir_definition];
    let mut atoms = vec![definition_atom(lir_plan)];
    let mut foundation = CanonicalLirFoundation::empty();
    if let Some(native_name) = native_name {
        let native_symbol = NativeExternalSymbolKey::for_target(
            scoop_identity::TargetProfileWireId::new(scoop_target_data::DARWIN_AARCH64_ID),
            scoop_identity::NativeSymbolNormalization::MachOExternalUnderscore,
            &SourceNativeSymbol::new(native_name).unwrap(),
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
        let unit = CborIdentityRecord::from_key(GeneratedBridgeUnitKey::OutboundFunction(
            fingerprint,
            scoop_identity::CResultAdaptation::Direct,
        ))
        .unwrap();
        let bridge_atom = CborIdentityRecord::from_key(GeneratedBridgeAtomKey::new(
            ConeIdentity::CORE,
            GeneratedBridgeAtomRoleKey::PrimaryEntry { unit: unit.id() },
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
        definitions.push(bridge_definition.clone());
        atoms.push(definition_atom(bridge_definition.id()));
        foundation.set_bridge_units(vec![unit.clone()]).unwrap();
        foundation.set_bridge_atoms(vec![bridge_atom]).unwrap();
    }
    let symbol_requests = definitions
        .iter()
        .map(|definition| {
            PersistentSymbolRequest::new(
                definition.key().primary_symbol_key().unwrap(),
                LinkageClass::ConeStrong,
            )
            .unwrap()
        })
        .collect();
    foundation.set_definition_plans(definitions).unwrap();
    foundation.set_definition_atoms(atoms).unwrap();
    foundation.set_symbol_requests(PersistentSymbolRequestTable::new(symbol_requests).unwrap());
    let foundation = ConeLirFoundation::try_new(ConeIdentity::CORE, foundation).unwrap();
    Fixture {
        foundation,
        lir_plan,
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

pub(in crate::link_object) fn member_plan(
    fixture: &Fixture,
    bridge_plan: &GeneratedBridgePlanSetV1,
) -> PlannedLinkObjectMemberSetV1 {
    let partition = ProducerUnitPartitionV1::from_foundation(&fixture.foundation).unwrap();
    let bridge_sets = bridge_plan
        .units()
        .iter()
        .map(|unit| CanonicalGeneratedBridgeObjectUnitSetV1::new(vec![unit.unit()]).unwrap())
        .collect();
    PlannedLinkObjectMemberSetV1::new(
        scoop_target_data::DARWIN_AARCH64,
        &partition,
        vec![CanonicalScoopLirObjectUnitSetV1::new(vec![fixture.lir_plan]).unwrap()],
        bridge_sets,
    )
    .unwrap()
}

pub(in crate::link_object) fn profile(
    compiler_build: &str,
    minimum_os: u32,
    sdk: u32,
) -> CBridgeToolchainProfileV1 {
    scoop_target_data::darwin_c_bridge_profile(
        DarwinCBridgeDeploymentContractV1::new(
            DarwinPackedVersionV1::new(minimum_os).unwrap(),
            DarwinPackedVersionV1::new(sdk).unwrap(),
            Vec::new(),
        )
        .unwrap(),
        AppleClangCompilerIdentityV1::new(21, 0, 0, compiler_build).unwrap(),
    )
    .unwrap()
}

fn object(minimum_os: u32, sdk: u32) -> Vec<u8> {
    let segment_size = 152_u32;
    let symtab_size = 24_u32;
    let dysymtab_size = 80_u32;
    let deployment_size = 24_u32;
    let command_count = 4_u32;
    let command_bytes = segment_size + symtab_size + dysymtab_size + deployment_size;
    let section_offset = 32 + command_bytes;
    let string_offset = section_offset + 4;
    let mut bytes = Vec::with_capacity(string_offset as usize + 1);

    push_u32(&mut bytes, macho::MH_MAGIC_64);
    push_u32(&mut bytes, macho::CPU_TYPE_ARM64);
    push_u32(&mut bytes, macho::CPU_SUBTYPE_ARM64_ALL);
    push_u32(&mut bytes, macho::MH_OBJECT);
    push_u32(&mut bytes, command_count);
    push_u32(&mut bytes, command_bytes);
    push_u32(&mut bytes, macho::MH_SUBSECTIONS_VIA_SYMBOLS);
    push_u32(&mut bytes, 0);

    push_u32(&mut bytes, macho::LC_SEGMENT_64);
    push_u32(&mut bytes, segment_size);
    bytes.extend_from_slice(&[0; 16]);
    push_u64(&mut bytes, 0);
    push_u64(&mut bytes, 4);
    push_u64(&mut bytes, u64::from(section_offset));
    push_u64(&mut bytes, 4);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 1);
    push_u32(&mut bytes, 0);
    push_fixed_name(&mut bytes, b"__text");
    push_fixed_name(&mut bytes, b"__TEXT");
    push_u64(&mut bytes, 0);
    push_u64(&mut bytes, 4);
    push_u32(&mut bytes, section_offset);
    push_u32(&mut bytes, 2);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);
    push_u32(
        &mut bytes,
        macho::S_REGULAR | macho::S_ATTR_PURE_INSTRUCTIONS | macho::S_ATTR_SOME_INSTRUCTIONS,
    );
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);

    push_u32(&mut bytes, macho::LC_SYMTAB);
    push_u32(&mut bytes, symtab_size);
    push_u32(&mut bytes, section_offset + 4);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, string_offset);
    push_u32(&mut bytes, 1);
    push_u32(&mut bytes, macho::LC_DYSYMTAB);
    push_u32(&mut bytes, dysymtab_size);
    bytes.extend_from_slice(&[0; 72]);

    push_u32(&mut bytes, macho::LC_BUILD_VERSION);
    push_u32(&mut bytes, deployment_size);
    push_u32(&mut bytes, macho::PLATFORM_MACOS);
    push_u32(&mut bytes, minimum_os);
    push_u32(&mut bytes, sdk);
    push_u32(&mut bytes, 0);
    bytes.extend_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd]);
    bytes.push(0);
    bytes
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_fixed_name(bytes: &mut Vec<u8>, name: &[u8]) {
    let mut fixed = [0; 16];
    fixed[..name.len()].copy_from_slice(name);
    bytes.extend_from_slice(&fixed);
}
