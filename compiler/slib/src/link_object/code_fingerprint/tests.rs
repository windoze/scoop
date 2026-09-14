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
    CanonicalLirFoundation, CanonicalNativeExternalRequirementSurfaceV1, LirTargetProfile,
    OdrFreeLirFoundation,
};

use super::*;
use crate::{ExtensionRequirement, LogicalMemberKey, MemberStableKey, SlibMemberRecord};

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
fn link_object_projection_is_canonical_and_ignores_non_code_members() {
    let first = object_record("first", b"first");
    let second = object_record("second", b"second");
    let mut expected = vec![expected(&first), expected(&second)];
    expected.sort_unstable_by_key(|member| member.member);
    let metadata = SlibMemberRecord::new(
        ConeIdentity::CORE,
        MemberStableKey::HirMetadata,
        SlibMemberRole::HirMetadata,
        b"metadata",
    )
    .unwrap();

    let actual =
        verify_directory_records(&expected, &[second.clone(), metadata, first.clone()]).unwrap();

    assert_eq!(actual.members().len(), 2);
    assert_eq!(actual.members()[0].member(), first.id().min(second.id()));
    assert_eq!(actual.members()[1].member(), first.id().max(second.id()));
    assert_eq!(scoop_wire::encode(&actual).unwrap()[0], 0x82);
}

#[test]
fn link_object_projection_rejects_content_drift_and_link_extensions() {
    let record = object_record("object", b"final");
    let expected = vec![expected(&record)];
    let changed = object_record("object", b"changed");
    assert_eq!(
        verify_directory_records(&expected, &[changed]),
        Err(CodeLinkObjectMemberValidationError::MemberContentMismatch(
            record.id()
        ))
    );

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
    assert_eq!(
        verify_directory_records(&expected, &[record, extension]),
        Err(CodeLinkObjectMemberValidationError::UnsupportedLinkExtension(capability))
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
    let foundation = OdrFreeLirFoundation::try_new(ConeIdentity::CORE, canonical).unwrap();
    let linked = CanonicalNativeExternalRequirementSurfaceV1::from_foundation(
        LirTargetProfile::DARWIN_AARCH64,
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
    let foundation = OdrFreeLirFoundation::try_new(ConeIdentity::CORE, canonical).unwrap();
    CanonicalNativeExternalRequirementSurfaceV1::from_foundation(
        LirTargetProfile::DARWIN_AARCH64,
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
        NativeExternalSymbolKey::darwin_macho_external(&SourceNativeSymbol::new(symbol).unwrap())
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
            target_profile: TargetProfileWireId::darwin_aarch64(),
            object_format: ObjectFormatId::macho_relocatable(),
            verifier_capability: capability,
        },
        payload,
    )
    .unwrap()
}

fn expected(record: &SlibMemberRecord) -> ExpectedFinalLinkObjectMemberV1 {
    ExpectedFinalLinkObjectMemberV1 {
        member: record.id(),
        stable_key: record.stable_key().clone(),
        role: record.role().clone(),
        byte_length: record.byte_length(),
        content_digest: record.sha256(),
    }
}
