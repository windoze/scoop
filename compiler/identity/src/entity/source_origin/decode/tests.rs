use scoop_wire::{DecodeLimits, WireErrorKind, decode_canonical, encode};

use super::{
    DecodedDefinitionOriginRecord, DecodedDefinitionOriginSubject, DecodedExpressionOrigin,
    DecodedSourceContextKey, DecodedSourceSpan, SourceContextResolutionError,
    SourceOriginResolutionError,
};
use crate::{
    CallableOwner, CborIdentityRecord, ConcreteExpressionOrigin, ConeIdentity,
    DecodedCborIdentityRecord, DecodedPersistentId, DefinitionOrigin, DefinitionOriginRecord,
    DefinitionOriginSubject, EvaluationOrigin, ExpressionOrigin, NominalDeclarationOwner,
    NormalizedSourcePath, PersistentCallableApplicationId, PersistentCallbackRegistrationId,
    PersistentConstructorId, PersistentEnumVariantFieldId, PersistentEnumVariantId,
    PersistentExtensionPropertyId, PersistentFieldId, PersistentFunctionId,
    PersistentGeneratedCallableId, PersistentGenericFunctionId, PersistentGenericTypeId,
    PersistentIdMismatch, PersistentIdResolver, PersistentInitializationUnitId,
    PersistentKeyResolver, PersistentLocalBindingId, PersistentLocalValueId,
    PersistentPropertyAccessorId, PersistentPropertyId, PersistentSourceContextId,
    PersistentSourceNativeExternalContractId, PersistentTypeAliasId, PersistentTypeId,
    PropertyOwner, SourceContextKey, SourceIdentity, SourceOriginError, SourceSpan,
    SourceSpanError,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolutionError;

impl std::fmt::Display for ResolutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("identity is absent from the test graph")
    }
}

impl std::error::Error for ResolutionError {}

struct Resolver;

impl PersistentIdResolver<ConeIdentity> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<ConeIdentity>,
    ) -> Result<ConeIdentity, Self::Error> {
        id.verify(ConeIdentity::CORE)
            .map_err(|_: PersistentIdMismatch<ConeIdentity>| ResolutionError)
    }
}

impl PersistentKeyResolver<PersistentSourceContextId, SourceContextKey> for Resolver {
    type Error = ResolutionError;

    fn resolve_key(
        &mut self,
        id: DecodedPersistentId<PersistentSourceContextId>,
    ) -> Result<std::sync::Arc<SourceContextKey>, Self::Error> {
        for key in context_keys() {
            let expected = PersistentSourceContextId::from_key(&key).unwrap();
            if expected.as_array() == id.as_array() {
                return id
                    .verify(expected)
                    .map(|_| std::sync::Arc::new(key))
                    .map_err(|_| ResolutionError);
            }
        }
        Err(ResolutionError)
    }
}

trait TestId {
    fn expected() -> Self;
}

macro_rules! test_identity {
    ($id:ty) => {
        impl TestId for $id {
            fn expected() -> Self {
                Self([7; 32])
            }
        }

        impl PersistentIdResolver<$id> for Resolver {
            type Error = ResolutionError;

            fn resolve(&mut self, id: DecodedPersistentId<$id>) -> Result<$id, Self::Error> {
                id.verify(<$id>::expected())
                    .map_err(|_: PersistentIdMismatch<$id>| ResolutionError)
            }
        }
    };
}

test_identity!(PersistentTypeId);
test_identity!(PersistentGenericTypeId);
test_identity!(PersistentFunctionId);
test_identity!(PersistentGenericFunctionId);
test_identity!(PersistentCallableApplicationId);
test_identity!(PersistentConstructorId);
test_identity!(PersistentPropertyAccessorId);
test_identity!(PersistentGeneratedCallableId);
test_identity!(PersistentPropertyId);
test_identity!(PersistentExtensionPropertyId);
test_identity!(PersistentInitializationUnitId);
test_identity!(PersistentTypeAliasId);
test_identity!(PersistentFieldId);
test_identity!(PersistentEnumVariantId);
test_identity!(PersistentEnumVariantFieldId);
test_identity!(PersistentLocalBindingId);
test_identity!(PersistentLocalValueId);
test_identity!(PersistentCallbackRegistrationId);
test_identity!(PersistentSourceNativeExternalContractId);

#[test]
fn all_source_context_records_round_trip_and_resolve() {
    let source = source();
    let keys = [
        SourceContextKey::File {
            source: source.clone(),
        },
        SourceContextKey::Nominal {
            source: source.clone(),
            owner: NominalDeclarationOwner::Concrete(PersistentTypeId::expected()),
        },
        SourceContextKey::Callable {
            source: source.clone(),
            owner: CallableOwner::Generated(PersistentGeneratedCallableId::expected()),
        },
        SourceContextKey::Property {
            source: source.clone(),
            owner: PropertyOwner::ExtensionProperty(PersistentExtensionPropertyId::expected()),
        },
        SourceContextKey::Initialization {
            source,
            unit: PersistentInitializationUnitId::expected(),
        },
    ];

    for key in keys {
        let record = CborIdentityRecord::<PersistentSourceContextId, _>::from_key(key).unwrap();
        let decoded = decode_canonical::<
            DecodedCborIdentityRecord<PersistentSourceContextId, DecodedSourceContextKey>,
        >(&encode(&record).unwrap(), DecodeLimits::default())
        .unwrap();
        assert_eq!(
            decoded.resolve(|key| key.resolve(&mut Resolver)).unwrap(),
            record
        );
    }
}

#[test]
fn source_context_resolution_rejects_an_unknown_owner_reference() {
    let key = SourceContextKey::Nominal {
        source: source(),
        owner: NominalDeclarationOwner::Concrete(PersistentTypeId::expected()),
    };
    let mut bytes = encode(&key).unwrap();
    assert_eq!(*bytes.last().unwrap(), 7);
    *bytes.last_mut().unwrap() = 8;
    let decoded =
        decode_canonical::<DecodedSourceContextKey>(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(
        decoded.resolve(&mut Resolver),
        Err(SourceContextResolutionError::Reference(ResolutionError))
    );
}

#[test]
fn source_context_decoder_rejects_unknown_and_incomplete_variants() {
    let unknown = decode_canonical::<DecodedSourceContextKey>(
        b"\xa2\x00\x06\x01\xa2\x01\x58\x20\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x02\x61x",
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert_eq!(unknown.kind(), &WireErrorKind::UnknownTag { tag: 6 });

    let incomplete = decode_canonical::<DecodedSourceContextKey>(
        b"\xa2\x00\x02\x01\xa2\x01\x58\x20\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x02\x61x",
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert_eq!(
        incomplete.kind(),
        &WireErrorKind::InvalidLength {
            expected: 3,
            actual: 2,
        }
    );
}

#[test]
fn expression_origins_round_trip_and_resolve_context_keys() {
    let definition_context = context_keys().remove(0);
    let evaluation_context = context_keys().remove(1);
    let definition = DefinitionOrigin::new(
        definition_context.source().clone(),
        SourceSpan::new(3, 7).unwrap(),
        &definition_context,
    )
    .unwrap();
    let evaluation = EvaluationOrigin::new(
        evaluation_context.source().clone(),
        SourceSpan::new(11, 19).unwrap(),
        &evaluation_context,
    )
    .unwrap();
    let origins = [
        ExpressionOrigin::Definition(definition.clone()),
        ExpressionOrigin::Concrete(ConcreteExpressionOrigin::new(definition, evaluation)),
    ];

    for origin in origins {
        let decoded = decode_canonical::<DecodedExpressionOrigin>(
            &encode(&origin).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        assert_eq!(decoded.resolve(&mut Resolver).unwrap(), origin);
    }
}

#[test]
fn source_origin_resolution_rejects_invalid_spans_and_context_source_mismatches() {
    let span =
        decode_canonical::<DecodedSourceSpan>(b"\xa2\x01\x05\x02\x04", DecodeLimits::default())
            .unwrap();
    assert_eq!(span.validate(), Err(SourceSpanError));

    let definition_context = context_keys().remove(0);
    let evaluation_context = context_keys().remove(1);
    let evaluation = EvaluationOrigin::new(
        evaluation_context.source().clone(),
        SourceSpan::new(11, 19).unwrap(),
        &evaluation_context,
    )
    .unwrap();
    let mut bytes = encode(&evaluation).unwrap();
    let wrong_context = PersistentSourceContextId::from_key(&definition_context).unwrap();
    let context_start = bytes.len() - wrong_context.as_array().len();
    bytes[context_start..].copy_from_slice(wrong_context.as_array());
    let decoded =
        decode_canonical::<super::DecodedEvaluationOrigin>(&bytes, DecodeLimits::default())
            .unwrap();
    assert_eq!(
        decoded.resolve(&mut Resolver),
        Err(SourceOriginResolutionError::Origin(
            SourceOriginError::ContextSourceMismatch
        ))
    );
}

#[test]
fn expression_origin_decoder_rejects_unknown_variants() {
    let error = decode_canonical::<DecodedExpressionOrigin>(
        b"\xa2\x00\x03\x01\x80",
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 3 });
}

#[test]
fn every_definition_origin_subject_round_trips_and_resolves() {
    let context = context_keys().remove(0);
    let origin = DefinitionOrigin::new(
        context.source().clone(),
        SourceSpan::new(23, 29).unwrap(),
        &context,
    )
    .unwrap();

    for subject in definition_subjects() {
        let record = DefinitionOriginRecord::new(subject, origin.clone());
        let decoded = decode_canonical::<DecodedDefinitionOriginRecord>(
            &encode(&record).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        assert_eq!(decoded.resolve(&mut Resolver).unwrap(), record);
    }
}

#[test]
fn definition_origin_subject_decoder_rejects_unknown_variants() {
    let mut bytes = vec![0xa2, 0x00, 0x13, 0x01, 0x58, 0x20];
    bytes.extend_from_slice(&[7; 32]);
    let error = decode_canonical::<DecodedDefinitionOriginSubject>(&bytes, DecodeLimits::default())
        .unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 19 });
}

fn source() -> SourceIdentity {
    SourceIdentity::new(
        ConeIdentity::CORE,
        NormalizedSourcePath::new("src/context.scoop").unwrap(),
    )
    .unwrap()
}

fn evaluation_source() -> SourceIdentity {
    SourceIdentity::new(
        ConeIdentity::CORE,
        NormalizedSourcePath::new("src/evaluation.scoop").unwrap(),
    )
    .unwrap()
}

fn context_keys() -> Vec<SourceContextKey> {
    vec![
        SourceContextKey::File { source: source() },
        SourceContextKey::File {
            source: evaluation_source(),
        },
    ]
}

fn definition_subjects() -> [DefinitionOriginSubject; 18] {
    [
        DefinitionOriginSubject::Type(PersistentTypeId::expected()),
        DefinitionOriginSubject::GenericType(PersistentGenericTypeId::expected()),
        DefinitionOriginSubject::Function(PersistentFunctionId::expected()),
        DefinitionOriginSubject::GenericFunction(PersistentGenericFunctionId::expected()),
        DefinitionOriginSubject::Constructor(PersistentConstructorId::expected()),
        DefinitionOriginSubject::Property(PersistentPropertyId::expected()),
        DefinitionOriginSubject::ExtensionProperty(PersistentExtensionPropertyId::expected()),
        DefinitionOriginSubject::PropertyAccessor(PersistentPropertyAccessorId::expected()),
        DefinitionOriginSubject::TypeAlias(PersistentTypeAliasId::expected()),
        DefinitionOriginSubject::Field(PersistentFieldId::expected()),
        DefinitionOriginSubject::EnumVariant(PersistentEnumVariantId::expected()),
        DefinitionOriginSubject::EnumVariantField(PersistentEnumVariantFieldId::expected()),
        DefinitionOriginSubject::GeneratedCallable(PersistentGeneratedCallableId::expected()),
        DefinitionOriginSubject::InitializationUnit(PersistentInitializationUnitId::expected()),
        DefinitionOriginSubject::LocalBinding(PersistentLocalBindingId::expected()),
        DefinitionOriginSubject::LocalValue(PersistentLocalValueId::expected()),
        DefinitionOriginSubject::CallbackRegistration(PersistentCallbackRegistrationId::expected()),
        DefinitionOriginSubject::SourceNativeContract(
            PersistentSourceNativeExternalContractId::expected(),
        ),
    ]
}
