use scoop_identity::{
    CanonicalCAbiFunctionSignature, CanonicalCAbiReturn, CanonicalIdentifier,
    CanonicalNativeLibraryName, CborIdentityRecord, ConeIdentity, DeclarationScope,
    DefinitionOwnerChain, NativeExternalContract, NativeExternalContractRecord,
    NativeExternalSymbolKey, NativeLibraryBinding, NativeLinkRequirementKey, PackagePath,
    PersistentSourceNativeExternalContractId, SourceDeclarationKey, SourceDeclarationSite,
    SourceNativeExternalContractKey, SourceNativeSymbol,
};

use super::*;
use crate::CanonicalLirFoundation;

#[test]
fn groups_identical_contracts_and_resolves_their_library() {
    let requirement = CborIdentityRecord::from_key(NativeLinkRequirementKey::target_default(
        CanonicalNativeLibraryName::new("sample").unwrap(),
    ))
    .unwrap();
    let contract = NativeExternalContract::c_function(
        NativeLibraryBinding::Requirement(requirement.id()),
        signature(),
    );
    let records = vec![
        contract_record(2, "native", contract.clone()),
        contract_record(1, "native", contract),
    ];
    let foundation = foundation(records, vec![requirement.clone()]);

    let surface = CanonicalNativeExternalRequirementSurfaceV1::from_foundation(
        LirTargetProfile::DARWIN_AARCH64,
        &foundation,
    )
    .unwrap();

    assert_eq!(surface.producer(), producer());
    assert_eq!(surface.contracts().len(), 1);
    assert_eq!(surface.contracts()[0].sources(), &[source(1), source(2)]);
    assert_eq!(
        surface.contracts()[0].library().requirement(),
        Some(&requirement)
    );
    assert_eq!(surface.library_requirements(), &[requirement]);
}

#[test]
fn preserves_default_native_namespace_without_fabricating_a_requirement() {
    let foundation = foundation(
        vec![contract_record(
            1,
            "native",
            NativeExternalContract::c_function(
                NativeLibraryBinding::DefaultNativeNamespace,
                signature(),
            ),
        )],
        Vec::new(),
    );

    let surface = CanonicalNativeExternalRequirementSurfaceV1::from_foundation(
        LirTargetProfile::DARWIN_AARCH64,
        &foundation,
    )
    .unwrap();

    assert_eq!(
        surface.contracts()[0].library(),
        &CanonicalNativeLibraryBindingV1::DefaultNativeNamespace
    );
    assert!(surface.library_requirements().is_empty());
}

#[test]
fn rejects_conflicting_contracts_for_one_link_symbol() {
    let requirement = CborIdentityRecord::from_key(NativeLinkRequirementKey::target_default(
        CanonicalNativeLibraryName::new("conflict").unwrap(),
    ))
    .unwrap();
    let foundation = foundation(
        vec![
            contract_record(
                1,
                "native",
                NativeExternalContract::c_function(
                    NativeLibraryBinding::DefaultNativeNamespace,
                    signature(),
                ),
            ),
            contract_record(
                2,
                "native",
                NativeExternalContract::c_function(
                    NativeLibraryBinding::Requirement(requirement.id()),
                    signature(),
                ),
            ),
        ],
        vec![requirement],
    );

    assert!(matches!(
        CanonicalNativeExternalRequirementSurfaceV1::from_foundation(
            LirTargetProfile::DARWIN_AARCH64,
            &foundation,
        ),
        Err(CanonicalNativeExternalRequirementBuildError::ConflictingContract { .. })
    ));
}

#[test]
fn rejects_missing_and_orphan_library_requirements() {
    let missing = CborIdentityRecord::from_key(NativeLinkRequirementKey::target_default(
        CanonicalNativeLibraryName::new("missing").unwrap(),
    ))
    .unwrap()
    .id();
    let missing_foundation = foundation(
        vec![contract_record(
            1,
            "native",
            NativeExternalContract::c_function(
                NativeLibraryBinding::Requirement(missing),
                signature(),
            ),
        )],
        Vec::new(),
    );
    assert!(matches!(
        CanonicalNativeExternalRequirementSurfaceV1::from_foundation(
            LirTargetProfile::DARWIN_AARCH64,
            &missing_foundation,
        ),
        Err(CanonicalNativeExternalRequirementBuildError::MissingLibraryRequirement {
            requirement,
            ..
        }) if requirement == missing
    ));

    let orphan = CborIdentityRecord::from_key(NativeLinkRequirementKey::target_default(
        CanonicalNativeLibraryName::new("orphan").unwrap(),
    ))
    .unwrap();
    let orphan_foundation = foundation(Vec::new(), vec![orphan.clone()]);
    assert_eq!(
        CanonicalNativeExternalRequirementSurfaceV1::from_foundation(
            LirTargetProfile::DARWIN_AARCH64,
            &orphan_foundation,
        ),
        Err(CanonicalNativeExternalRequirementBuildError::UnusedLibraryRequirement(orphan.id()))
    );
}

fn foundation(
    contracts: Vec<NativeExternalContractRecord>,
    requirements: Vec<CanonicalNativeLibraryRequirementV1>,
) -> OdrFreeLirFoundation {
    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_native_contracts(contracts).unwrap();
    canonical
        .set_native_link_requirements(requirements)
        .unwrap();
    OdrFreeLirFoundation::try_new(producer(), canonical).unwrap()
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

fn signature() -> CanonicalCAbiFunctionSignature {
    CanonicalCAbiFunctionSignature::cdecl(Vec::new(), CanonicalCAbiReturn::Void)
}

const fn producer() -> ConeIdentity {
    ConeIdentity::CORE
}

fn source(seed: u8) -> PersistentSourceNativeExternalContractId {
    let declaration = SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            producer(),
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
