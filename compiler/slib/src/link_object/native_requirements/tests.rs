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
    OdrFreeLirFoundation, StrongExternalLirBridgeSurfaceV1,
};

use super::super::strong_relocation_closure::tests::{
    verified_member_with_undefined, verified_member_without_relocations,
};
use super::super::symbol_verification::tests::fixture_for_producer;
use super::*;
use crate::{verify_core_strong_requirements_v1, verify_current_cone_strong_relocation_closure_v1};

#[test]
fn resolves_source_extern_uses_and_preserves_unclassified_externals() {
    let producer = ConeIdentity::SINGLE_FILE;
    let object = fixture_for_producer(producer, "nativeConsumer");
    let member = verified_member_with_undefined(&object, b"_native");
    let core = core_closure(producer, member);
    let native = native_surface(
        producer,
        vec![contract_record(
            producer,
            "declaration",
            "native",
            NativeLibraryBinding::DefaultNativeNamespace,
        )],
        Vec::new(),
    );

    let verified = verify_source_external_requirements_v1(core, native).unwrap();

    assert_eq!(verified.producer(), producer);
    assert_eq!(verified.source_external_requirements().len(), 1);
    assert_eq!(
        verified.source_external_requirements()[0]
            .requirement()
            .library()
            .binding(),
        NativeLibraryBinding::DefaultNativeNamespace
    );
    assert_eq!(
        verified.source_external_requirements()[0]
            .use_site()
            .symbol(),
        b"_native"
    );
    assert!(verified.remaining_external_candidates().is_empty());

    let object = fixture_for_producer(producer, "runtimeConsumer");
    let member = verified_member_with_undefined(&object, b"_runtime_symbol");
    let core = core_closure(producer, member);
    let native = native_surface(producer, Vec::new(), Vec::new());
    let verified = verify_source_external_requirements_v1(core, native).unwrap();
    assert!(verified.source_external_requirements().is_empty());
    assert_eq!(verified.remaining_external_candidates().len(), 1);
    assert_eq!(
        verified.remaining_external_candidates()[0].symbol(),
        b"_runtime_symbol"
    );
}

#[test]
fn carries_the_exact_library_requirement_into_each_use() {
    let producer = ConeIdentity::SINGLE_FILE;
    let library = CborIdentityRecord::from_key(NativeLinkRequirementKey::target_default(
        CanonicalNativeLibraryName::new("sample").unwrap(),
    ))
    .unwrap();
    let object = fixture_for_producer(producer, "libraryConsumer");
    let member = verified_member_with_undefined(&object, b"_library_symbol");
    let core = core_closure(producer, member);
    let native = native_surface(
        producer,
        vec![contract_record(
            producer,
            "libraryDeclaration",
            "library_symbol",
            NativeLibraryBinding::Requirement(library.id()),
        )],
        vec![library.clone()],
    );

    let verified = verify_source_external_requirements_v1(core, native).unwrap();

    assert_eq!(
        verified.source_external_requirements()[0]
            .requirement()
            .library()
            .requirement(),
        Some(&library)
    );
}

#[test]
fn retains_declared_contracts_that_have_no_object_use() {
    let producer = ConeIdentity::SINGLE_FILE;
    let object = fixture_for_producer(producer, "unusedNativeDeclaration");
    let member = verified_member_without_relocations(&object);
    let core = core_closure(producer, member);
    let native = native_surface(
        producer,
        vec![contract_record(
            producer,
            "unusedDeclaration",
            "unused_native",
            NativeLibraryBinding::DefaultNativeNamespace,
        )],
        Vec::new(),
    );

    let verified = verify_source_external_requirements_v1(core, native).unwrap();

    assert!(verified.source_external_requirements().is_empty());
    assert_eq!(verified.native_requirements().contracts().len(), 1);
}

#[test]
fn rejects_a_native_surface_from_another_producer() {
    let producer = ConeIdentity::SINGLE_FILE;
    let object = fixture_for_producer(producer, "producerMismatch");
    let member = verified_member_without_relocations(&object);
    let core = core_closure(producer, member);
    let native = native_surface(ConeIdentity::CORE, Vec::new(), Vec::new());

    assert_eq!(
        verify_source_external_requirements_v1(core, native),
        Err(SourceExternalRequirementValidationError::ProducerMismatch {
            object: producer,
            native: ConeIdentity::CORE,
        })
    );
}

pub(in crate::link_object) fn core_closure(
    producer: ConeIdentity,
    member: crate::VerifiedMemberObjectRelocationIndexV1,
) -> VerifiedCoreStrongRequirementClosureV1 {
    let strong = verify_current_cone_strong_relocation_closure_v1(vec![member]).unwrap();
    let bridges = StrongExternalLirBridgeSurfaceV1::try_new(producer, Vec::new()).unwrap();
    let core_fixture =
        super::super::symbol_verification::tests::fixture_named("nativeRequirementCoreOwner");
    let core_strong = verify_current_cone_strong_relocation_closure_v1(vec![
        super::super::strong_relocation_closure::tests::verified_member_without_relocations(
            &core_fixture,
        ),
    ])
    .unwrap();
    let core_owners =
        crate::CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(&core_strong)
            .unwrap();
    verify_core_strong_requirements_v1(
        LirTargetProfile::DARWIN_AARCH64,
        strong,
        bridges,
        core_owners,
    )
    .unwrap()
}

pub(in crate::link_object) fn native_surface(
    producer: ConeIdentity,
    contracts: Vec<NativeExternalContractRecord>,
    libraries: Vec<scoop_lir::CanonicalNativeLibraryRequirementV1>,
) -> CanonicalNativeExternalRequirementSurfaceV1 {
    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_native_contracts(contracts).unwrap();
    canonical.set_native_link_requirements(libraries).unwrap();
    let foundation = OdrFreeLirFoundation::try_new(producer, canonical).unwrap();
    CanonicalNativeExternalRequirementSurfaceV1::from_foundation(
        LirTargetProfile::DARWIN_AARCH64,
        &foundation,
    )
    .unwrap()
}

pub(in crate::link_object) fn contract_record(
    producer: ConeIdentity,
    declaration: &str,
    symbol: &str,
    library: NativeLibraryBinding,
) -> NativeExternalContractRecord {
    NativeExternalContractRecord::new(
        source_contract(producer, declaration),
        NativeExternalSymbolKey::darwin_macho_external(&SourceNativeSymbol::new(symbol).unwrap())
            .unwrap(),
        NativeExternalContract::c_function(library, signature()),
    )
    .unwrap()
}

fn source_contract(
    producer: ConeIdentity,
    declaration: &str,
) -> PersistentSourceNativeExternalContractId {
    let declaration = SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            producer,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(declaration).unwrap(),
        0,
        None,
        Vec::new(),
    );
    PersistentSourceNativeExternalContractId::from_key(
        &SourceNativeExternalContractKey::function(&declaration).unwrap(),
    )
    .unwrap()
}

fn signature() -> CanonicalCAbiFunctionSignature {
    CanonicalCAbiFunctionSignature::cdecl(Vec::new(), CanonicalCAbiReturn::Void)
}
