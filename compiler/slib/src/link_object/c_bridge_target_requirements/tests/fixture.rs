use crate::link_object::generated_c_objects::tests::{fixture, member_plan, profile};
use crate::link_object::native_requirements::tests::{contract_record, native_surface};
use crate::link_object::relocation_verification::tests::add_undefined_symbols;
use crate::link_object::symbol_verification::tests::object_for_plan_with_deployment;
use crate::{
    DigestPatchSiteV1, GeneratedCBridgeObjectCandidateV1, PlannedStrongObjectSymbolRoleV1,
    PlannedStrongObjectSymbolSetV1, ScoopLirDigestPatchSitesV1, ScoopLirObjectCandidateV1,
    read_generated_c_objects_v1, verify_builtin_object_strong_relocations_v1,
};
use scoop_identity::{
    ConeIdentity, DefinitionAtomRole, DigestNodeId, DigestNodeKey, DigestPatchIntentKey,
    DigestSemanticFieldRole, NativeLibraryBinding,
};
use scoop_lir::{
    CBridgeProductionSetV1, CBridgeToolchainProfileV1, CanonicalNativeExternalRequirementSurfaceV1,
    DigestNodeV1, GeneratedBridgePlanSetV1, ObjectSymbolSurfaceV1,
};
const MINIMUM_OS: u32 = 0x000d_0100;
const SDK: u32 = 0x000e_0200;

pub(in crate::link_object) struct SemanticFixture {
    pub(in crate::link_object) scoop_patch_sites: ScoopLirDigestPatchSitesV1,
    pub(in crate::link_object) bridge_plan: GeneratedBridgePlanSetV1,
    pub(in crate::link_object) native_requirements: CanonicalNativeExternalRequirementSurfaceV1,
    pub(in crate::link_object) profile: CBridgeToolchainProfileV1,
}

pub(in crate::link_object) fn semantic_fixture(
    native_name: &str,
    relocation_symbols: &[&[u8]],
) -> SemanticFixture {
    semantic_fixture_with_additional_contracts(native_name, &[], relocation_symbols)
}

pub(in crate::link_object) fn semantic_fixture_with_additional_contracts(
    native_name: &str,
    additional_native_names: &[&str],
    relocation_symbols: &[&[u8]],
) -> SemanticFixture {
    let fixture = fixture(Some(native_name));
    let bridge_plan = GeneratedBridgePlanSetV1::from_foundation(&fixture.foundation).unwrap();
    let member_plan = member_plan(&fixture, &bridge_plan);
    let surface = ObjectSymbolSurfaceV1::from_foundation(&fixture.foundation).unwrap();
    let symbol_plan = PlannedStrongObjectSymbolSetV1::new(
        scoop_target_data::DARWIN_AARCH64,
        &surface,
        &member_plan,
    )
    .unwrap();
    let profile = profile("clang-2100.1.1.101", MINIMUM_OS, SDK);

    let scoop_member = member_plan.scoop_lir_members()[0].member_id();
    let scoop_object = object_for_plan_with_deployment(
        symbol_plan.member(scoop_member).unwrap(),
        |role| match role {
            PlannedStrongObjectSymbolRoleV1::PrimaryDefinition { .. }
            | PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart { .. } => 0,
            PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd { .. } => 64,
        },
        &[],
        None,
        &[0; 64],
    );
    let bridge_member = member_plan.generated_bridge_members()[0].member_id();
    let bridge_symbols = symbol_plan.member(bridge_member).unwrap();
    let primary = primary_role(bridge_symbols);
    let relocations = relocation_symbols
        .iter()
        .enumerate()
        .map(|(index, _)| (u32::try_from(index).unwrap() * 4, primary))
        .collect::<Vec<_>>();
    let mut bridge_bytes = object_for_plan_with_deployment(
        bridge_symbols,
        canonical_value,
        &relocations,
        Some((MINIMUM_OS, SDK)),
        &[0xaa, 0xbb, 0xcc, 0xdd, 0, 0, 0, 0],
    )
    .bytes;
    if !relocation_symbols.is_empty() {
        bridge_bytes = add_undefined_symbols(bridge_bytes, true, relocation_symbols);
    }
    let scoop_objects = [ScoopLirObjectCandidateV1::new(
        scoop_member,
        &scoop_object.bytes,
    )];
    let bridge_objects = [GeneratedCBridgeObjectCandidateV1::new(
        bridge_member,
        &bridge_bytes,
    )];
    let production = CBridgeProductionSetV1::from_generated_bridge_plan(&bridge_plan, &profile);
    let production_proof =
        read_generated_c_objects_v1(production, &profile, &bridge_objects).unwrap();
    let builtins = verify_builtin_object_strong_relocations_v1(
        &member_plan,
        &symbol_plan,
        &scoop_objects,
        production_proof,
        &bridge_objects,
    )
    .unwrap();
    let image_key = DigestNodeKey::runtime_image(fixture.foundation.producer());
    let image_node_id = DigestNodeId::from_key(&image_key).unwrap();
    let image_patch = DigestPatchIntentKey::new(
        image_node_id,
        fixture.lir_plan,
        DefinitionAtomRole::Primary,
        DigestSemanticFieldRole::RuntimeImage,
    );
    let image_node = DigestNodeV1::new(image_key, Vec::new(), vec![image_patch]).unwrap();
    let image_intent = image_node.patch_intents()[0].id();
    let patch_sites = [DigestPatchSiteV1::new(
        image_intent,
        scoop_member,
        u64::try_from(scoop_object.section_offset).unwrap() + 16,
    )];
    let scoop_patch_sites = ScoopLirDigestPatchSitesV1::new(builtins, (patch_sites).to_vec());
    let mut contracts = vec![contract_record(
        ConeIdentity::CORE,
        "nativeBridgeDeclaration",
        native_name,
        NativeLibraryBinding::DefaultNativeNamespace,
    )];
    contracts.extend(
        additional_native_names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                contract_record(
                    ConeIdentity::CORE,
                    &format!("additionalNativeBridgeDeclaration{index}"),
                    name,
                    NativeLibraryBinding::DefaultNativeNamespace,
                )
            }),
    );
    let native_requirements = native_surface(ConeIdentity::CORE, contracts, Vec::new());
    SemanticFixture {
        scoop_patch_sites,
        bridge_plan,
        native_requirements,
        profile,
    }
}

fn primary_role(
    symbols: &crate::PlannedMemberStrongObjectSymbolsV1,
) -> PlannedStrongObjectSymbolRoleV1 {
    symbols
        .symbols()
        .iter()
        .find_map(|symbol| match symbol.role() {
            role @ PlannedStrongObjectSymbolRoleV1::PrimaryDefinition { .. } => Some(role),
            _ => None,
        })
        .unwrap()
}

fn canonical_value(role: PlannedStrongObjectSymbolRoleV1) -> u64 {
    match role {
        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition { .. }
        | PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart { .. } => 0,
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd { .. } => 8,
    }
}
