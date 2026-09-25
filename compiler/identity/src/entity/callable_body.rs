use std::fmt;

use scoop_wire::{
    Decoder, Encoder, HashError, RuntimeDecode, RuntimeDecodeError, RuntimeDecodeErrorKind,
    RuntimeDecoder, RuntimeEncode, RuntimeEncodeError, RuntimeEncoder, WireDecode, WireEncode,
    WireError,
};

use super::CallableOdrMemberId;
use crate::ids::derive_runtime_persistent_id;
use crate::{
    CallableOwner, CborIdentityRecord, ConeIdentity, DeclarationName, DecodedPersistentId,
    DuplicateSignatureKey, ExactOrdinaryNoArgUnitSignature, OdrMemberId, OdrMemberIdentityError,
    OdrMemberKey, OptionalSignatureType, PersistentCallableBodyId, PersistentConstructorId,
    PersistentFunctionId, PersistentGeneratedCallableId, PersistentId, PersistentIdResolver,
    PersistentInitializationUnitId, PersistentKeyResolver, PersistentPropertyAccessorId,
    SourceDeclarationKey, SourceDeclarationKind, SourceSignatureFingerprint,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StrongCallableDefinitionOwner {
    Function(PersistentFunctionId),
    Constructor(PersistentConstructorId),
    PropertyAccessor(PersistentPropertyAccessorId),
    GeneratedCallable(PersistentGeneratedCallableId),
}

impl StrongCallableDefinitionOwner {
    pub const fn callable_owner(self) -> CallableOwner {
        match self {
            Self::Function(id) => CallableOwner::Function(id),
            Self::Constructor(id) => CallableOwner::Constructor(id),
            Self::PropertyAccessor(id) => CallableOwner::Accessor(id),
            Self::GeneratedCallable(id) => CallableOwner::Generated(id),
        }
    }
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

impl WireEncode for StrongCallableDefinitionOwner {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Function(id) => encode_wire_sum(encoder, 1, id),
            Self::Constructor(id) => encode_wire_sum(encoder, 2, id),
            Self::PropertyAccessor(id) => encode_wire_sum(encoder, 3, id),
            Self::GeneratedCallable(id) => encode_wire_sum(encoder, 4, id),
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
    pub fn hash_stream_length(key: &CallableBodyKey) -> Result<u64, HashError> {
        let payload = match key.kind() {
            CallableBodyKeyKind::Strong(_) => 4 + 4 + 32,
            CallableBodyKeyKind::Odr(_) | CallableBodyKeyKind::InitializationStartupGateway(_) => {
                4 + 32
            }
            CallableBodyKeyKind::RootGateway { .. } => 4 + 32 + 32,
        };
        scoop_wire::domain_separated_hash_stream_length("scoop-callable-body-v1", payload)
    }

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

impl DecodedStrongCallableDefinitionOwner {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<StrongCallableDefinitionOwner, E>
    where
        R: PersistentIdResolver<PersistentFunctionId, Error = E>
            + PersistentIdResolver<PersistentConstructorId, Error = E>
            + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
            + PersistentIdResolver<PersistentGeneratedCallableId, Error = E>,
    {
        match self {
            Self::Function(id) => resolver
                .resolve(id)
                .map(StrongCallableDefinitionOwner::Function),
            Self::Constructor(id) => resolver
                .resolve(id)
                .map(StrongCallableDefinitionOwner::Constructor),
            Self::PropertyAccessor(id) => resolver
                .resolve(id)
                .map(StrongCallableDefinitionOwner::PropertyAccessor),
            Self::GeneratedCallable(id) => resolver
                .resolve(id)
                .map(StrongCallableDefinitionOwner::GeneratedCallable),
        }
    }
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

impl WireEncode for DecodedStrongCallableDefinitionOwner {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Function(id) => encode_wire_sum(encoder, 1, id),
            Self::Constructor(id) => encode_wire_sum(encoder, 2, id),
            Self::PropertyAccessor(id) => encode_wire_sum(encoder, 3, id),
            Self::GeneratedCallable(id) => encode_wire_sum(encoder, 4, id),
        }
    }
}

impl WireDecode for DecodedStrongCallableDefinitionOwner {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        if fields != 2 {
            return Err(WireError::new(
                scoop_wire::WireErrorKind::InvalidLength {
                    expected: 2,
                    actual: fields,
                },
                decoder.path().clone(),
                Some(decoder.position()),
            ));
        }
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Function),
            2 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Constructor),
            3 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::PropertyAccessor),
            4 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::GeneratedCallable),
            tag => Err(WireError::new(
                scoop_wire::WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
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
                let owner = owner
                    .resolve(resolver)
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

/// Persistent proof that a source declaration is the unique shape accepted
/// by executable-entry lowering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutableSourceEntryIdentity {
    root_cone: ConeIdentity,
    declaration: PersistentFunctionId,
    source_signature: ExactOrdinaryNoArgUnitSignature,
    source_signature_fingerprint: SourceSignatureFingerprint,
    main: MainCallableBodyId,
}

impl ExecutableSourceEntryIdentity {
    pub fn try_new(
        declaration: &CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey>,
        source_signature: ExactOrdinaryNoArgUnitSignature,
    ) -> Result<Self, ExecutableSourceEntryIdentityError> {
        let key = declaration.key();
        if key.declaration_kind() != SourceDeclarationKind::Function {
            return Err(ExecutableSourceEntryIdentityError::NotFunction);
        }
        if !key.owners().owners().is_empty() {
            return Err(ExecutableSourceEntryIdentityError::NotTopLevel);
        }
        if !matches!(key.name(), DeclarationName::Named(name) if name.as_str() == "main") {
            return Err(ExecutableSourceEntryIdentityError::NotMain);
        }
        if !matches!(
            key.duplicate_signature(),
            DuplicateSignatureKey::Function {
                type_parameter_count: 0,
                receiver: OptionalSignatureType::Absent,
                parameters,
            } if parameters.is_empty()
        ) {
            return Err(ExecutableSourceEntryIdentityError::InvalidDeclarationShape);
        }
        let declaration_id = declaration.id();
        let body = PersistentCallableBodyId::from_key(&CallableBodyKey::strong(
            StrongCallableDefinitionOwner::Function(declaration_id),
        ))
        .map_err(ExecutableSourceEntryIdentityError::Hash)?;
        let source_signature_fingerprint =
            SourceSignatureFingerprint::from_signature(&source_signature)
                .map_err(ExecutableSourceEntryIdentityError::Hash)?;
        Ok(Self {
            root_cone: key.origin(),
            declaration: declaration_id,
            source_signature,
            source_signature_fingerprint,
            main: MainCallableBodyId(body),
        })
    }

    pub const fn root_cone(&self) -> ConeIdentity {
        self.root_cone
    }

    pub const fn declaration(&self) -> PersistentFunctionId {
        self.declaration
    }

    pub const fn source_signature(&self) -> &ExactOrdinaryNoArgUnitSignature {
        &self.source_signature
    }

    pub const fn source_signature_fingerprint(&self) -> SourceSignatureFingerprint {
        self.source_signature_fingerprint
    }

    pub const fn main(&self) -> MainCallableBodyId {
        self.main
    }
}

impl WireEncode for ExecutableSourceEntryIdentity {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        self.root_cone.encode(encoder)?;
        encoder.field(2)?;
        self.declaration.encode(encoder)?;
        encoder.field(3)?;
        self.source_signature.encode(encoder)?;
        encoder.field(4)?;
        self.source_signature_fingerprint.encode(encoder)?;
        encoder.field(5)?;
        self.main.encode(encoder)
    }
}

/// Untrusted wire form of [`ExecutableSourceEntryIdentity`].
///
/// Readers must rebuild the trusted proof from the referenced source
/// declaration and the trusted core `Unit` exact identity, then compare the
/// complete encoded value. No field is independently promoted.
#[derive(Debug)]
pub struct DecodedExecutableSourceEntryIdentity {
    root_cone: DecodedPersistentId<ConeIdentity>,
    declaration: DecodedPersistentId<PersistentFunctionId>,
    source_signature: crate::DecodedExactCallableSignature,
    source_signature_fingerprint: DecodedPersistentId<SourceSignatureFingerprint>,
    main: DecodedPersistentId<PersistentCallableBodyId>,
}

impl DecodedExecutableSourceEntryIdentity {
    pub const fn declaration(&self) -> DecodedPersistentId<PersistentFunctionId> {
        self.declaration
    }
}

impl WireEncode for DecodedExecutableSourceEntryIdentity {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        self.root_cone.encode(encoder)?;
        encoder.field(2)?;
        self.declaration.encode(encoder)?;
        encoder.field(3)?;
        self.source_signature.encode(encoder)?;
        encoder.field(4)?;
        self.source_signature_fingerprint.encode(encoder)?;
        encoder.field(5)?;
        self.main.encode(encoder)
    }
}

impl WireDecode for DecodedExecutableSourceEntryIdentity {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(5)?;
        Ok(Self {
            root_cone: decoder.field(1, DecodedPersistentId::decode)?,
            declaration: decoder.field(2, DecodedPersistentId::decode)?,
            source_signature: decoder.field(3, crate::DecodedExactCallableSignature::decode)?,
            source_signature_fingerprint: decoder.field(4, DecodedPersistentId::decode)?,
            main: decoder.field(5, DecodedPersistentId::decode)?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutableSourceEntryIdentityError {
    NotFunction,
    NotTopLevel,
    NotMain,
    InvalidDeclarationShape,
    Hash(HashError),
}

impl fmt::Display for ExecutableSourceEntryIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid executable source entry identity: {self:?}"
        )
    }
}

impl std::error::Error for ExecutableSourceEntryIdentityError {}

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

fn encode_wire_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encoder.field(0)?;
    encoder.unsigned(tag)?;
    encoder.field(1)?;
    value.encode(encoder)
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
mod tests;
