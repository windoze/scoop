use scoop_wire::{DecodeLimits, WireErrorKind, decode_canonical, encode};

use super::{
    CallbackIdentityResolutionError, DecodedCallbackApplicationKey, DecodedCallbackRegistrationKey,
    DecodedSignatureCallableShape,
};
use crate::{
    CallableMaterializationContext, CallbackApplicationIdentityError, CallbackApplicationKey,
    CallbackMode, CallbackParameterIndex, CallbackRegistrationKey, CborIdentityRecord,
    DecodedCborIdentityRecord, Effect, GeneratedCallableKey, LexicalCallableParent,
    PersistentCallableApplicationId, PersistentCallbackApplicationId,
    PersistentCallbackRegistrationId, PersistentConstructorId, PersistentEnumVariantId,
    PersistentFunctionId, PersistentGeneratedCallableId, PersistentGenericFunctionId,
    PersistentGenericTypeId, PersistentIdMismatch, PersistentIdResolver,
    PersistentInitializationUnitId, PersistentKeyResolver, PersistentPropertyAccessorId,
    PersistentTypeId, SignatureCallableShape, SignatureTypeKey, SourceCAbiFunctionSignature,
    SourceCAbiReturn, StructuralDefinitionPath, StructuralDefinitionSiteRole,
    StructuralPathSegment,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolutionError;

struct Resolver {
    registration: CallbackRegistrationKey,
}

macro_rules! id_resolver {
    ($id:ty, $expected:expr) => {
        impl PersistentIdResolver<$id> for Resolver {
            type Error = ResolutionError;

            fn resolve(&mut self, id: crate::DecodedPersistentId<$id>) -> Result<$id, Self::Error> {
                id.verify($expected)
                    .map_err(|_: PersistentIdMismatch<$id>| ResolutionError)
            }
        }
    };
}

id_resolver!(PersistentFunctionId, function());
id_resolver!(PersistentGenericFunctionId, generic_function());
id_resolver!(PersistentConstructorId, constructor());
id_resolver!(PersistentPropertyAccessorId, accessor());
id_resolver!(PersistentTypeId, plain_type());
id_resolver!(PersistentGenericTypeId, generic_type());
id_resolver!(PersistentCallableApplicationId, callable_application());
id_resolver!(PersistentInitializationUnitId, initialization_unit());
id_resolver!(PersistentEnumVariantId, enum_variant());

impl PersistentKeyResolver<PersistentGeneratedCallableId, GeneratedCallableKey> for Resolver {
    type Error = ResolutionError;

    fn resolve_key(
        &mut self,
        _id: crate::DecodedPersistentId<PersistentGeneratedCallableId>,
    ) -> Result<std::sync::Arc<GeneratedCallableKey>, Self::Error> {
        Err(ResolutionError)
    }
}

impl PersistentKeyResolver<PersistentCallbackRegistrationId, CallbackRegistrationKey> for Resolver {
    type Error = ResolutionError;

    fn resolve_key(
        &mut self,
        id: crate::DecodedPersistentId<PersistentCallbackRegistrationId>,
    ) -> Result<std::sync::Arc<CallbackRegistrationKey>, Self::Error> {
        let expected = PersistentCallbackRegistrationId::from_key(&self.registration)
            .map_err(|_| ResolutionError)?;
        id.verify(expected)
            .map_err(|_: PersistentIdMismatch<PersistentCallbackRegistrationId>| ResolutionError)?;
        Ok(std::sync::Arc::new(self.registration.clone()))
    }
}

#[test]
fn managed_signature_shape_round_trips_and_resolves() {
    let shape = SignatureCallableShape::new(
        Effect::Suspend,
        Some(nominal()),
        vec![binder(), nominal()],
        nominal(),
    );
    let decoded = decode_canonical::<DecodedSignatureCallableShape>(
        &encode(&shape).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();

    assert_eq!(
        decoded
            .resolve(&mut resolver(registration(nominal())))
            .unwrap(),
        shape
    );
}

#[test]
fn callback_registration_record_resolves_before_verifying_identity() {
    let registration = registration(nominal());
    let record: CborIdentityRecord<PersistentCallbackRegistrationId, _> =
        CborIdentityRecord::from_key(registration.clone()).unwrap();
    let decoded = decode_canonical::<
        DecodedCborIdentityRecord<PersistentCallbackRegistrationId, DecodedCallbackRegistrationKey>,
    >(&encode(&record).unwrap(), DecodeLimits::default())
    .unwrap();
    let mut resolver = resolver(registration.clone());
    let resolved = decoded.resolve(|key| key.resolve(&mut resolver)).unwrap();

    assert_eq!(resolved.key(), &registration);
}

#[test]
fn callback_application_records_resolve_registration_and_context() {
    let registration = registration(nominal());
    let applications = [
        CallbackApplicationKey::new(
            &registration,
            CallableMaterializationContext::NoSubstitution,
        )
        .unwrap(),
        CallbackApplicationKey::new(
            &registration,
            CallableMaterializationContext::Application(callable_application()),
        )
        .unwrap(),
        CallbackApplicationKey::new(
            &registration,
            CallableMaterializationContext::InitializationApplication(initialization_unit()),
        )
        .unwrap(),
    ];

    for application in applications {
        let record: CborIdentityRecord<PersistentCallbackApplicationId, _> =
            CborIdentityRecord::from_key(application).unwrap();
        let decoded = decode_canonical::<
            DecodedCborIdentityRecord<
                PersistentCallbackApplicationId,
                DecodedCallbackApplicationKey,
            >,
        >(&encode(&record).unwrap(), DecodeLimits::default())
        .unwrap();
        let mut resolver = resolver(registration.clone());
        let resolved = decoded.resolve(|key| key.resolve(&mut resolver)).unwrap();

        assert_eq!(resolved.key(), &application);
    }
}

#[test]
fn callback_application_rechecks_binder_substitution_rule() {
    let registration = registration(binder());
    let registration_id = PersistentCallbackRegistrationId::from_key(&registration).unwrap();
    let raw = RawCallbackApplicationKey {
        registration: registration_id,
        context: CallableMaterializationContext::NoSubstitution,
    };
    let decoded = decode_canonical::<DecodedCallbackApplicationKey>(
        &encode(&raw).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();

    assert_eq!(
        decoded.resolve(&mut resolver(registration)),
        Err(CallbackIdentityResolutionError::Application(
            CallbackApplicationIdentityError::BinderRequiresSubstitution
        ))
    );
}

#[test]
fn callback_application_rejects_a_registration_key_for_another_id() {
    let expected_registration = registration(nominal());
    let other = registration(SignatureTypeKey::RawPointer(Box::new(nominal())));
    let raw = RawCallbackApplicationKey {
        registration: PersistentCallbackRegistrationId::from_key(&other).unwrap(),
        context: CallableMaterializationContext::NoSubstitution,
    };
    let decoded = decode_canonical::<DecodedCallbackApplicationKey>(
        &encode(&raw).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();

    assert_eq!(
        decoded.resolve(&mut resolver(expected_registration)),
        Err(CallbackIdentityResolutionError::Reference(ResolutionError))
    );
}

#[test]
fn callback_decoder_rejects_unknown_effect_and_large_index() {
    let shape = SignatureCallableShape::new(Effect::Ordinary, None, Vec::new(), nominal());
    let mut bytes = encode(&shape).unwrap();
    assert_eq!(&bytes[..3], &[0xa4, 0x01, 0x01]);
    bytes[2] = 3;
    let error = decode_canonical::<DecodedSignatureCallableShape>(&bytes, DecodeLimits::default())
        .unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 3 });

    let error = decode_canonical::<CallbackParameterIndex>(
        b"\x1b\x00\x00\x00\x01\x00\x00\x00\x00",
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::IntegerOutOfRange);
}

fn resolver(registration: CallbackRegistrationKey) -> Resolver {
    Resolver { registration }
}

fn registration(result: SignatureTypeKey) -> CallbackRegistrationKey {
    CallbackRegistrationKey::new(
        LexicalCallableParent::function(function()),
        path(),
        SourceCAbiFunctionSignature::new(vec![nominal()], SourceCAbiReturn::Value(nominal())),
        CallbackParameterIndex::new(1),
        SignatureCallableShape::new(Effect::Ordinary, None, vec![nominal()], result),
        CallbackMode::OneShot,
    )
}

fn path() -> StructuralDefinitionPath {
    StructuralDefinitionPath::from_first(
        StructuralPathSegment::new(StructuralDefinitionSiteRole::CallbackConversion, 0),
        [],
    )
}

fn nominal() -> SignatureTypeKey {
    SignatureTypeKey::Nominal(plain_type())
}

const fn binder() -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth: 0, index: 0 }
}

const fn function() -> PersistentFunctionId {
    PersistentFunctionId([1; 32])
}

const fn generic_function() -> PersistentGenericFunctionId {
    PersistentGenericFunctionId([2; 32])
}

const fn constructor() -> PersistentConstructorId {
    PersistentConstructorId([3; 32])
}

const fn accessor() -> PersistentPropertyAccessorId {
    PersistentPropertyAccessorId([4; 32])
}

const fn plain_type() -> PersistentTypeId {
    PersistentTypeId([5; 32])
}

const fn generic_type() -> PersistentGenericTypeId {
    PersistentGenericTypeId([6; 32])
}

const fn callable_application() -> PersistentCallableApplicationId {
    PersistentCallableApplicationId([7; 32])
}

const fn initialization_unit() -> PersistentInitializationUnitId {
    PersistentInitializationUnitId([8; 32])
}

const fn enum_variant() -> PersistentEnumVariantId {
    PersistentEnumVariantId([9; 32])
}

struct RawCallbackApplicationKey {
    registration: PersistentCallbackRegistrationId,
    context: CallableMaterializationContext,
}

impl scoop_wire::WireEncode for RawCallbackApplicationKey {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.registration.encode(encoder)?;
        encoder.field(2)?;
        self.context.encode(encoder)
    }
}
