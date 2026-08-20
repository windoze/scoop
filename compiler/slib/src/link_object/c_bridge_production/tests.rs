use object::macho;
use scoop_identity::{
    CborIdentityRecord, ConeIdentity, DefinitionAtomRole, DefinitionAtomSubkey,
    GeneratedBridgeAtomKey, GeneratedBridgeAtomRoleKey, GeneratedBridgeUnitId,
    GeneratedBridgeUnitKey, LinkageClass, NativeExternalContract,
    NativeExternalContractFingerprint, NativeExternalSymbolKey, ObjectDefinitionAtomId,
    ObjectDefinitionAtomKey, ObjectDefinitionPlanId, ObjectDefinitionPlanKey,
    PersistentNativeExternalSymbolId, PersistentSymbolRequest, PersistentSymbolRequestTable,
    SourceNativeSymbol, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{
    AppleClangCompilerIdentityV1, CBridgeProductionSetV1, CBridgeToolchainProfileV1,
    CanonicalLirFoundation, ConeLirFoundation, DarwinCBridgeDeploymentContractV1,
    DarwinPackedVersionV1, GeneratedBridgePlanSetV1, ProducerUnitPartitionV1,
};

use super::*;
use crate::{CanonicalGeneratedBridgeObjectUnitSetV1, CanonicalScoopLirObjectUnitSetV1};

const MINIMUM_OS: u32 = 0x000d_0100;
const SDK: u32 = 0x000e_0200;

#[test]
fn binds_profile_production_plan_members_and_object_envelopes() {
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

    let verified = verify_c_bridge_production_envelopes_v1(
        bridge_plan.clone(),
        production.clone(),
        &profile,
        &member_plan,
        &objects,
    )
    .unwrap();

    assert_eq!(verified.producer(), ConeIdentity::CORE);
    assert_eq!(verified.production(), &production);
    assert_eq!(verified.members().len(), 1);
    assert_eq!(verified.members()[0].plan().member_id(), bridge_member);
    let super::super::ObjectEnvelopeFormatV1::DarwinArm64 { deployment, .. } = verified.members()
        [0]
    .envelope()
    .sections()
    .envelope()
    .format() else {
        panic!("Darwin envelope")
    };
    assert_eq!(
        deployment.as_ref(),
        Some(&super::super::DarwinDeploymentCommandV1::BuildVersion {
            minimum_os: MINIMUM_OS,
            sdk: SDK,
            tools: Vec::new(),
        })
    );
}

#[test]
fn accepts_explicit_not_used_only_when_plan_and_members_are_empty() {
    let fixture = fixture(None);
    let bridge_plan = GeneratedBridgePlanSetV1::from_foundation(&fixture.foundation).unwrap();
    let member_plan = member_plan(&fixture, &bridge_plan);
    let profile = profile("clang-2100.1.1.101", MINIMUM_OS, SDK);
    let production = CBridgeProductionSetV1::from_generated_bridge_plan(&bridge_plan, &profile);

    let verified = verify_c_bridge_production_envelopes_v1(
        bridge_plan.clone(),
        production.clone(),
        &profile,
        &member_plan,
        &[],
    )
    .unwrap();

    assert_eq!(verified.production(), &CBridgeProductionSetV1::NotUsed);
    assert!(verified.members().is_empty());
}

#[test]
fn rejects_used_for_an_empty_plan_and_not_used_for_a_nonempty_plan() {
    let empty = fixture(None);
    let empty_bridge_plan = GeneratedBridgePlanSetV1::from_foundation(&empty.foundation).unwrap();
    let empty_member_plan = member_plan(&empty, &empty_bridge_plan);
    let used = fixture(Some("native_bridge"));
    let used_bridge_plan = GeneratedBridgePlanSetV1::from_foundation(&used.foundation).unwrap();
    let used_member_plan = member_plan(&used, &used_bridge_plan);
    let profile = profile("clang-2100.1.1.101", MINIMUM_OS, SDK);
    let used_production =
        CBridgeProductionSetV1::from_generated_bridge_plan(&used_bridge_plan, &profile);

    assert_eq!(
        verify_c_bridge_production_envelopes_v1(
            empty_bridge_plan,
            used_production,
            &profile,
            &empty_member_plan,
            &[],
        ),
        Err(CBridgeProductionEnvelopeValidationError::UnexpectedProduction)
    );
    assert_eq!(
        verify_c_bridge_production_envelopes_v1(
            used_bridge_plan,
            CBridgeProductionSetV1::NotUsed,
            &profile,
            &used_member_plan,
            &[],
        ),
        Err(CBridgeProductionEnvelopeValidationError::MissingProduction)
    );
}

#[test]
fn rejects_manifest_profile_even_when_object_deployment_would_validate() {
    let fixture = fixture(Some("native_bridge"));
    let bridge_plan = GeneratedBridgePlanSetV1::from_foundation(&fixture.foundation).unwrap();
    let member_plan = member_plan(&fixture, &bridge_plan);
    let manifest_profile = profile("clang-2100.1.1.101", MINIMUM_OS, SDK);
    let request_profile = profile("clang-2100.1.1.102", MINIMUM_OS, SDK);
    let production =
        CBridgeProductionSetV1::from_generated_bridge_plan(&bridge_plan, &manifest_profile);
    let bytes = object(MINIMUM_OS, SDK);
    let objects = [GeneratedCBridgeObjectCandidateV1::new(
        member_plan.generated_bridge_members()[0].member_id(),
        &bytes,
    )];

    assert_eq!(
        verify_c_bridge_production_envelopes_v1(
            bridge_plan,
            production,
            &request_profile,
            &member_plan,
            &objects,
        ),
        Err(CBridgeProductionEnvelopeValidationError::ProfileFingerprintMismatch)
    );
}

#[test]
fn rejects_missing_duplicate_and_wrong_deployment_objects() {
    let fixture = fixture(Some("native_bridge"));
    let bridge_plan = GeneratedBridgePlanSetV1::from_foundation(&fixture.foundation).unwrap();
    let member_plan = member_plan(&fixture, &bridge_plan);
    let profile = profile("clang-2100.1.1.101", MINIMUM_OS, SDK);
    let production = CBridgeProductionSetV1::from_generated_bridge_plan(&bridge_plan, &profile);
    let member = member_plan.generated_bridge_members()[0].member_id();
    let bytes = object(MINIMUM_OS, SDK);

    assert_eq!(
        verify_c_bridge_production_envelopes_v1(
            bridge_plan.clone(),
            production.clone(),
            &profile,
            &member_plan,
            &[],
        ),
        Err(CBridgeProductionEnvelopeValidationError::MissingObjectMember(member))
    );

    let duplicate = [
        GeneratedCBridgeObjectCandidateV1::new(member, &bytes),
        GeneratedCBridgeObjectCandidateV1::new(member, &bytes),
    ];
    assert_eq!(
        verify_c_bridge_production_envelopes_v1(
            bridge_plan.clone(),
            production.clone(),
            &profile,
            &member_plan,
            &duplicate,
        ),
        Err(CBridgeProductionEnvelopeValidationError::DuplicateObjectMember(member))
    );

    let wrong_bytes = object(MINIMUM_OS, SDK + 0x100);
    let wrong = [GeneratedCBridgeObjectCandidateV1::new(member, &wrong_bytes)];
    assert!(matches!(
        verify_c_bridge_production_envelopes_v1(
            bridge_plan.clone(),
            production.clone(),
            &profile,
            &member_plan,
            &wrong,
        ),
        Err(CBridgeProductionEnvelopeValidationError::ObjectEnvelope {
            member: actual,
            source: super::super::GeneratedCBridgeObjectEnvelopeValidationError::DeploymentMismatch { .. },
        }) if actual == member
    ));
}

#[test]
fn rejects_member_plan_for_a_different_generated_unit() {
    let first = fixture(Some("native_bridge_a"));
    let bridge_plan = GeneratedBridgePlanSetV1::from_foundation(&first.foundation).unwrap();
    let other = fixture(Some("native_bridge_b"));
    let other_bridge_plan = GeneratedBridgePlanSetV1::from_foundation(&other.foundation).unwrap();
    let other_member_plan = member_plan(&other, &other_bridge_plan);
    let profile = profile("clang-2100.1.1.101", MINIMUM_OS, SDK);
    let production = CBridgeProductionSetV1::from_generated_bridge_plan(&bridge_plan, &profile);

    assert_eq!(
        verify_c_bridge_production_envelopes_v1(
            bridge_plan,
            production,
            &profile,
            &other_member_plan,
            &[],
        ),
        Err(
            CBridgeProductionEnvelopeValidationError::UnexpectedPlannedUnit(
                other.bridge_unit.unwrap(),
            )
        )
    );
}

pub(in crate::link_object) struct Fixture {
    pub(in crate::link_object) foundation: ConeLirFoundation,
    pub(in crate::link_object) lir_plan: ObjectDefinitionPlanId,
    pub(in crate::link_object) bridge_unit: Option<GeneratedBridgeUnitId>,
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
    let bridge_unit = native_name.map(|native_name| {
        let native_symbol = NativeExternalSymbolKey::darwin_macho_external(
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
        unit.id()
    });
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
        bridge_unit,
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
        scoop_lir::LirTargetProfile::DARWIN_AARCH64,
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
    CBridgeToolchainProfileV1::new_darwin_aarch64_apple_clang(
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
