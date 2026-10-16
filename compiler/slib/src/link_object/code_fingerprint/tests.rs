use scoop_identity::{
    CanonicalCAbiFunctionSignature, CanonicalCAbiReturn, CanonicalIdentifier,
    CanonicalNativeLibraryName, CapabilityId, CborIdentityRecord, ConeIdentity, DeclarationScope,
    DefinitionOwnerChain, NativeExternalContract, NativeExternalContractRecord,
    NativeExternalSymbolKey, NativeLibraryBinding, NativeLinkRequirementKey, ObjectFormatId,
    PackagePath, PersistentSourceNativeExternalContractId, SourceDeclarationKey,
    SourceDeclarationSite, SourceNativeExternalContractKey, SourceNativeSymbol,
    TargetProfileWireId,
};
use scoop_lir::{
    CanonicalLirFoundation, CanonicalNativeExternalRequirementSurfaceV1, ConeLirFoundation,
    CrossConeLirBridgeSectionV1,
};

use super::*;
use crate::{
    CrossConeLinkSemanticImportSetV1, ExtensionRequirement, LogicalMemberKey, MemberStableKey,
    SlibMemberRecord,
};

#[test]
fn empty_link_object_projection_is_the_canonical_empty_array() {
    assert_eq!(
        scoop_wire::encode(&CodeLinkObjectMemberSetV1 {
            members: Vec::new()
        })
        .unwrap(),
        vec![0x80]
    );
}

#[test]
fn known_link_contributions_distinguish_legacy_and_cross_cone_profiles() {
    let legacy = CanonicalKnownLinkExtensionCodeContributionSetV1::empty();
    assert_eq!(scoop_wire::encode(&legacy).unwrap(), vec![0x80]);

    let foundation =
        ConeLirFoundation::try_new(ConeIdentity::CORE, CanonicalLirFoundation::empty()).unwrap();
    let bridge = CrossConeLirBridgeSectionV1::try_new(&foundation, Vec::new(), Vec::new()).unwrap();
    let semantic_imports = CrossConeLinkSemanticImportSetV1::from_lir_bridge(&bridge).unwrap();
    let cross_cone =
        CanonicalKnownLinkExtensionCodeContributionSetV1::from_cross_cone_semantic_imports(
            &semantic_imports,
        )
        .unwrap();

    assert_eq!(cross_cone.contributions().len(), 1);
    assert_eq!(
        cross_cone.contributions()[0].capability(),
        &crate::lir_cross_cone_link_closure_capability()
    );
    assert_eq!(
        cross_cone.contributions()[0].canonical_projection(),
        &[0x80]
    );

    let mut expected = vec![0x81, 0xa2, 0x01, 0xa3, 0x01, 0x72];
    expected.extend_from_slice(b"org.scoop-lang.lir");
    expected.extend_from_slice(&[0x02, 0x77]);
    expected.extend_from_slice(b"cross-cone-link-closure");
    expected.extend_from_slice(&[0x03, 0x01, 0x02, 0x41, 0x80]);
    assert_eq!(scoop_wire::encode(&cross_cone).unwrap(), expected);
}

#[test]
fn layout_profile_code_contribution_contains_both_semantic_import_projections() {
    let consumer = ConeIdentity::SINGLE_FILE;
    let provider = crate::link_object::layout_link_closure::tests::fixture::Provider::new();
    let layout = provider.consumer(consumer);
    let foundation = ConeLirFoundation::try_new(consumer, CanonicalLirFoundation::empty()).unwrap();
    let bridge = CrossConeLirBridgeSectionV1::try_new(&foundation, Vec::new(), Vec::new()).unwrap();
    let callable_imports = CrossConeLinkSemanticImportSetV1::from_lir_bridge(&bridge).unwrap();

    let contributions =
        CanonicalKnownLinkExtensionCodeContributionSetV1::from_cross_cone_layout_semantic_imports(
            &callable_imports,
            layout.selected().physical_imports(),
        )
        .unwrap();
    assert_eq!(contributions.contributions().len(), 2);
    assert_eq!(
        contributions.contributions()[0].capability(),
        &crate::lir_cross_cone_layout_link_closure_capability()
    );
    assert_eq!(
        contributions.contributions()[1].capability(),
        &crate::lir_cross_cone_link_closure_capability()
    );
    assert_eq!(
        contributions.contributions()[1].canonical_projection(),
        &[0x80]
    );
    assert_eq!(
        contributions.contributions()[0].canonical_projection(),
        scoop_wire::encode(&layout.selected().physical_imports().code_projection()).unwrap()
    );
    assert_ne!(
        contributions.contributions()[0].canonical_projection(),
        &[0x80]
    );
    assert_eq!(scoop_wire::encode(&contributions).unwrap()[0], 0x82);
}

#[test]
fn link_object_projection_is_canonical_and_ignores_non_code_members() {
    let first = object_record("first", b"first");
    let second = object_record("second", b"second");
    let metadata = SlibMemberRecord::new(
        ConeIdentity::CORE,
        MemberStableKey::HirMetadata,
        SlibMemberRole::HirMetadata,
        b"metadata",
    )
    .unwrap();

    let actual = project_directory_records(&[second.clone(), metadata, first.clone()]).unwrap();

    assert_eq!(actual.members().len(), 2);
    assert_eq!(actual.members()[0].member(), first.id().min(second.id()));
    assert_eq!(actual.members()[1].member(), first.id().max(second.id()));
    assert_eq!(scoop_wire::encode(&actual).unwrap()[0], 0x82);
}

#[test]
fn link_object_projection_rejects_unknown_link_extensions() {
    let record = object_record("object", b"final");
    let capability = CapabilityId::new("example.extension", "required", 1).unwrap();
    let extension = SlibMemberRecord::new(
        ConeIdentity::CORE,
        MemberStableKey::ExtensionBlob {
            capability: capability.clone(),
            logical_key: LogicalMemberKey::new(b"extension".to_vec()).unwrap(),
        },
        SlibMemberRole::ExtensionBlob {
            capability: capability.clone(),
            requirement: ExtensionRequirement::Link,
        },
        b"extension",
    )
    .unwrap();
    let member = extension.id();
    assert_eq!(
        project_directory_records(&[record, extension]),
        Err(CodeObjectProjectionError::UnsupportedLinkExtension { member, capability })
    );
}

#[test]
fn omits_source_provenance_and_uses_native_symbol_order() {
    let first = requirement_surface(vec![
        contract_record(2, "zeta", default_contract()),
        contract_record(3, "alpha", default_contract()),
        contract_record(1, "zeta", default_contract()),
    ]);
    let second = requirement_surface(vec![
        contract_record(9, "zeta", default_contract()),
        contract_record(8, "alpha", default_contract()),
    ]);

    let first = CanonicalNativeExternalContractCodeSetV1::from_requirement_surface(&first).unwrap();
    let second =
        CanonicalNativeExternalContractCodeSetV1::from_requirement_surface(&second).unwrap();

    assert_eq!(first, second);
    assert_eq!(first.contracts().len(), 2);
    assert_eq!(
        first.contracts()[0]
            .symbol_key()
            .native_link_symbol()
            .as_bytes(),
        b"_alpha"
    );
    assert_eq!(
        first.contracts()[1]
            .symbol_key()
            .native_link_symbol()
            .as_bytes(),
        b"_zeta"
    );
    assert_eq!(
        scoop_wire::encode(&first).unwrap(),
        scoop_wire::encode(&second).unwrap()
    );
}

#[test]
fn changing_the_contract_changes_the_code_projection() {
    let default = CanonicalNativeExternalContractCodeSetV1::from_requirement_surface(
        &requirement_surface(vec![contract_record(1, "native", default_contract())]),
    )
    .unwrap();
    let requirement = CborIdentityRecord::from_key(NativeLinkRequirementKey::target_default(
        scoop_identity::TargetProfileWireId::new(scoop_target_data::DARWIN_AARCH64_ID),
        CanonicalNativeLibraryName::new("sample").unwrap(),
    ))
    .unwrap();
    let linked = NativeExternalContract::c_function(
        NativeLibraryBinding::Requirement(requirement.id()),
        signature(),
    );
    let mut canonical = CanonicalLirFoundation::empty();
    canonical
        .set_native_contracts(vec![contract_record(1, "native", linked)])
        .unwrap();
    canonical
        .set_native_link_requirements(vec![requirement])
        .unwrap();
    let foundation = ConeLirFoundation::try_new(ConeIdentity::CORE, canonical).unwrap();
    let linked = CanonicalNativeExternalRequirementSurfaceV1::from_foundation(
        scoop_target_data::DARWIN_AARCH64,
        &foundation,
    )
    .unwrap();
    let linked =
        CanonicalNativeExternalContractCodeSetV1::from_requirement_surface(&linked).unwrap();

    assert_ne!(
        scoop_wire::encode(&default).unwrap(),
        scoop_wire::encode(&linked).unwrap()
    );
}

fn requirement_surface(
    contracts: Vec<NativeExternalContractRecord>,
) -> CanonicalNativeExternalRequirementSurfaceV1 {
    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_native_contracts(contracts).unwrap();
    let foundation = ConeLirFoundation::try_new(ConeIdentity::CORE, canonical).unwrap();
    CanonicalNativeExternalRequirementSurfaceV1::from_foundation(
        scoop_target_data::DARWIN_AARCH64,
        &foundation,
    )
    .unwrap()
}

fn contract_record(
    source_seed: u8,
    symbol: &str,
    contract: NativeExternalContract,
) -> NativeExternalContractRecord {
    NativeExternalContractRecord::new(
        source(source_seed),
        NativeExternalSymbolKey::for_target(
            scoop_identity::TargetProfileWireId::new(scoop_target_data::DARWIN_AARCH64_ID),
            scoop_identity::NativeSymbolNormalization::MachOExternalUnderscore,
            &SourceNativeSymbol::new(symbol).unwrap(),
        )
        .unwrap(),
        contract,
    )
    .unwrap()
}

fn default_contract() -> NativeExternalContract {
    NativeExternalContract::c_function(NativeLibraryBinding::DefaultNativeNamespace, signature())
}

fn signature() -> CanonicalCAbiFunctionSignature {
    CanonicalCAbiFunctionSignature::cdecl(Vec::new(), CanonicalCAbiReturn::Void)
}

fn source(seed: u8) -> PersistentSourceNativeExternalContractId {
    let declaration = SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(&format!("native_{seed}")).unwrap(),
        0,
        None,
        Vec::new(),
    );
    PersistentSourceNativeExternalContractId::from_key(
        &SourceNativeExternalContractKey::function(&declaration).unwrap(),
    )
    .unwrap()
}

fn object_record(logical_key: &str, payload: &[u8]) -> SlibMemberRecord {
    let capability = crate::scoop_lir_link_object_capability();
    SlibMemberRecord::new(
        ConeIdentity::CORE,
        MemberStableKey::LinkObject {
            verifier_capability: capability.clone(),
            logical_key: LogicalMemberKey::new(logical_key.as_bytes().to_vec()).unwrap(),
        },
        SlibMemberRole::LinkObject {
            target_profile: TargetProfileWireId::new(scoop_target_data::DARWIN_AARCH64_ID),
            object_format: ObjectFormatId::macho_relocatable(),
            verifier_capability: capability,
        },
        payload,
    )
    .unwrap()
}
