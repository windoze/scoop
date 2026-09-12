use std::fmt;

use scoop_identity::{
    CallableBodyKey, CallableMaterializationContext, CallbackApplicationKey, CallbackMode,
    CallbackParameterIndex, CallbackRegistrationKey, CanonicalCAbiFunctionSignature,
    CanonicalCAbiReturn, CanonicalCAbiSignatureFingerprint, CanonicalIdentifier, ConeIdentity,
    DeclarationScope, DecodedPersistentId, DefinitionOwnerChain, Effect, ExactTypeKey,
    GeneratedBridgeUnitId, GeneratedBridgeUnitKey, LexicalCallableParent, PackagePath,
    PersistentCallableBodyId, PersistentCallbackApplicationId, PersistentExactTypeId,
    PersistentFunctionId, PersistentIdMismatch, PersistentIdResolver, PersistentSafepointSiteId,
    PersistentTypeId, RuntimeTypeId, SafepointId, SafepointSiteKey, SafepointSiteRole,
    SignatureCallableShape, SignatureTypeKey, SourceCAbiFunctionSignature, SourceCAbiReturn,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind, StrongCallableDefinitionOwner,
    StructuralDefinitionPath, StructuralDefinitionSiteRole, StructuralPathSegment,
};
use scoop_wire::{
    BudgetMeter, DecodeLimits, ResourceKind, WireErrorKind, WirePath, decode_canonical, encode,
};

use super::{
    CallbackBridgeRecord, DecodedCallbackBridgeRecord, DecodedRuntimeTypeMappingRecord,
    DecodedSafepointMappingRecord, RuntimeTypeMappingRecord, RuntimeTypeMappingResolutionError,
    SafepointMappingRecord, SafepointMappingResolutionError,
};

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolutionError;

impl fmt::Display for ResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("test reference does not exist")
    }
}

impl std::error::Error for ResolutionError {}

struct Resolver {
    exact: PersistentExactTypeId,
    site: PersistentSafepointSiteId,
    application: PersistentCallbackApplicationId,
    signature: CanonicalCAbiSignatureFingerprint,
    unit: GeneratedBridgeUnitId,
}

macro_rules! resolve_fixture_id {
    ($id:ty, $field:ident) => {
        impl PersistentIdResolver<$id> for Resolver {
            type Error = ResolutionError;

            fn resolve(&mut self, decoded: DecodedPersistentId<$id>) -> Result<$id, Self::Error> {
                verify(decoded, self.$field)
            }
        }
    };
}

resolve_fixture_id!(PersistentExactTypeId, exact);
resolve_fixture_id!(PersistentSafepointSiteId, site);
resolve_fixture_id!(PersistentCallbackApplicationId, application);
resolve_fixture_id!(CanonicalCAbiSignatureFingerprint, signature);
resolve_fixture_id!(GeneratedBridgeUnitId, unit);

#[test]
fn derived_mapping_records_roundtrip_and_recompute_ids() {
    let fixture = Fixture::new();
    let runtime = RuntimeTypeMappingRecord::new(fixture.exact).unwrap();
    let safepoint = SafepointMappingRecord::new(fixture.site).unwrap();

    let runtime_bytes = encode(&runtime).unwrap();
    let decoded_runtime = decode_canonical::<DecodedRuntimeTypeMappingRecord>(
        &runtime_bytes,
        DecodeLimits::default(),
    )
    .unwrap();
    assert_eq!(encode(&decoded_runtime).unwrap(), runtime_bytes);
    assert_eq!(
        decoded_runtime
            .resolve(&mut fixture.resolver(), &mut meter(), &WirePath::root())
            .unwrap(),
        runtime
    );

    let safepoint_bytes = encode(&safepoint).unwrap();
    let decoded_safepoint = decode_canonical::<DecodedSafepointMappingRecord>(
        &safepoint_bytes,
        DecodeLimits::default(),
    )
    .unwrap();
    assert_eq!(encode(&decoded_safepoint).unwrap(), safepoint_bytes);
    assert_eq!(
        decoded_safepoint
            .resolve(&mut fixture.resolver(), &mut meter(), &WirePath::root())
            .unwrap(),
        safepoint
    );
}

#[test]
fn derived_mapping_hashes_are_precharged_at_the_exact_limit() {
    let fixture = Fixture::new();
    let runtime = RuntimeTypeMappingRecord::new(fixture.exact).unwrap();
    let decoded_runtime = decode_canonical::<DecodedRuntimeTypeMappingRecord>(
        &encode(&runtime).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
    let runtime_work = (RuntimeTypeId::hash_stream_length().unwrap() + 72) / 64;
    for (limit, accepted) in [
        (runtime_work - 1, false),
        (runtime_work, true),
        (runtime_work + 1, true),
    ] {
        let mut meter = BudgetMeter::new(DecodeLimits {
            validation_work_units: limit,
            ..DecodeLimits::default()
        });
        let result =
            decoded_runtime.resolve(&mut fixture.resolver(), &mut meter, &WirePath::root());
        assert_eq!(result.is_ok(), accepted);
        if !accepted {
            assert!(matches!(
                result,
                Err(RuntimeTypeMappingResolutionError::Resource(ref error))
                    if error.kind() == &WireErrorKind::LimitExceeded {
                        resource: ResourceKind::ValidationWorkUnits,
                        limit,
                        observed: runtime_work,
                    }
            ));
        }
    }

    let safepoint = SafepointMappingRecord::new(fixture.site).unwrap();
    let decoded_safepoint = decode_canonical::<DecodedSafepointMappingRecord>(
        &encode(&safepoint).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
    let safepoint_work = (SafepointId::hash_stream_length().unwrap() + 72) / 64;
    for (limit, accepted) in [
        (safepoint_work - 1, false),
        (safepoint_work, true),
        (safepoint_work + 1, true),
    ] {
        let mut meter = BudgetMeter::new(DecodeLimits {
            validation_work_units: limit,
            ..DecodeLimits::default()
        });
        let result =
            decoded_safepoint.resolve(&mut fixture.resolver(), &mut meter, &WirePath::root());
        assert_eq!(result.is_ok(), accepted);
        if !accepted {
            assert!(matches!(
                result,
                Err(SafepointMappingResolutionError::Resource(ref error))
                    if error.kind() == &WireErrorKind::LimitExceeded {
                        resource: ResourceKind::ValidationWorkUnits,
                        limit,
                        observed: safepoint_work,
                    }
            ));
        }
    }
}

#[test]
fn stale_derived_runtime_ids_are_rejected() {
    let fixture = Fixture::new();
    let runtime = RuntimeTypeMappingRecord::new(fixture.exact).unwrap();
    let mut runtime_bytes = encode(&runtime).unwrap();
    let last = runtime_bytes.last_mut().unwrap();
    *last ^= 1;
    let decoded_runtime = decode_canonical::<DecodedRuntimeTypeMappingRecord>(
        &runtime_bytes,
        DecodeLimits::default(),
    )
    .unwrap();
    assert!(
        decoded_runtime
            .resolve(&mut fixture.resolver(), &mut meter(), &WirePath::root())
            .is_err()
    );

    let safepoint = SafepointMappingRecord::new(fixture.site).unwrap();
    let mut safepoint_bytes = encode(&safepoint).unwrap();
    let last = safepoint_bytes.last_mut().unwrap();
    *last ^= 1;
    let decoded_safepoint = decode_canonical::<DecodedSafepointMappingRecord>(
        &safepoint_bytes,
        DecodeLimits::default(),
    )
    .unwrap();
    assert!(
        decoded_safepoint
            .resolve(&mut fixture.resolver(), &mut meter(), &WirePath::root())
            .is_err()
    );
}

#[test]
fn zero_derived_runtime_ids_are_rejected_during_decode() {
    let fixture = Fixture::new();
    let mut bytes = vec![0xa2, 0x01];
    bytes.extend(encode(&fixture.exact).unwrap());
    bytes.extend([0x02, 0x00]);
    assert!(
        decode_canonical::<DecodedRuntimeTypeMappingRecord>(&bytes, DecodeLimits::default())
            .is_err()
    );
}

#[test]
fn callback_bridge_record_roundtrips_typed_references() {
    let fixture = Fixture::new();
    let record = CallbackBridgeRecord::new(fixture.application, fixture.signature, fixture.unit);
    let bytes = encode(&record).unwrap();
    let decoded =
        decode_canonical::<DecodedCallbackBridgeRecord>(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    assert_eq!(decoded.resolve(&mut fixture.resolver()).unwrap(), record);
}

#[test]
fn callback_bridge_rejects_a_missing_typed_unit() {
    let fixture = Fixture::new();
    let record = CallbackBridgeRecord::new(fixture.application, fixture.signature, fixture.unit);
    let decoded = decode_canonical::<DecodedCallbackBridgeRecord>(
        &encode(&record).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
    let mut resolver = fixture.resolver();
    resolver.unit = GeneratedBridgeUnitId::from_key(&GeneratedBridgeUnitKey::CallbackTrampoline {
        signature: fixture.signature,
        context_index: CallbackParameterIndex::new(1),
    })
    .unwrap();
    assert!(decoded.resolve(&mut resolver).is_err());
}

struct Fixture {
    exact: PersistentExactTypeId,
    site: PersistentSafepointSiteId,
    application: PersistentCallbackApplicationId,
    signature: CanonicalCAbiSignatureFingerprint,
    unit: GeneratedBridgeUnitId,
}

impl Fixture {
    fn new() -> Self {
        let site = || {
            SourceDeclarationSite::new(
                ConeIdentity::CORE,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap()
        };
        let nominal_key = SourceDeclarationKey::nominal(
            site(),
            CanonicalIdentifier::new("Value").unwrap(),
            SourceNominalKind::Class,
            0,
        );
        let nominal = PersistentTypeId::from_source_declaration(&nominal_key).unwrap();
        let exact = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(nominal)).unwrap();
        let function_key = SourceDeclarationKey::function(
            site(),
            CanonicalIdentifier::new("invoke").unwrap(),
            0,
            None,
            Vec::new(),
        );
        let function = PersistentFunctionId::from_source_declaration(&function_key).unwrap();
        let body = PersistentCallableBodyId::from_key(&CallableBodyKey::strong(
            StrongCallableDefinitionOwner::Function(function),
        ))
        .unwrap();
        let site = PersistentSafepointSiteId::from_key(&SafepointSiteKey::new(
            body,
            SafepointSiteRole::ManagedCall,
            0,
        ))
        .unwrap();

        let registration = CallbackRegistrationKey::new(
            LexicalCallableParent::function(function),
            StructuralDefinitionPath::from_first(
                StructuralPathSegment::new(StructuralDefinitionSiteRole::CallbackConversion, 0),
                [],
            ),
            SourceCAbiFunctionSignature::new(Vec::new(), SourceCAbiReturn::Void),
            CallbackParameterIndex::new(0),
            SignatureCallableShape::new(
                Effect::Ordinary,
                None,
                Vec::new(),
                SignatureTypeKey::Nominal(nominal),
            ),
            CallbackMode::Reusable,
        );
        let application_key = CallbackApplicationKey::new(
            &registration,
            CallableMaterializationContext::NoSubstitution,
        )
        .unwrap();
        let application = PersistentCallbackApplicationId::from_key(&application_key).unwrap();
        let signature = CanonicalCAbiSignatureFingerprint::from_signature(
            &CanonicalCAbiFunctionSignature::cdecl(Vec::new(), CanonicalCAbiReturn::Void),
        )
        .unwrap();
        let unit = GeneratedBridgeUnitId::from_key(&GeneratedBridgeUnitKey::CallbackTrampoline {
            signature,
            context_index: CallbackParameterIndex::new(0),
        })
        .unwrap();

        Self {
            exact,
            site,
            application,
            signature,
            unit,
        }
    }

    fn resolver(&self) -> Resolver {
        Resolver {
            exact: self.exact,
            site: self.site,
            application: self.application,
            signature: self.signature,
            unit: self.unit,
        }
    }
}

fn verify<I: scoop_identity::PersistentId>(
    decoded: DecodedPersistentId<I>,
    expected: I,
) -> Result<I, ResolutionError> {
    decoded
        .verify(expected)
        .map_err(|_: PersistentIdMismatch<I>| ResolutionError)
}
