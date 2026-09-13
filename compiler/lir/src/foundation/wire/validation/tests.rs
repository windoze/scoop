use scoop_identity::{
    CDataPointee, CPointerStorage, CallableBodyKey, CallableMaterializationContext,
    CallbackApplicationKey, CallbackMode, CallbackParameterIndex, CallbackRegistrationKey,
    CanonicalCAbiFunctionSignature, CanonicalCAbiParameter, CanonicalCAbiReturn,
    CanonicalCAbiSignatureFingerprintRecord, CanonicalCStorageType, CanonicalIdentifier,
    CborIdentityRecord, ConeCoordinate, ConeIdentity, DeclarationScope,
    DecodedCallbackApplicationKey, DecodedCallbackRegistrationKey, DecodedCborIdentityRecord,
    DecodedExactTypeKey, DecodedSourceDeclarationKey, DecodedSourceNativeExternalContractRecord,
    DefinitionOwnerChain, Effect, ExactTypeKey, GeneratedBridgeAtomKey, GeneratedBridgeAtomRoleKey,
    GeneratedBridgeUnitId, GeneratedBridgeUnitKey, IdentityLayer, LinkageClass,
    NativeExternalContract, NativeExternalContractRecord, NativeExternalSymbolKey,
    NativeLibraryBinding, ObjectDefinitionPlanKey, PackagePath, PendingIdentityValidation,
    PersistentCallbackApplicationId, PersistentCallbackRegistrationId, PersistentExactTypeId,
    PersistentFunctionId, PersistentSymbolKey, PersistentSymbolRequest,
    PersistentSymbolRequestTable, PersistentTypeId, RuntimeIdentityRecord, RuntimeTypeId,
    SafepointId, SafepointSiteKey, SafepointSiteRole, SignatureCallableShape, SignatureTypeKey,
    SourceCAbiFunctionSignature, SourceCAbiReturn, SourceCallingConvention, SourceDeclarationKey,
    SourceDeclarationSite, SourceExternFunctionAbi, SourceNativeExternalContract,
    SourceNativeExternalContractKey, SourceNativeExternalContractRecord,
    SourceNativeLibraryBinding, SourceNativeSymbol, SourceNominalKind,
    StrongCallableDefinitionOwner, StrongDefinitionEntity, StrongDefinitionRole,
    StructuralDefinitionPath, StructuralDefinitionSiteRole, StructuralPathSegment,
    ValidatedIdentityGraph,
};
use scoop_wire::{BudgetMeter, DecodeLimits, decode_canonical, encode};

use super::*;

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

#[test]
fn empty_foundation_validates_and_reencodes_identically() {
    let canonical = CanonicalLirFoundation::empty();
    let bytes = encode(&canonical).unwrap();
    let decoded =
        decode_canonical::<DecodedLirFoundation>(&bytes, DecodeLimits::default()).unwrap();
    let mut identities = graph_with_authorities(&[ConeIdentity::CORE]);

    let validated = decoded
        .validate(ConeIdentity::CORE, &mut identities, &mut meter())
        .unwrap();

    assert_eq!(validated.producer(), ConeIdentity::CORE);
    assert_eq!(validated.counts(), canonical.counts());
    assert_eq!(encode(&validated).unwrap(), bytes);
}

#[test]
fn validates_callback_safepoint_runtime_and_symbol_relations_atomically() {
    let (decoded, mut identities, bytes) = callback_fixture(&[0, 1], 0);

    let validated = decoded
        .validate(ConeIdentity::CORE, &mut identities, &mut meter())
        .unwrap();

    let counts = validated.counts();
    assert_eq!(counts.callable_bodies, 1);
    assert_eq!(counts.safepoint_sites, 2);
    assert_eq!(counts.runtime_types, 1);
    assert_eq!(counts.safepoints, 2);
    assert_eq!(counts.symbol_requests, 1);
    assert_eq!(counts.c_abi_signatures, 1);
    assert_eq!(counts.bridge_units, 1);
    assert_eq!(counts.callback_bridges, 1);
    assert_eq!(encode(&validated).unwrap(), bytes);
}

#[test]
fn rejects_noncanonical_mapping_order_after_all_relations_resolve() {
    let (mut decoded, mut identities, _) = callback_fixture(&[0, 1], 0);
    decoded.decoded.safepoints.swap(0, 1);

    assert!(matches!(
        decoded.validate(ConeIdentity::CORE, &mut identities, &mut meter()),
        Err(LirFoundationValidationError::NonCanonicalFoundation)
    ));
}

#[test]
fn rejects_a_gap_in_role_local_safepoint_ordinals() {
    let (decoded, mut identities, _) = callback_fixture(&[1], 0);

    assert!(matches!(
        decoded.validate(ConeIdentity::CORE, &mut identities, &mut meter()),
        Err(LirFoundationValidationError::SafepointRelation(
            SafepointRelationError::NonContiguousOrdinal {
                expected: 0,
                actual: 1,
                ..
            }
        ))
    ));
}

#[test]
fn rejects_an_unbounded_safepoint_ordinal_scan_through_the_work_budget() {
    let (decoded, mut identities, _) = callback_fixture(&[u32::MAX], 0);
    let prior_hash_work = (RuntimeTypeId::hash_stream_length().unwrap() + 72) / 64
        + (SafepointId::hash_stream_length().unwrap() + 72) / 64;
    let observed = prior_hash_work + u64::from(u32::MAX) + 1;

    let error = decoded
        .validate(ConeIdentity::CORE, &mut identities, &mut meter())
        .unwrap_err();
    assert!(matches!(
        error,
        LirFoundationValidationError::Resource(ref error)
            if error.kind() == &scoop_wire::WireErrorKind::LimitExceeded {
                resource: scoop_wire::ResourceKind::ValidationWorkUnits,
                limit: DecodeLimits::default().validation_work_units,
                observed,
            }
    ));
}

#[test]
fn rejects_a_safepoint_site_without_its_runtime_mapping() {
    let (mut decoded, mut identities, _) = callback_fixture(&[0], 0);
    decoded.decoded.safepoints.clear();

    assert!(matches!(
        decoded.validate(ConeIdentity::CORE, &mut identities, &mut meter()),
        Err(LirFoundationValidationError::SafepointRelation(
            SafepointRelationError::MissingMapping { .. }
        ))
    ));
}

#[test]
fn rejects_a_callback_unit_with_the_wrong_context_parameter() {
    let (decoded, mut identities, _) = callback_fixture(&[0], 1);

    assert!(matches!(
        decoded.validate(ConeIdentity::CORE, &mut identities, &mut meter()),
        Err(LirFoundationValidationError::BridgeRelation(
            BridgeRelationError::CallbackUnitMismatch { .. }
        ))
    ));
}

#[test]
fn validates_native_contract_and_outbound_bridge_relations() {
    let (decoded, mut identities, bytes) = native_contract_fixture(None);

    let validated = decoded
        .validate(ConeIdentity::CORE, &mut identities, &mut meter())
        .unwrap();

    assert_eq!(validated.counts().native_contracts, 1);
    assert_eq!(validated.counts().bridge_units, 1);
    assert_eq!(encode(&validated).unwrap(), bytes);
}

#[test]
fn rejects_a_missing_target_contract_for_a_hir_native_contract() {
    let (mut decoded, mut identities, _) = native_contract_fixture(None);
    decoded.decoded.native_contracts.clear();

    assert!(matches!(
        decoded.validate(ConeIdentity::CORE, &mut identities, &mut meter()),
        Err(LirFoundationValidationError::NativeContractRelation(
            NativeContractRelationError::MissingRecord { .. }
        ))
    ));
}

#[test]
fn rejects_a_bridge_atom_owned_by_another_cone() {
    let foreign = ConeCoordinate::new("example", "foreign", "0.1.0")
        .unwrap()
        .identity()
        .unwrap();
    let (decoded, mut identities, _) = native_contract_fixture(Some(foreign));

    assert!(matches!(
        decoded.validate(ConeIdentity::CORE, &mut identities, &mut meter()),
        Err(LirFoundationValidationError::Ownership(
            LirFoundationOwnershipError::ForeignBridgeAtom { actual, .. }
        )) if actual == foreign
    ));
}

#[test]
fn rejects_a_strong_definition_plan_owned_by_another_cone() {
    let foreign = ConeCoordinate::new("example", "foreign", "0.1.0")
        .unwrap()
        .identity()
        .unwrap();
    let plan = CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(
            foreign,
            StrongDefinitionEntity::cone_image(ConeIdentity::CORE),
            StrongDefinitionRole::ImageDescriptor,
        )
        .unwrap(),
    )
    .unwrap();
    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_definition_plans(vec![plan]).unwrap();
    let bytes = encode(&canonical).unwrap();
    let decoded =
        decode_canonical::<DecodedLirFoundation>(&bytes, DecodeLimits::default()).unwrap();
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    pending.register_authority(foreign).unwrap();
    decoded.register_identities(&mut pending).unwrap();
    decoded.resolve_identities(&mut pending).unwrap();
    let mut identities = pending.finish().unwrap();

    assert!(matches!(
        decoded.validate(ConeIdentity::CORE, &mut identities, &mut meter()),
        Err(LirFoundationValidationError::Ownership(
            LirFoundationOwnershipError::ForeignStrongDefinitionPlan { actual, .. }
        )) if actual == foreign
    ));
}

fn callback_fixture(
    safepoint_ordinals: &[u32],
    bridge_context_index: u32,
) -> (DecodedLirFoundation, ValidatedIdentityGraph, Vec<u8>) {
    let declaration_site = || {
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap()
    };
    let function_key = SourceDeclarationKey::function(
        declaration_site(),
        CanonicalIdentifier::new("run").unwrap(),
        0,
        None,
        Vec::new(),
    );
    let function: CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey> =
        CborIdentityRecord::from_key(function_key.clone()).unwrap();
    let nominal_key = SourceDeclarationKey::nominal(
        declaration_site(),
        CanonicalIdentifier::new("Value").unwrap(),
        SourceNominalKind::Class,
        0,
    );
    let nominal = CborIdentityRecord::from_key(nominal_key.clone()).unwrap();
    let exact = CborIdentityRecord::from_key(ExactTypeKey::Nominal(nominal.id())).unwrap();
    let context_exact = CborIdentityRecord::from_key(ExactTypeKey::RawPointer(exact.id())).unwrap();

    let context_signature =
        SignatureTypeKey::RawPointer(Box::new(SignatureTypeKey::Nominal(nominal.id())));
    let registration_key = CallbackRegistrationKey::new(
        scoop_identity::LexicalCallableParent::function(function.id()),
        StructuralDefinitionPath::from_first(
            StructuralPathSegment::new(StructuralDefinitionSiteRole::CallbackConversion, 0),
            [],
        ),
        SourceCAbiFunctionSignature::new(vec![context_signature], SourceCAbiReturn::Void),
        CallbackParameterIndex::new(0),
        SignatureCallableShape::new(
            Effect::Ordinary,
            None,
            Vec::new(),
            SignatureTypeKey::Nominal(nominal.id()),
        ),
        CallbackMode::Reusable,
    );
    let registration = CborIdentityRecord::from_key(registration_key.clone()).unwrap();
    let application_key = CallbackApplicationKey::new(
        &registration_key,
        CallableMaterializationContext::NoSubstitution,
    )
    .unwrap();
    let application = CborIdentityRecord::from_key(application_key).unwrap();

    let body = RuntimeIdentityRecord::from_key(&CallableBodyKey::strong(
        StrongCallableDefinitionOwner::Function(function.id()),
    ))
    .unwrap();
    let mut site_records = Vec::new();
    let mut mappings = Vec::new();
    for ordinal in safepoint_ordinals {
        let site = CborIdentityRecord::from_key(SafepointSiteKey::new(
            body.id(),
            SafepointSiteRole::ManagedCall,
            *ordinal,
        ))
        .unwrap();
        mappings.push(SafepointMappingRecord::new(site.id()).unwrap());
        site_records.push(site);
    }
    let context_storage = CanonicalCStorageType::DataPointer {
        exact_type: context_exact.id(),
        pointee: CDataPointee::OpaqueUnit,
        storage: CPointerStorage::Direct,
    };
    let signature =
        CanonicalCAbiSignatureFingerprintRecord::new(CanonicalCAbiFunctionSignature::cdecl(
            vec![CanonicalCAbiParameter::new(context_exact.id(), context_storage).unwrap()],
            CanonicalCAbiReturn::Void,
        ))
        .unwrap();
    let unit = CborIdentityRecord::from_key(GeneratedBridgeUnitKey::CallbackTrampoline {
        signature: signature.fingerprint(),
        context_index: CallbackParameterIndex::new(bridge_context_index),
    })
    .unwrap();
    let symbol_request = PersistentSymbolRequest::new(
        PersistentSymbolKey::CallableBody(body.id()),
        LinkageClass::ConeStrong,
    )
    .unwrap();

    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_callable_bodies(vec![body]).unwrap();
    canonical.set_safepoint_sites(site_records).unwrap();
    canonical
        .set_runtime_types(vec![RuntimeTypeMappingRecord::new(exact.id()).unwrap()])
        .unwrap();
    canonical.set_safepoints(mappings).unwrap();
    canonical.set_symbol_requests(PersistentSymbolRequestTable::new(vec![symbol_request]).unwrap());
    canonical
        .set_c_abi_signatures(vec![signature.clone()])
        .unwrap();
    canonical.set_bridge_units(vec![unit]).unwrap();
    canonical
        .set_callback_bridges(vec![CallbackBridgeRecord::new(
            application.id(),
            signature.fingerprint(),
            GeneratedBridgeUnitId::from_key(&GeneratedBridgeUnitKey::CallbackTrampoline {
                signature: signature.fingerprint(),
                context_index: CallbackParameterIndex::new(bridge_context_index),
            })
            .unwrap(),
        )])
        .unwrap();
    let bytes = encode(&canonical).unwrap();
    let decoded =
        decode_canonical::<DecodedLirFoundation>(&bytes, DecodeLimits::default()).unwrap();

    let decoded_function = decode_canonical::<
        DecodedCborIdentityRecord<PersistentFunctionId, DecodedSourceDeclarationKey>,
    >(&encode(&function).unwrap(), DecodeLimits::default())
    .unwrap();
    let decoded_nominal = decode_canonical::<
        DecodedCborIdentityRecord<PersistentTypeId, DecodedSourceDeclarationKey>,
    >(&encode(&nominal).unwrap(), DecodeLimits::default())
    .unwrap();
    let decoded_exact = decode_canonical::<
        DecodedCborIdentityRecord<PersistentExactTypeId, DecodedExactTypeKey>,
    >(&encode(&exact).unwrap(), DecodeLimits::default())
    .unwrap();
    let decoded_context_exact = decode_canonical::<
        DecodedCborIdentityRecord<PersistentExactTypeId, DecodedExactTypeKey>,
    >(&encode(&context_exact).unwrap(), DecodeLimits::default())
    .unwrap();
    let decoded_registration = decode_canonical::<
        DecodedCborIdentityRecord<PersistentCallbackRegistrationId, DecodedCallbackRegistrationKey>,
    >(&encode(&registration).unwrap(), DecodeLimits::default())
    .unwrap();
    let decoded_application = decode_canonical::<
        DecodedCborIdentityRecord<PersistentCallbackApplicationId, DecodedCallbackApplicationKey>,
    >(&encode(&application).unwrap(), DecodeLimits::default())
    .unwrap();

    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    pending
        .register(IdentityLayer::Hir, &decoded_function)
        .unwrap();
    pending
        .register(IdentityLayer::Hir, &decoded_nominal)
        .unwrap();
    pending
        .register(IdentityLayer::Hir, &decoded_exact)
        .unwrap();
    pending
        .register(IdentityLayer::Hir, &decoded_context_exact)
        .unwrap();
    pending
        .register(IdentityLayer::Hir, &decoded_registration)
        .unwrap();
    pending
        .register(IdentityLayer::Mir, &decoded_application)
        .unwrap();
    decoded.register_identities(&mut pending).unwrap();

    pending.resolve(&decoded_function).unwrap();
    pending.resolve(&decoded_nominal).unwrap();
    pending.resolve(&decoded_exact).unwrap();
    pending.resolve(&decoded_context_exact).unwrap();
    pending.resolve(&decoded_registration).unwrap();
    pending.resolve(&decoded_application).unwrap();
    decoded.resolve_identities(&mut pending).unwrap();
    (decoded, pending.finish().unwrap(), bytes)
}

fn native_contract_fixture(
    atom_producer: Option<ConeIdentity>,
) -> (DecodedLirFoundation, ValidatedIdentityGraph, Vec<u8>) {
    let function_key = SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("native").unwrap(),
        0,
        None,
        Vec::new(),
    );
    let function: CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey> =
        CborIdentityRecord::from_key(function_key.clone()).unwrap();
    let source_key = SourceNativeExternalContractKey::function(&function_key).unwrap();
    let source = SourceNativeExternalContractRecord::new(
        source_key,
        SourceNativeExternalContract::Function {
            symbol: SourceNativeSymbol::new("native").unwrap(),
            library: SourceNativeLibraryBinding::DefaultNativeNamespace,
            abi: SourceExternFunctionAbi::C(SourceCAbiFunctionSignature::new(
                Vec::new(),
                SourceCAbiReturn::Void,
            )),
            calling_convention: SourceCallingConvention::Cdecl,
        },
    )
    .unwrap();
    let contract = NativeExternalContractRecord::new(
        source.id(),
        NativeExternalSymbolKey::darwin_macho_external(&SourceNativeSymbol::new("native").unwrap())
            .unwrap(),
        NativeExternalContract::c_function(
            NativeLibraryBinding::DefaultNativeNamespace,
            CanonicalCAbiFunctionSignature::cdecl(Vec::new(), CanonicalCAbiReturn::Void),
        ),
    )
    .unwrap();
    let unit = CborIdentityRecord::from_key(GeneratedBridgeUnitKey::OutboundFunction(
        contract.fingerprint(),
    ))
    .unwrap();
    let atom = atom_producer.map(|producer| {
        CborIdentityRecord::from_key(GeneratedBridgeAtomKey::new(
            producer,
            GeneratedBridgeAtomRoleKey::PrimaryEntry { unit: unit.id() },
        ))
        .unwrap()
    });

    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_native_contracts(vec![contract]).unwrap();
    canonical.set_bridge_units(vec![unit]).unwrap();
    canonical
        .set_bridge_atoms(atom.into_iter().collect())
        .unwrap();
    let bytes = encode(&canonical).unwrap();
    let decoded =
        decode_canonical::<DecodedLirFoundation>(&bytes, DecodeLimits::default()).unwrap();

    let decoded_function = decode_canonical::<
        DecodedCborIdentityRecord<PersistentFunctionId, DecodedSourceDeclarationKey>,
    >(&encode(&function).unwrap(), DecodeLimits::default())
    .unwrap();
    let decoded_source = decode_canonical::<DecodedSourceNativeExternalContractRecord>(
        &encode(&source).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    if let Some(producer) = atom_producer {
        pending.register_authority(producer).unwrap();
    }
    pending
        .register(IdentityLayer::Hir, &decoded_function)
        .unwrap();
    pending
        .register_source_native_contract(IdentityLayer::Hir, &decoded_source)
        .unwrap();
    decoded.register_identities(&mut pending).unwrap();
    pending.resolve(&decoded_function).unwrap();
    pending
        .resolve_source_native_contract(&decoded_source)
        .unwrap();
    decoded.resolve_identities(&mut pending).unwrap();
    (decoded, pending.finish().unwrap(), bytes)
}

fn graph_with_authorities(authorities: &[ConeIdentity]) -> ValidatedIdentityGraph {
    let mut pending = PendingIdentityValidation::new();
    for authority in authorities {
        pending.register_authority(*authority).unwrap();
    }
    pending.finish().unwrap()
}
