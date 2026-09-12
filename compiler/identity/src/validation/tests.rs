use scoop_wire::budget::COLLECTION_ELEMENT_BYTES;
use scoop_wire::{
    BudgetMeter, DecodeLimits, ResourceKind, WireErrorKind, WirePath, decode_canonical, encode,
};

use super::*;
use crate::{
    CanonicalCAbiFunctionSignature, CanonicalCAbiReturn, CanonicalCAbiSignatureFingerprintRecord,
    CanonicalIdentifier, CborIdentityRecord, ConeCoordinate, ConeIdentity, DeclarationScope,
    DecodedCanonicalCAbiSignatureFingerprintRecord, DecodedCborIdentityRecord, DecodedExactTypeKey,
    DecodedNativeExternalContractRecord, DecodedSourceDeclarationKey,
    DecodedSourceNativeExternalContractRecord, DefinitionOwnerChain, ExactTypeKey,
    NativeExternalContract, NativeExternalContractRecord, NativeExternalSymbolKey,
    NativeLibraryBinding, PackagePath, PersistentExactTypeId, PersistentFunctionId,
    PersistentSourceNativeExternalContractId, PersistentTypeId, SourceCAbiFunctionSignature,
    SourceCAbiReturn, SourceCallingConvention, SourceDeclarationKey, SourceDeclarationSite,
    SourceExternFunctionAbi, SourceNativeExternalContract, SourceNativeExternalContractKey,
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
    let mut meter = BudgetMeter::new(DecodeLimits::default());
    let records = graph
        .records::<PersistentTypeId, SourceDeclarationKey>(
            IdentityLayer::Hir,
            &mut meter,
            &WirePath::root(),
        )
        .unwrap();
    assert_eq!(records, vec![source_type_record()]);
}

#[test]
fn canonical_record_materialization_precharges_exact_slot_budget() {
    let decoded = decoded_source_type();
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    pending.register(IdentityLayer::Hir, &decoded).unwrap();
    pending.resolve(&decoded).unwrap();
    let graph = pending.finish().unwrap();

    for (limit, accepted) in [
        (COLLECTION_ELEMENT_BYTES - 1, false),
        (COLLECTION_ELEMENT_BYTES, true),
        (COLLECTION_ELEMENT_BYTES + 1, true),
    ] {
        let mut meter = BudgetMeter::new(DecodeLimits {
            logical_heap_bytes: limit,
            ..DecodeLimits::default()
        });
        let result = graph.records::<PersistentTypeId, SourceDeclarationKey>(
            IdentityLayer::Hir,
            &mut meter,
            &WirePath::root().field(2),
        );
        assert_eq!(result.is_ok(), accepted);
        if !accepted {
            assert!(matches!(
                result,
                Err(IdentityValidationError::Resource(ref error))
                    if error.kind() == &WireErrorKind::LimitExceeded {
                        resource: ResourceKind::LogicalHeapBytes,
                        limit,
                        observed: COLLECTION_ELEMENT_BYTES,
                    }
            ));
        }
    }
}

#[test]
fn canonical_record_materialization_shares_the_validated_key() {
    let decoded = decoded_source_type();
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    pending.register(IdentityLayer::Hir, &decoded).unwrap();
    pending.resolve(&decoded).unwrap();
    let graph = pending.finish().unwrap();
    let mut meter = BudgetMeter::new(DecodeLimits::default());

    let first = graph
        .records::<PersistentTypeId, SourceDeclarationKey>(
            IdentityLayer::Hir,
            &mut meter,
            &WirePath::root(),
        )
        .unwrap();
    let second = graph
        .records::<PersistentTypeId, SourceDeclarationKey>(
            IdentityLayer::Hir,
            &mut meter,
            &WirePath::root(),
        )
        .unwrap();

    assert!(std::ptr::eq(first[0].key(), second[0].key()));
}

#[test]
fn identity_hash_is_charged_before_registration() {
    let decoded = decoded_source_type();
    let hash_work = {
        let mut meter = BudgetMeter::new(DecodeLimits::default());
        let mut pending = PendingIdentityValidation::with_meter(&mut meter);
        pending.register(IdentityLayer::Hir, &decoded).unwrap();
        meter.usage().validation_work_units
    };
    assert_eq!(hash_work, 2);

    for (limit, accepted) in [
        (hash_work - 1, false),
        (hash_work, true),
        (hash_work + 1, true),
    ] {
        let mut meter = BudgetMeter::new(DecodeLimits {
            validation_work_units: limit,
            ..DecodeLimits::default()
        });
        let result = {
            let mut pending = PendingIdentityValidation::with_meter(&mut meter);
            pending.register(IdentityLayer::Hir, &decoded)
        };
        assert_eq!(result.is_ok(), accepted);
        if !accepted {
            assert!(matches!(
                result,
                Err(IdentityValidationError::Resource(ref error))
                    if error.kind() == &WireErrorKind::LimitExceeded {
                        resource: ResourceKind::ValidationWorkUnits,
                        limit,
                        observed: hash_work,
                    }
            ));
        }
    }
}

#[test]
fn identity_resolution_working_copy_has_inclusive_owned_byte_boundaries() {
    let decoded = decoded_source_type();
    let expected = {
        let mut meter = BudgetMeter::new(DecodeLimits::default());
        let mut pending = PendingIdentityValidation::with_meter(&mut meter);
        pending.register_authority(ConeIdentity::CORE).unwrap();
        pending.register(IdentityLayer::Hir, &decoded).unwrap();
        pending.resolve(&decoded).unwrap();
        meter.usage().owned_bytes
    };
    assert!(expected > 0);

    for (limit, accepted) in [
        (expected - 1, false),
        (expected, true),
        (expected + 1, true),
    ] {
        let mut meter = BudgetMeter::new(DecodeLimits {
            owned_bytes: limit,
            ..DecodeLimits::default()
        });
        let result = (|| {
            let mut pending = PendingIdentityValidation::with_meter(&mut meter);
            pending.register_authority(ConeIdentity::CORE)?;
            pending.register(IdentityLayer::Hir, &decoded)?;
            pending.resolve(&decoded)
        })();
        assert_eq!(result.is_ok(), accepted);
        if accepted {
            assert_eq!(meter.usage().owned_bytes, expected);
        } else {
            assert!(matches!(
                result,
                Err(IdentityValidationError::Resource(ref error))
                    if error.kind() == &WireErrorKind::LimitExceeded {
                        resource: ResourceKind::OwnedBytes,
                        limit,
                        observed: expected,
                    }
            ));
        }
    }
}

#[test]
fn c_abi_leaf_hashes_are_precharged_at_each_transaction_phase() {
    let record = CanonicalCAbiSignatureFingerprintRecord::new(
        CanonicalCAbiFunctionSignature::cdecl(Vec::new(), CanonicalCAbiReturn::Void),
    )
    .unwrap();
    let decoded = decode_canonical::<DecodedCanonicalCAbiSignatureFingerprintRecord>(
        &encode(&record).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
    let hash_length = decoded.candidate_hash_stream_length().unwrap();
    let hash_work = (hash_length + 72) / 64;
    let expected = {
        let mut meter = BudgetMeter::new(DecodeLimits::default());
        let mut pending = PendingIdentityValidation::with_meter(&mut meter);
        pending
            .register_c_abi_signature(IdentityLayer::Lir, &decoded)
            .unwrap();
        pending.resolve_c_abi_signature(&decoded).unwrap();
        pending.finish().unwrap();
        meter.usage().validation_work_units
    };
    assert!(expected > hash_work * 3 + 1);

    for (limit, accepted) in [
        (expected - 1, false),
        (expected, true),
        (expected + 1, true),
    ] {
        let mut meter = BudgetMeter::new(DecodeLimits {
            validation_work_units: limit,
            ..DecodeLimits::default()
        });
        let result = (|| {
            let mut pending = PendingIdentityValidation::with_meter(&mut meter);
            pending.register_c_abi_signature(IdentityLayer::Lir, &decoded)?;
            pending.resolve_c_abi_signature(&decoded)?;
            pending.finish()
        })();
        assert_eq!(result.is_ok(), accepted);
        if accepted {
            assert_eq!(meter.usage().validation_work_units, expected);
        } else {
            assert!(matches!(
                result,
                Err(IdentityValidationError::Resource(ref error))
                    if error.kind() == &WireErrorKind::LimitExceeded {
                        resource: ResourceKind::ValidationWorkUnits,
                        limit,
                        observed: expected,
                    }
            ));
        }
    }
}

#[test]
fn native_contract_leaf_hashes_are_precharged_without_a_source_dependency() {
    let source = source_native_contract_record();
    let symbol = NativeExternalSymbolKey::darwin_macho_external(
        &SourceNativeSymbol::new("native_entry").unwrap(),
    )
    .unwrap();
    let contract = NativeExternalContract::c_function(
        NativeLibraryBinding::DefaultNativeNamespace,
        CanonicalCAbiFunctionSignature::cdecl(Vec::new(), CanonicalCAbiReturn::Void),
    );
    let record = NativeExternalContractRecord::new(source.id(), symbol, contract).unwrap();
    let decoded = decode_canonical::<DecodedNativeExternalContractRecord>(
        &encode(&record).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
    let [symbol_length, fingerprint_length] =
        NativeExternalContractRecord::hash_stream_lengths(record.symbol_key(), record.contract())
            .unwrap();
    let hash_work = (symbol_length + 72) / 64 + (fingerprint_length + 72) / 64;
    let expected = {
        let mut meter = BudgetMeter::new(DecodeLimits::default());
        let mut pending = PendingIdentityValidation::with_meter(&mut meter);
        pending
            .register_native_external_contract(IdentityLayer::Lir, &decoded)
            .unwrap();
        pending.resolve_native_external_contract(&decoded).unwrap();
        pending.finish().unwrap();
        meter.usage().validation_work_units
    };
    assert!(expected > hash_work * 3 + 1);

    for (limit, accepted) in [
        (expected - 1, false),
        (expected, true),
        (expected + 1, true),
    ] {
        let mut meter = BudgetMeter::new(DecodeLimits {
            validation_work_units: limit,
            ..DecodeLimits::default()
        });
        let result = (|| {
            let mut pending = PendingIdentityValidation::with_meter(&mut meter);
            pending.register_native_external_contract(IdentityLayer::Lir, &decoded)?;
            pending.resolve_native_external_contract(&decoded)?;
            pending.finish()
        })();
        assert_eq!(result.is_ok(), accepted);
        if accepted {
            assert_eq!(meter.usage().validation_work_units, expected);
        } else {
            assert!(matches!(
                result,
                Err(IdentityValidationError::Resource(ref error))
                    if error.kind() == &WireErrorKind::LimitExceeded {
                        resource: ResourceKind::ValidationWorkUnits,
                        limit,
                        observed: expected,
                    }
            ));
        }
    }
}

#[test]
fn metered_identity_graph_charges_each_edge_before_stable_kahn() {
    let decoded = decoded_source_type();
    let limits = DecodeLimits {
        decoded_edges: 0,
        ..DecodeLimits::default()
    };
    let mut meter = BudgetMeter::new(limits);
    let error = {
        let mut pending = PendingIdentityValidation::with_meter(&mut meter);
        pending.register_authority(ConeIdentity::CORE).unwrap();
        pending.register(IdentityLayer::Hir, &decoded).unwrap();
        pending.resolve(&decoded).unwrap_err()
    };
    assert!(matches!(
        error,
        IdentityValidationError::Resource(ref error)
            if error.kind() == &WireErrorKind::LimitExceeded {
                resource: ResourceKind::DecodedEdges,
                limit: 0,
                observed: 1,
            }
    ));

    let (expected_heap, expected_work) = {
        let mut meter = BudgetMeter::new(DecodeLimits::default());
        let mut pending = PendingIdentityValidation::with_meter(&mut meter);
        pending.register_authority(ConeIdentity::CORE).unwrap();
        pending.register(IdentityLayer::Hir, &decoded).unwrap();
        pending.resolve(&decoded).unwrap();
        pending.finish().unwrap();
        (
            meter.usage().logical_heap_bytes,
            meter.usage().validation_work_units,
        )
    };
    assert!(expected_heap > 192);
    for (limit, accepted) in [
        (expected_heap - 1, false),
        (expected_heap, true),
        (expected_heap + 1, true),
    ] {
        let mut meter = BudgetMeter::new(DecodeLimits {
            logical_heap_bytes: limit,
            ..DecodeLimits::default()
        });
        let result = (|| {
            let mut pending = PendingIdentityValidation::with_meter(&mut meter);
            pending.register_authority(ConeIdentity::CORE)?;
            pending.register(IdentityLayer::Hir, &decoded)?;
            pending.resolve(&decoded)?;
            pending.finish()
        })();
        assert_eq!(result.is_ok(), accepted);
        if accepted {
            assert_eq!(meter.usage().logical_heap_bytes, expected_heap);
            assert_eq!(meter.usage().decoded_edges, 1);
            assert_eq!(meter.usage().validation_work_units, expected_work);
        } else {
            assert!(matches!(
                result,
                Err(IdentityValidationError::Resource(ref error))
                    if error.kind() == &WireErrorKind::LimitExceeded {
                        resource: ResourceKind::LogicalHeapBytes,
                        limit,
                        observed: expected_heap,
                    }
            ));
        }
    }
}

#[test]
fn stable_kahn_uses_no_input_driven_recursion() {
    const NODE_COUNT: u64 = 16_384;
    let mut candidates = HashMap::new();
    let mut nodes = Vec::new();
    for index in 0..NODE_COUNT {
        let mut bytes = [0_u8; 32];
        bytes[24..].copy_from_slice(&index.to_be_bytes());
        let node = IdentityNode {
            kind: "test",
            bytes,
        };
        nodes.push(node);
        candidates.insert(
            node,
            Candidate {
                trusted_id: Arc::new(bytes),
                layer: Some(IdentityLayer::Hir),
                resolved: true,
                dependency_count: u64::from(index != 0),
                dependents: Vec::new(),
            },
        );
    }
    for pair in nodes.windows(2) {
        candidates
            .get_mut(&pair[0])
            .unwrap()
            .dependents
            .push(pair[1]);
    }

    assert_eq!(
        find_cycle(&mut candidates, NODE_COUNT, &WirePath::default()).unwrap(),
        None
    );
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
    let same_key = <ValidatedIdentityGraph as PersistentKeyResolver<
        PersistentSourceNativeExternalContractId,
        SourceNativeExternalContractKey,
    >>::resolve_key(&mut graph, decoded_contract.decoded_id())
    .unwrap();
    assert!(std::sync::Arc::ptr_eq(&key, &same_key));
    assert_eq!(key.as_ref(), &contract.key());
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
