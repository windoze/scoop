use scoop_wire::{DecodeLimits, decode_canonical, encode};

use super::*;
use crate::{
    CanonicalIdentifier, CborIdentityRecord, ConeCoordinate, ConeIdentity, DeclarationScope,
    DecodedCborIdentityRecord, DecodedExactTypeKey, DecodedSourceDeclarationKey,
    DecodedSourceNativeExternalContractRecord, DefinitionOwnerChain, ExactTypeKey, PackagePath,
    PersistentExactTypeId, PersistentFunctionId, PersistentSourceNativeExternalContractId,
    PersistentTypeId, SourceCAbiFunctionSignature, SourceCAbiReturn, SourceCallingConvention,
    SourceDeclarationKey, SourceDeclarationSite, SourceExternFunctionAbi,
    SourceNativeExternalContract, SourceNativeExternalContractKey,
    SourceNativeExternalContractRecord, SourceNativeLibraryBinding, SourceNativeSymbol,
    SourceNominalKind,
};

fn source_type_record() -> CborIdentityRecord<PersistentTypeId, SourceDeclarationKey> {
    let cone = ConeCoordinate::reserved_core().identity().unwrap();
    let site = SourceDeclarationSite::new(
        cone,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let name = CanonicalIdentifier::new("Widget").unwrap();
    CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
        site,
        name,
        SourceNominalKind::Struct,
        0,
    ))
    .unwrap()
}

fn decoded_source_type() -> DecodedCborIdentityRecord<PersistentTypeId, DecodedSourceDeclarationKey>
{
    decode_canonical(
        &encode(&source_type_record()).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap()
}

fn exact_type_record() -> CborIdentityRecord<PersistentExactTypeId, ExactTypeKey> {
    CborIdentityRecord::from_key(ExactTypeKey::Nominal(source_type_record().id())).unwrap()
}

fn decoded_exact_type() -> DecodedCborIdentityRecord<PersistentExactTypeId, DecodedExactTypeKey> {
    decode_canonical(
        &encode(&exact_type_record()).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap()
}

fn source_function_record() -> CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey> {
    CborIdentityRecord::from_key(SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("nativeEntry").unwrap(),
        0,
        None,
        Vec::new(),
    ))
    .unwrap()
}

fn source_native_contract_record() -> SourceNativeExternalContractRecord {
    let function = source_function_record();
    SourceNativeExternalContractRecord::new(
        SourceNativeExternalContractKey::function(function.key()).unwrap(),
        SourceNativeExternalContract::Function {
            symbol: SourceNativeSymbol::new("native_entry").unwrap(),
            library: SourceNativeLibraryBinding::DefaultNativeNamespace,
            abi: SourceExternFunctionAbi::C(SourceCAbiFunctionSignature::new(
                Vec::new(),
                SourceCAbiReturn::Void,
            )),
            calling_convention: SourceCallingConvention::Cdecl,
        },
    )
    .unwrap()
}

#[test]
fn commits_a_complete_identity_transaction() {
    let decoded = decoded_source_type();
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    pending.register(IdentityLayer::Hir, &decoded).unwrap();
    pending.resolve(&decoded).unwrap();

    let graph = pending.finish().unwrap();
    let records = graph
        .records::<PersistentTypeId, SourceDeclarationKey>(IdentityLayer::Hir)
        .unwrap();
    assert_eq!(records, vec![source_type_record()]);
}

#[test]
fn commits_a_source_native_contract_in_the_same_transaction() {
    let function = source_function_record();
    let contract = source_native_contract_record();
    let decoded_function = decode_canonical::<
        DecodedCborIdentityRecord<PersistentFunctionId, DecodedSourceDeclarationKey>,
    >(&encode(&function).unwrap(), DecodeLimits::default())
    .unwrap();
    let decoded_contract = decode_canonical::<DecodedSourceNativeExternalContractRecord>(
        &encode(&contract).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    pending
        .register(IdentityLayer::Hir, &decoded_function)
        .unwrap();
    pending
        .register_source_native_contract(IdentityLayer::Hir, &decoded_contract)
        .unwrap();
    pending.resolve(&decoded_function).unwrap();
    pending
        .resolve_source_native_contract(&decoded_contract)
        .unwrap();

    let mut graph = pending.finish().unwrap();
    let key = <ValidatedIdentityGraph as PersistentKeyResolver<
        PersistentSourceNativeExternalContractId,
        SourceNativeExternalContractKey,
    >>::resolve_key(&mut graph, decoded_contract.decoded_id())
    .unwrap();
    assert_eq!(key, contract.key());
}

#[test]
fn duplicate_registration_poisons_the_transaction() {
    let decoded = decoded_source_type();
    let mut pending = PendingIdentityValidation::new();
    pending.register(IdentityLayer::Hir, &decoded).unwrap();
    assert!(matches!(
        pending.register(IdentityLayer::Mir, &decoded),
        Err(IdentityValidationError::DuplicateIdentity { .. })
    ));
    assert!(matches!(
        pending.finish(),
        Err(IdentityValidationError::Poisoned)
    ));
}

#[test]
fn unresolved_registration_cannot_commit() {
    let decoded = decoded_source_type();
    let mut pending = PendingIdentityValidation::new();
    pending.register(IdentityLayer::Hir, &decoded).unwrap();
    assert!(matches!(
        pending.finish(),
        Err(IdentityValidationError::UnresolvedIdentity { .. })
    ));
}

#[test]
fn missing_reference_poisons_the_transaction() {
    let decoded = decoded_exact_type();
    let mut pending = PendingIdentityValidation::new();
    pending.register(IdentityLayer::Hir, &decoded).unwrap();

    let error = pending.resolve(&decoded).unwrap_err();
    assert!(matches!(
        error,
        IdentityValidationError::InvalidRecord { ref reason, .. }
            if reason.contains("missing type identity")
    ));
    assert!(matches!(
        pending.finish(),
        Err(IdentityValidationError::Poisoned)
    ));
}

#[test]
fn earlier_layer_cannot_reference_a_future_layer_candidate() {
    let source = decoded_source_type();
    let exact = decoded_exact_type();
    let mut pending = PendingIdentityValidation::new();
    pending.register(IdentityLayer::Mir, &source).unwrap();
    pending.register(IdentityLayer::Hir, &exact).unwrap();

    let error = pending.resolve(&exact).unwrap_err();
    assert!(matches!(
        error,
        IdentityValidationError::InvalidRecord { ref reason, .. }
            if reason.contains("HIR identity references future MIR type identity")
    ));
}

#[test]
fn registration_closes_when_resolution_starts() {
    let source = decoded_source_type();
    let exact = decoded_exact_type();
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    pending.register(IdentityLayer::Hir, &source).unwrap();
    pending.resolve(&source).unwrap();

    assert_eq!(
        pending.register(IdentityLayer::Hir, &exact),
        Err(IdentityValidationError::RegistrationClosed)
    );
}

#[test]
fn registration_rejects_an_id_that_does_not_match_its_decoded_key() {
    let mut bytes = encode(&source_type_record()).unwrap();
    assert_eq!(&bytes[..4], &[0xa2, 1, 0x58, 0x20]);
    bytes[4] ^= 1;
    let decoded = decode_canonical::<
        DecodedCborIdentityRecord<PersistentTypeId, DecodedSourceDeclarationKey>,
    >(&bytes, DecodeLimits::default())
    .unwrap();
    let mut pending = PendingIdentityValidation::new();

    assert!(matches!(
        pending.register(IdentityLayer::Hir, &decoded),
        Err(IdentityValidationError::IdentityMismatch { .. })
    ));
}

#[test]
fn semantic_import_reuses_world_ids_across_origins() {
    let graph = validated_source_type_graph();
    let fingerprint = SemanticOriginFingerprint::new([1; 32], [2; 32], [3; 32]);
    let mut session = SemanticIdentitySession::new();

    let (first, _, _) = session
        .import(ConeIdentity::CORE, fingerprint, &graph)
        .unwrap()
        .into_parts();
    let first_id = first.get(source_type_record().id()).unwrap();
    let (second, _, _) = session
        .import(ConeIdentity::SINGLE_FILE, fingerprint, &graph)
        .unwrap()
        .into_parts();
    let second_id = second.get(source_type_record().id()).unwrap();

    assert_eq!(first_id, second_id);
    assert_eq!(first_id.persistent(), source_type_record().id());
    assert_eq!(session.origin_count(), 2);
    assert_eq!(session.entity_count(), 1);
}

#[test]
fn semantic_import_rejects_origin_conflicts_without_partial_commit() {
    let graph = validated_source_type_graph();
    let mut session = SemanticIdentitySession::new();
    session
        .import(
            ConeIdentity::CORE,
            SemanticOriginFingerprint::new([1; 32], [2; 32], [3; 32]),
            &graph,
        )
        .unwrap();
    let origin_count = session.origin_count();
    let entity_count = session.entity_count();

    let error = match session.import(
        ConeIdentity::CORE,
        SemanticOriginFingerprint::new([9; 32], [2; 32], [3; 32]),
        &graph,
    ) {
        Ok(_) => panic!("conflicting origin fingerprints must be rejected"),
        Err(error) => error,
    };

    assert_eq!(
        error,
        SemanticIdentityImportError::OriginConflict {
            origin: ConeIdentity::CORE
        }
    );
    assert_eq!(session.origin_count(), origin_count);
    assert_eq!(session.entity_count(), entity_count);
}

fn validated_source_type_graph() -> ValidatedIdentityGraph {
    let decoded = decoded_source_type();
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    pending.register(IdentityLayer::Hir, &decoded).unwrap();
    pending.resolve(&decoded).unwrap();
    pending.finish().unwrap()
}
