use scoop_identity::{
    CanonicalCAbiFunctionSignature, CanonicalCAbiReturn, CanonicalIdentifier,
    CanonicalNativeLibraryName, CborIdentityRecord, ConeIdentity, DeclarationScope,
    DefinitionOwnerChain, NativeExternalContract, NativeExternalContractRecord,
    NativeExternalSymbolKey, NativeLibraryBinding, NativeLinkRequirementKey, PackagePath,
    PersistentSourceNativeExternalContractId, SourceDeclarationKey, SourceDeclarationSite,
    SourceNativeExternalContractKey, SourceNativeSymbol,
};
use scoop_lir::{
    CanonicalLirFoundation, CanonicalNativeExternalRequirementSurfaceV1, LirTargetProfile,
    OdrFreeLirFoundation,
};

use super::*;

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
