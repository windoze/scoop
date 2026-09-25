//! Persistent MIR identity metadata shared with the LIR lowering boundary.

use std::cmp::Ordering;
use std::fmt;

use scoop_identity::{
    CallableOdrMemberId, CallableOwner, CallbackMode, DecodedCallableOwner,
    DecodedExactCallableSignature, DecodedPersistentId, ExactCallableSignature,
    ExactCallableSignatureResolutionError, OdrMemberId, PersistentCallableApplicationId,
    PersistentCallbackApplicationId, PersistentConstructorId, PersistentExactTypeId,
    PersistentFunctionId, PersistentGeneratedCallableId, PersistentGenericFunctionId,
    PersistentIdResolver, PersistentPropertyAccessorId,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

/// The persistent implementation whose full managed signature is recorded.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CallableSignatureSubject {
    Strong(CallableOwner),
    Odr(CallableOdrMemberId),
}

impl CallableSignatureSubject {
    pub const fn strong(owner: CallableOwner) -> Self {
        Self::Strong(owner)
    }

    pub const fn odr(member: CallableOdrMemberId) -> Self {
        Self::Odr(member)
    }

    pub const fn kind_tag(self) -> u8 {
        match self {
            Self::Strong(_) => 1,
            Self::Odr(_) => 2,
        }
    }

    pub fn raw_id(self) -> [u8; 32] {
        match self {
            Self::Strong(owner) => callable_owner_raw_id(owner),
            Self::Odr(member) => *member.member().as_array(),
        }
    }

    pub fn compare_sort_key(self, other: Self) -> Ordering {
        self.kind_tag()
            .cmp(&other.kind_tag())
            .then_with(|| self.raw_id().cmp(&other.raw_id()))
    }
}

impl WireEncode for CallableSignatureSubject {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Strong(owner) => encode_value_sum(encoder, 1, owner),
            Self::Odr(member) => encode_value_sum(encoder, 2, member),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableSignatureRecord {
    subject: CallableSignatureSubject,
    signature: ExactCallableSignature,
}

impl CallableSignatureRecord {
    pub const fn new(subject: CallableSignatureSubject, signature: ExactCallableSignature) -> Self {
        Self { subject, signature }
    }

    pub const fn subject(&self) -> CallableSignatureSubject {
        self.subject
    }

    pub const fn signature(&self) -> &ExactCallableSignature {
        &self.signature
    }
}

impl WireEncode for CallableSignatureRecord {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.subject.encode(encoder)?;
        encoder.field(2)?;
        self.signature.encode(encoder)
    }
}

/// The fixed managed adapter storage boundary used by callback materializations.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ForeignCallbackStorageAbi {
    ClosureResultRootsThrowableToU32,
}

impl WireEncode for ForeignCallbackStorageAbi {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(1)
    }
}

impl WireDecode for ForeignCallbackStorageAbi {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::ClosureResultRootsThrowableToU32),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallbackApplicationRecord {
    application: PersistentCallbackApplicationId,
    managed_adapter: CallableSignatureSubject,
    managed_signature: ExactCallableSignature,
    storage_abi: ForeignCallbackStorageAbi,
    mode: CallbackMode,
}

impl CallbackApplicationRecord {
    pub const fn new(
        application: PersistentCallbackApplicationId,
        managed_adapter: CallableSignatureSubject,
        managed_signature: ExactCallableSignature,
        storage_abi: ForeignCallbackStorageAbi,
        mode: CallbackMode,
    ) -> Self {
        Self {
            application,
            managed_adapter,
            managed_signature,
            storage_abi,
            mode,
        }
    }

    pub const fn application(&self) -> PersistentCallbackApplicationId {
        self.application
    }

    pub const fn managed_adapter(&self) -> CallableSignatureSubject {
        self.managed_adapter
    }

    pub const fn managed_signature(&self) -> &ExactCallableSignature {
        &self.managed_signature
    }

    pub const fn storage_abi(&self) -> ForeignCallbackStorageAbi {
        self.storage_abi
    }

    pub const fn mode(&self) -> CallbackMode {
        self.mode
    }
}

impl WireEncode for CallbackApplicationRecord {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        self.application.encode(encoder)?;
        encoder.field(2)?;
        self.managed_adapter.encode(encoder)?;
        encoder.field(3)?;
        self.managed_signature.encode(encoder)?;
        encoder.field(4)?;
        self.storage_abi.encode(encoder)?;
        encoder.field(5)?;
        self.mode.encode(encoder)
    }
}

/// Resolves every typed identity used by a callable signature record.
pub trait CallableSignatureResolver<E>:
    PersistentIdResolver<PersistentFunctionId, Error = E>
    + PersistentIdResolver<PersistentGenericFunctionId, Error = E>
    + PersistentIdResolver<PersistentCallableApplicationId, Error = E>
    + PersistentIdResolver<PersistentConstructorId, Error = E>
    + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
    + PersistentIdResolver<PersistentGeneratedCallableId, Error = E>
    + PersistentIdResolver<PersistentExactTypeId, Error = E>
{
    fn resolve_callable_odr_member(
        &mut self,
        member: DecodedPersistentId<OdrMemberId>,
    ) -> Result<CallableOdrMemberId, E>;
}

pub trait CallbackApplicationResolver<E>:
    CallableSignatureResolver<E> + PersistentIdResolver<PersistentCallbackApplicationId, Error = E>
{
}

impl<T, E> CallbackApplicationResolver<E> for T where
    T: CallableSignatureResolver<E>
        + PersistentIdResolver<PersistentCallbackApplicationId, Error = E>
{
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DecodedCallableSignatureSubject {
    Strong(DecodedCallableOwner),
    Odr(DecodedPersistentId<OdrMemberId>),
}

impl DecodedCallableSignatureSubject {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<CallableSignatureSubject, E>
    where
        R: CallableSignatureResolver<E>,
    {
        match self {
            Self::Strong(owner) => owner
                .resolve(resolver)
                .map(CallableSignatureSubject::Strong),
            Self::Odr(member) => resolver
                .resolve_callable_odr_member(member)
                .map(CallableSignatureSubject::Odr),
        }
    }
}

impl WireEncode for DecodedCallableSignatureSubject {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Strong(owner) => encode_value_sum(encoder, 1, owner),
            Self::Odr(member) => encode_value_sum(encoder, 2, member),
        }
    }
}

impl WireDecode for DecodedCallableSignatureSubject {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedCallableOwner::decode)
                    .map(Self::Strong)
            }
            2 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder.field(1, DecodedPersistentId::decode).map(Self::Odr)
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCallableSignatureRecord {
    subject: DecodedCallableSignatureSubject,
    signature: DecodedExactCallableSignature,
}

impl DecodedCallableSignatureRecord {
    pub const fn subject(&self) -> DecodedCallableSignatureSubject {
        self.subject
    }

    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CallableSignatureRecord, CallableSignatureResolutionError<E>>
    where
        R: CallableSignatureResolver<E>,
    {
        let subject = self
            .subject
            .resolve(resolver)
            .map_err(CallableSignatureResolutionError::Subject)?;
        let signature = self
            .signature
            .resolve(resolver)
            .map_err(CallableSignatureResolutionError::Signature)?;
        Ok(CallableSignatureRecord::new(subject, signature))
    }
}

impl WireEncode for DecodedCallableSignatureRecord {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.subject.encode(encoder)?;
        encoder.field(2)?;
        self.signature.encode(encoder)
    }
}

impl WireDecode for DecodedCallableSignatureRecord {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            subject: decoder.field(1, DecodedCallableSignatureSubject::decode)?,
            signature: decoder.field(2, DecodedExactCallableSignature::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCallbackApplicationRecord {
    application: DecodedPersistentId<PersistentCallbackApplicationId>,
    managed_adapter: DecodedCallableSignatureSubject,
    managed_signature: DecodedExactCallableSignature,
    storage_abi: ForeignCallbackStorageAbi,
    mode: CallbackMode,
}

impl DecodedCallbackApplicationRecord {
    pub const fn decoded_application(
        &self,
    ) -> DecodedPersistentId<PersistentCallbackApplicationId> {
        self.application
    }

    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CallbackApplicationRecord, CallbackApplicationResolutionError<E>>
    where
        R: CallbackApplicationResolver<E>,
    {
        let application = resolver
            .resolve(self.application)
            .map_err(CallbackApplicationResolutionError::Application)?;
        let managed_adapter = self
            .managed_adapter
            .resolve(resolver)
            .map_err(CallbackApplicationResolutionError::ManagedAdapter)?;
        let managed_signature = self
            .managed_signature
            .resolve(resolver)
            .map_err(CallbackApplicationResolutionError::ManagedSignature)?;
        Ok(CallbackApplicationRecord::new(
            application,
            managed_adapter,
            managed_signature,
            self.storage_abi,
            self.mode,
        ))
    }
}

impl WireEncode for DecodedCallbackApplicationRecord {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        self.application.encode(encoder)?;
        encoder.field(2)?;
        self.managed_adapter.encode(encoder)?;
        encoder.field(3)?;
        self.managed_signature.encode(encoder)?;
        encoder.field(4)?;
        self.storage_abi.encode(encoder)?;
        encoder.field(5)?;
        self.mode.encode(encoder)
    }
}

impl WireDecode for DecodedCallbackApplicationRecord {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(5)?;
        Ok(Self {
            application: decoder.field(1, DecodedPersistentId::decode)?,
            managed_adapter: decoder.field(2, DecodedCallableSignatureSubject::decode)?,
            managed_signature: decoder.field(3, DecodedExactCallableSignature::decode)?,
            storage_abi: decoder.field(4, ForeignCallbackStorageAbi::decode)?,
            mode: decoder.field(5, CallbackMode::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableSignatureResolutionError<E> {
    Subject(E),
    Signature(ExactCallableSignatureResolutionError<E>),
}

impl<E: fmt::Display> fmt::Display for CallableSignatureResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Subject(error) => error.fmt(formatter),
            Self::Signature(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for CallableSignatureResolutionError<E> {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallbackApplicationResolutionError<E> {
    Application(E),
    ManagedAdapter(E),
    ManagedSignature(ExactCallableSignatureResolutionError<E>),
}

impl<E: fmt::Display> fmt::Display for CallbackApplicationResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Application(error) | Self::ManagedAdapter(error) => error.fmt(formatter),
            Self::ManagedSignature(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for CallbackApplicationResolutionError<E> {}

fn callable_owner_raw_id(owner: CallableOwner) -> [u8; 32] {
    match owner {
        CallableOwner::Function(id) => *id.as_array(),
        CallableOwner::GenericTemplate(id) => *id.as_array(),
        CallableOwner::Application(id) => *id.as_array(),
        CallableOwner::Constructor(id) => *id.as_array(),
        CallableOwner::Accessor(id) => *id.as_array(),
        CallableOwner::Generated(id) => *id.as_array(),
    }
}

fn encode_value_sum(
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

fn expect_sum_length(decoder: &Decoder<'_>, actual: u64, expected: u64) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(wire_error(
            decoder,
            WireErrorKind::InvalidLength { actual, expected },
        ))
    }
}

fn wire_error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

#[cfg(test)]
mod tests;
