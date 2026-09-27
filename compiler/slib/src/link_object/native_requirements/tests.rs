use scoop_identity::{
    CanonicalCAbiFunctionSignature, CanonicalCAbiReturn, CanonicalCStorageType,
    CanonicalIdentifier, CanonicalNativeLibraryName, CborIdentityRecord, ConeIdentity,
    CoreBuiltinNominal, DeclarationScope, DefinitionOwnerChain, ExactTypeKey,
    NativeExternalContract, NativeExternalContractRecord, NativeExternalSymbolKey,
    NativeLibraryBinding, NativeLinkRequirementKey, PackagePath, PersistentExactTypeId,
    PersistentSourceNativeExternalContractId, SourceDeclarationKey, SourceDeclarationSite,
    SourceNativeExternalContractKey, SourceNativeSymbol,
};
use scoop_lir::{
    CanonicalLirFoundation, CanonicalNativeExternalRequirementSurfaceV1, ConeLirFoundation,
    LirTargetProfile,
};

use super::super::strong_relocation_closure::tests::{
    verified_member_with_undefined, verified_member_with_undefined_form,
    verified_member_without_relocations,
};
use super::super::symbol_verification::tests::fixture_for_producer;
use super::*;
use crate::{
    verify_current_cone_strong_relocation_closure_v1, verify_dependency_strong_requirements_v1,
};

#[test]
fn resolves_source_extern_uses_and_preserves_unclassified_externals() {
    let producer = ConeIdentity::SINGLE_FILE;
    let object = fixture_for_producer(producer, "nativeConsumer");
    let member = verified_member_with_undefined(&object, b"_native");
    let core = dependency_closure(member);
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
    let core = dependency_closure(member);
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
fn admits_tlvp_relocations_only_for_typed_tls_contracts() {
    let producer = ConeIdentity::SINGLE_FILE;
    let storage = CanonicalCStorageType::Boolean {
        exact_type: unit_exact_type(),
    };
    let tls_contract =
        NativeExternalContract::mutable_tls(NativeLibraryBinding::DefaultNativeNamespace, storage);
    let tls_record =
        contract_record_with_contract(producer, "tlsDeclaration", "native_tls", tls_contract);
    let object = fixture_for_producer(producer, "nativeTlsConsumer");
    let member = verified_member_with_undefined_form(
        &object,
        b"_native_tls",
        VerifiedDarwinArm64RelocationFormV1::TlvpLoadPage21,
    );
    let verified = verify_source_external_requirements_v1(
        dependency_closure(member),
        native_surface(producer, vec![tls_record], Vec::new()),
    )
    .unwrap();
    assert_eq!(verified.source_external_requirements().len(), 1);

    let data_contract =
        NativeExternalContract::mutable_data(NativeLibraryBinding::DefaultNativeNamespace, storage);
    let data_record =
        contract_record_with_contract(producer, "dataDeclaration", "native_data", data_contract);
    let fingerprint = data_record.fingerprint();
    let object = fixture_for_producer(producer, "nativeDataConsumer");
    let member = verified_member_with_undefined_form(
        &object,
        b"_native_data",
        VerifiedDarwinArm64RelocationFormV1::TlvpLoadPageOffset12,
    );
    assert_eq!(
        verify_source_external_requirements_v1(
            dependency_closure(member),
            native_surface(producer, vec![data_record], Vec::new()),
        ),
        Err(
            SourceExternalRequirementValidationError::TlvpRelocationRequiresTlsContract {
                contract: fingerprint,
                form: VerifiedDarwinArm64RelocationFormV1::TlvpLoadPageOffset12,
            }
        )
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
    let core = dependency_closure(member);
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
    let core = dependency_closure(member);
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
    let core = dependency_closure(member);
    let native = native_surface(ConeIdentity::CORE, Vec::new(), Vec::new());

    assert_eq!(
        verify_source_external_requirements_v1(core, native),
        Err(SourceExternalRequirementValidationError::ProducerMismatch {
            object: producer,
            native: ConeIdentity::CORE,
        })
    );
}

pub(in crate::link_object) fn dependency_closure(
    member: crate::VerifiedMemberObjectRelocationIndexV1,
) -> VerifiedCrossConeStrongRequirementClosureV1 {
    let strong = verify_current_cone_strong_relocation_closure_v1(vec![member]).unwrap();

    verify_dependency_strong_requirements_v1(LirTargetProfile::DARWIN_AARCH64, strong, &[]).unwrap()
}

pub(in crate::link_object) fn native_surface(
    producer: ConeIdentity,
    contracts: Vec<NativeExternalContractRecord>,
    libraries: Vec<scoop_lir::CanonicalNativeLibraryRequirementV1>,
) -> CanonicalNativeExternalRequirementSurfaceV1 {
    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_native_contracts(contracts).unwrap();
    canonical.set_native_link_requirements(libraries).unwrap();
    let foundation = ConeLirFoundation::try_new(producer, canonical).unwrap();
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
    contract_record_with_contract(
        producer,
        declaration,
        symbol,
        NativeExternalContract::c_function(library, signature()),
    )
}

fn contract_record_with_contract(
    producer: ConeIdentity,
    declaration: &str,
    symbol: &str,
    contract: NativeExternalContract,
) -> NativeExternalContractRecord {
    NativeExternalContractRecord::new(
        source_contract(producer, declaration),
        NativeExternalSymbolKey::darwin_macho_external(&SourceNativeSymbol::new(symbol).unwrap())
            .unwrap(),
        contract,
    )
    .unwrap()
}

fn unit_exact_type() -> PersistentExactTypeId {
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        CoreBuiltinNominal::Unit.identity_record().id(),
    ))
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
