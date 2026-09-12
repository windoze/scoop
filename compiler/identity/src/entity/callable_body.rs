use std::fmt;

use scoop_wire::{
    HashError, RuntimeDecode, RuntimeDecodeError, RuntimeDecodeErrorKind, RuntimeDecoder,
    RuntimeEncode, RuntimeEncodeError, RuntimeEncoder, WireEncode,
};

use super::CallableOdrMemberId;
use crate::ids::derive_runtime_persistent_id;
use crate::{
    ConeIdentity, DecodedPersistentId, OdrMemberId, OdrMemberIdentityError, OdrMemberKey,
    PersistentCallableBodyId, PersistentConstructorId, PersistentFunctionId,
    PersistentGeneratedCallableId, PersistentId, PersistentIdResolver,
    PersistentInitializationUnitId, PersistentKeyResolver, PersistentPropertyAccessorId,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StrongCallableDefinitionOwner {
    Function(PersistentFunctionId),
    Constructor(PersistentConstructorId),
    PropertyAccessor(PersistentPropertyAccessorId),
    GeneratedCallable(PersistentGeneratedCallableId),
}

impl RuntimeEncode for StrongCallableDefinitionOwner {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        match self {
            Self::Function(id) => encode_runtime_sum(encoder, 1, id),
            Self::Constructor(id) => encode_runtime_sum(encoder, 2, id),
            Self::PropertyAccessor(id) => encode_runtime_sum(encoder, 3, id),
            Self::GeneratedCallable(id) => encode_runtime_sum(encoder, 4, id),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableBodyKeyKind {
    Strong(StrongCallableDefinitionOwner),
    Odr(CallableOdrMemberId),
    RootGateway {
        root_cone: ConeIdentity,
        main: MainCallableBodyId,
    },
    InitializationStartupGateway(PersistentInitializationUnitId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableBodyKey(CallableBodyKeyKind);

impl CallableBodyKey {
    pub const fn strong(owner: StrongCallableDefinitionOwner) -> Self {
        Self(CallableBodyKeyKind::Strong(owner))
    }

    pub const fn odr(member: CallableOdrMemberId) -> Self {
        Self(CallableBodyKeyKind::Odr(member))
    }

    pub const fn root_gateway(root_cone: ConeIdentity, main: MainCallableBodyId) -> Self {
        Self(CallableBodyKeyKind::RootGateway { root_cone, main })
    }

    pub const fn initialization_startup_gateway(unit: PersistentInitializationUnitId) -> Self {
        Self(CallableBodyKeyKind::InitializationStartupGateway(unit))
    }

    pub const fn kind(&self) -> CallableBodyKeyKind {
        self.0
    }
}

impl RuntimeEncode for CallableBodyKey {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        match self.0 {
            CallableBodyKeyKind::Strong(owner) => encode_runtime_sum(encoder, 1, &owner),
            CallableBodyKeyKind::Odr(member) => {
                encoder.u32(2)?;
                encoder.fixed(member.member().as_array())
            }
            CallableBodyKeyKind::RootGateway { root_cone, main } => {
                encoder.u32(3)?;
                encoder.fixed(root_cone.as_array())?;
                encoder.fixed(main.body().as_array())
            }
            CallableBodyKeyKind::InitializationStartupGateway(unit) => {
                encoder.u32(4)?;
                encoder.fixed(unit.as_array())
            }
        }
    }
}

impl PersistentCallableBodyId {
    pub fn from_key(key: &CallableBodyKey) -> Result<Self, HashError> {
        derive_runtime_persistent_id("scoop-callable-body-v1", key)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DecodedStrongCallableDefinitionOwner {
    Function(DecodedPersistentId<PersistentFunctionId>),
    Constructor(DecodedPersistentId<PersistentConstructorId>),
    PropertyAccessor(DecodedPersistentId<PersistentPropertyAccessorId>),
    GeneratedCallable(DecodedPersistentId<PersistentGeneratedCallableId>),
}

impl RuntimeEncode for DecodedStrongCallableDefinitionOwner {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        match self {
            Self::Function(id) => encode_runtime_sum(encoder, 1, id),
            Self::Constructor(id) => encode_runtime_sum(encoder, 2, id),
            Self::PropertyAccessor(id) => encode_runtime_sum(encoder, 3, id),
            Self::GeneratedCallable(id) => encode_runtime_sum(encoder, 4, id),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DecodedCallableBodyKeyKind {
    Strong(DecodedStrongCallableDefinitionOwner),
    Odr(DecodedPersistentId<OdrMemberId>),
    RootGateway {
        root_cone: DecodedPersistentId<ConeIdentity>,
        main: DecodedPersistentId<PersistentCallableBodyId>,
    },
    InitializationStartupGateway(DecodedPersistentId<PersistentInitializationUnitId>),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DecodedCallableBodyKey(DecodedCallableBodyKeyKind);

impl DecodedCallableBodyKey {
    pub const fn kind(&self) -> DecodedCallableBodyKeyKind {
        self.0
    }

    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CallableBodyKey, CallableBodyResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentFunctionId, Error = E>
            + PersistentIdResolver<PersistentConstructorId, Error = E>
            + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
            + PersistentIdResolver<PersistentGeneratedCallableId, Error = E>
            + PersistentKeyResolver<OdrMemberId, OdrMemberKey, Error = E>
            + PersistentIdResolver<ConeIdentity, Error = E>
            + PersistentIdResolver<PersistentCallableBodyId, Error = E>
            + PersistentIdResolver<PersistentInitializationUnitId, Error = E>,
    {
        match self.0 {
            DecodedCallableBodyKeyKind::Strong(owner) => {
                let owner = match owner {
                    DecodedStrongCallableDefinitionOwner::Function(id) => resolver
                        .resolve(id)
                        .map(StrongCallableDefinitionOwner::Function),
                    DecodedStrongCallableDefinitionOwner::Constructor(id) => resolver
                        .resolve(id)
                        .map(StrongCallableDefinitionOwner::Constructor),
                    DecodedStrongCallableDefinitionOwner::PropertyAccessor(id) => resolver
                        .resolve(id)
                        .map(StrongCallableDefinitionOwner::PropertyAccessor),
                    DecodedStrongCallableDefinitionOwner::GeneratedCallable(id) => resolver
                        .resolve(id)
                        .map(StrongCallableDefinitionOwner::GeneratedCallable),
                }
                .map_err(CallableBodyResolutionError::Reference)?;
                Ok(CallableBodyKey::strong(owner))
            }
            DecodedCallableBodyKeyKind::Odr(member) => {
                let key = resolver
                    .resolve_key(member)
                    .map_err(CallableBodyResolutionError::Reference)?;
                let member = CallableOdrMemberId::from_key(&key)
                    .map_err(CallableBodyResolutionError::Member)?;
                Ok(CallableBodyKey::odr(member))
            }
            DecodedCallableBodyKeyKind::RootGateway { root_cone, main } => {
                let root_cone = resolver
                    .resolve(root_cone)
                    .map_err(CallableBodyResolutionError::Reference)?;
                let main = resolver
                    .resolve(main)
                    .map(MainCallableBodyId::from_body)
                    .map_err(CallableBodyResolutionError::Reference)?;
                Ok(CallableBodyKey::root_gateway(root_cone, main))
            }
            DecodedCallableBodyKeyKind::InitializationStartupGateway(unit) => resolver
                .resolve(unit)
                .map(CallableBodyKey::initialization_startup_gateway)
                .map_err(CallableBodyResolutionError::Reference),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableBodyResolutionError<E> {
    Reference(E),
    Member(OdrMemberIdentityError),
}

impl<E: fmt::Display> fmt::Display for CallableBodyResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(error) => error.fmt(formatter),
            Self::Member(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for CallableBodyResolutionError<E> {}

impl RuntimeDecode for DecodedCallableBodyKey {
    fn runtime_decode(decoder: &mut RuntimeDecoder<'_>) -> Result<Self, RuntimeDecodeError> {
        let kind = match decoder.u32()? {
            1 => DecodedCallableBodyKeyKind::Strong(decode_strong_owner(decoder)?),
            2 => DecodedCallableBodyKeyKind::Odr(decode_persistent_id(decoder)?),
            3 => DecodedCallableBodyKeyKind::RootGateway {
                root_cone: decode_persistent_id(decoder)?,
                main: decode_persistent_id(decoder)?,
            },
            4 => DecodedCallableBodyKeyKind::InitializationStartupGateway(decode_persistent_id(
                decoder,
            )?),
            tag => return Err(decoder.error(RuntimeDecodeErrorKind::UnknownTag { tag })),
        };
        Ok(Self(kind))
    }
}

impl RuntimeEncode for DecodedCallableBodyKey {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        match self.0 {
            DecodedCallableBodyKeyKind::Strong(owner) => encode_runtime_sum(encoder, 1, &owner),
            DecodedCallableBodyKeyKind::Odr(member) => encode_runtime_sum(encoder, 2, &member),
            DecodedCallableBodyKeyKind::RootGateway { root_cone, main } => {
                encoder.u32(3)?;
                root_cone.runtime_encode(encoder)?;
                main.runtime_encode(encoder)
            }
            DecodedCallableBodyKeyKind::InitializationStartupGateway(unit) => {
                encode_runtime_sum(encoder, 4, &unit)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MainCallableBodyId(PersistentCallableBodyId);

impl MainCallableBodyId {
    pub(crate) const fn from_body(body: PersistentCallableBodyId) -> Self {
        Self(body)
    }

    pub const fn body(self) -> PersistentCallableBodyId {
        self.0
    }
}

impl RuntimeEncode for MainCallableBodyId {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        encoder.fixed(self.0.as_array())
    }
}

impl WireEncode for MainCallableBodyId {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.0.encode(encoder)
    }
}

fn encode_runtime_sum(
    encoder: &mut RuntimeEncoder,
    tag: u32,
    value: &impl RuntimeEncode,
) -> Result<(), RuntimeEncodeError> {
    encoder.u32(tag)?;
    value.runtime_encode(encoder)
}

fn decode_strong_owner(
    decoder: &mut RuntimeDecoder<'_>,
) -> Result<DecodedStrongCallableDefinitionOwner, RuntimeDecodeError> {
    match decoder.u32()? {
        1 => decode_persistent_id(decoder).map(DecodedStrongCallableDefinitionOwner::Function),
        2 => decode_persistent_id(decoder).map(DecodedStrongCallableDefinitionOwner::Constructor),
        3 => decode_persistent_id(decoder)
            .map(DecodedStrongCallableDefinitionOwner::PropertyAccessor),
        4 => decode_persistent_id(decoder)
            .map(DecodedStrongCallableDefinitionOwner::GeneratedCallable),
        tag => Err(decoder.error(RuntimeDecodeErrorKind::UnknownTag { tag })),
    }
}

fn decode_persistent_id<I: PersistentId>(
    decoder: &mut RuntimeDecoder<'_>,
) -> Result<DecodedPersistentId<I>, RuntimeDecodeError> {
    let source = decoder.fixed(32)?;
    let mut bytes = [0; 32];
    bytes.copy_from_slice(source);
    Ok(DecodedPersistentId::from_unvalidated_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use scoop_wire::{RuntimeDecodeErrorKind, decode_runtime, encode_runtime};

    use super::{
        CallableBodyKey, CallableBodyKeyKind, DecodedCallableBodyKey, DecodedCallableBodyKeyKind,
        DecodedStrongCallableDefinitionOwner, MainCallableBodyId, StrongCallableDefinitionOwner,
    };
    use crate::{
        ConeIdentity, OdrMemberId, PersistentCallableBodyId, PersistentFunctionId,
        PersistentInitializationUnitId,
    };

    #[test]
    fn strong_callable_body_has_fixed_runtime_bytes_and_identity() {
        let function = PersistentFunctionId(ConeIdentity::CORE.0);
        let key = CallableBodyKey::strong(StrongCallableDefinitionOwner::Function(function));

        assert_eq!(
            encode_runtime(&key).unwrap(),
            [b"\x01\0\0\0\x01\0\0\0".as_slice(), function.as_array()].concat()
        );
        assert_eq!(
            PersistentCallableBodyId::from_key(&key)
                .unwrap()
                .to_string(),
            "e8de63b8e2758608238897adc56513f83fd4083605bfcbe293c67e8d41c9d2bd"
        );
    }

    #[test]
    fn root_gateway_keeps_root_and_main_as_separate_typed_fields() {
        let main_body = PersistentCallableBodyId(ConeIdentity::CORE.0);
        let main = MainCallableBodyId(main_body);
        let key = CallableBodyKey::root_gateway(ConeIdentity::SINGLE_FILE, main);
        let encoded = encode_runtime(&key).unwrap();

        assert_eq!(&encoded[..4], b"\x03\0\0\0");
        assert_eq!(&encoded[4..36], ConeIdentity::SINGLE_FILE.as_array());
        assert_eq!(&encoded[36..], main_body.as_array());
        assert_eq!(
            key.kind(),
            CallableBodyKeyKind::RootGateway {
                root_cone: ConeIdentity::SINGLE_FILE,
                main,
            }
        );
    }

    #[test]
    fn decoded_body_keys_round_trip_every_runtime_variant() {
        let bytes = ConeIdentity::CORE.0;
        let encoded = [
            [b"\x01\0\0\0\x01\0\0\0".as_slice(), bytes.as_slice()].concat(),
            [b"\x01\0\0\0\x02\0\0\0".as_slice(), bytes.as_slice()].concat(),
            [b"\x01\0\0\0\x03\0\0\0".as_slice(), bytes.as_slice()].concat(),
            [b"\x01\0\0\0\x04\0\0\0".as_slice(), bytes.as_slice()].concat(),
            [b"\x02\0\0\0".as_slice(), bytes.as_slice()].concat(),
            [
                b"\x03\0\0\0".as_slice(),
                ConeIdentity::SINGLE_FILE.as_array(),
                bytes.as_slice(),
            ]
            .concat(),
            [b"\x04\0\0\0".as_slice(), bytes.as_slice()].concat(),
        ];

        for bytes in encoded {
            let decoded = decode_runtime::<DecodedCallableBodyKey>(&bytes).unwrap();
            assert_eq!(encode_runtime(&decoded).unwrap(), bytes);
        }

        let strong = decode_runtime::<DecodedCallableBodyKey>(&encoded_strong(bytes)).unwrap();
        assert!(matches!(
            strong.kind(),
            DecodedCallableBodyKeyKind::Strong(
                DecodedStrongCallableDefinitionOwner::Function(id)
            ) if id.as_array() == &bytes
        ));

        let odr = decode_runtime::<DecodedCallableBodyKey>(
            &[b"\x02\0\0\0".as_slice(), bytes.as_slice()].concat(),
        )
        .unwrap();
        assert!(matches!(
            odr.kind(),
            DecodedCallableBodyKeyKind::Odr(id) if id.as_array() == OdrMemberId(bytes).as_array()
        ));

        let unit = decode_runtime::<DecodedCallableBodyKey>(
            &[b"\x04\0\0\0".as_slice(), bytes.as_slice()].concat(),
        )
        .unwrap();
        assert!(matches!(
            unit.kind(),
            DecodedCallableBodyKeyKind::InitializationStartupGateway(id)
                if id.as_array() == PersistentInitializationUnitId(bytes).as_array()
        ));
    }

    #[test]
    fn decoded_body_key_rejects_unknown_and_incomplete_variants() {
        let unknown = decode_runtime::<DecodedCallableBodyKey>(b"\x05\0\0\0").unwrap_err();
        assert_eq!(
            unknown.kind(),
            RuntimeDecodeErrorKind::UnknownTag { tag: 5 }
        );

        let unknown_owner =
            decode_runtime::<DecodedCallableBodyKey>(b"\x01\0\0\0\x05\0\0\0").unwrap_err();
        assert_eq!(
            unknown_owner.kind(),
            RuntimeDecodeErrorKind::UnknownTag { tag: 5 }
        );

        let incomplete = decode_runtime::<DecodedCallableBodyKey>(b"\x02\0\0\0").unwrap_err();
        assert!(matches!(
            incomplete.kind(),
            RuntimeDecodeErrorKind::UnexpectedEnd { .. }
        ));
    }

    fn encoded_strong(bytes: [u8; 32]) -> Vec<u8> {
        [b"\x01\0\0\0\x01\0\0\0".as_slice(), bytes.as_slice()].concat()
    }
}
