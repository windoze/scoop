use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{FieldIdentityError, FieldIdentityKey};
use crate::{
    CanonicalIdentifierError, DecodedCanonicalIdentifier, DecodedNominalDeclarationOwner,
    DecodedPersistentId, GeneratedNominalKey, PersistentGenericTypeId, PersistentIdResolver,
    PersistentKeyResolver, PersistentLocalValueId, PersistentPropertyId, PersistentTypeId,
    SourceDeclarationKey,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedSourceFieldKey {
    Declared {
        owner: DecodedNominalDeclarationOwner,
        name: DecodedCanonicalIdentifier,
    },
    PropertyBacking {
        owner: DecodedNominalDeclarationOwner,
        property: DecodedPersistentId<PersistentPropertyId>,
    },
    PropertyDelegate {
        owner: DecodedNominalDeclarationOwner,
        property: DecodedPersistentId<PersistentPropertyId>,
    },
}

impl WireEncode for DecodedSourceFieldKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Declared { owner, name } => encode_two_value_sum(encoder, 1, owner, name),
            Self::PropertyBacking { owner, property } => {
                encode_two_value_sum(encoder, 2, owner, property)
            }
            Self::PropertyDelegate { owner, property } => {
                encode_two_value_sum(encoder, 3, owner, property)
            }
        }
    }
}

impl WireDecode for DecodedSourceFieldKey {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        expect_sum_length(decoder, fields, 3)?;
        match tag {
            1 => Ok(Self::Declared {
                owner: decoder.field(1, DecodedNominalDeclarationOwner::decode)?,
                name: decoder.field(2, DecodedCanonicalIdentifier::decode)?,
            }),
            2 => Ok(Self::PropertyBacking {
                owner: decoder.field(1, DecodedNominalDeclarationOwner::decode)?,
                property: decoder.field(2, DecodedPersistentId::decode)?,
            }),
            3 => Ok(Self::PropertyDelegate {
                owner: decoder.field(1, DecodedNominalDeclarationOwner::decode)?,
                property: decoder.field(2, DecodedPersistentId::decode)?,
            }),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedGeneratedFieldKey {
    BoxPayload,
    ClosureCapture(DecodedPersistentId<PersistentLocalValueId>),
    CallableReferenceReceiver(DecodedPersistentId<PersistentLocalValueId>),
    CoroutineFrameState,
    CoroutineFrameCompletion,
    CoroutineFrameSaved(DecodedPersistentId<PersistentLocalValueId>),
    CoroutineFrameFailure,
    CoroutineAdapterFrame,
    CoroutineAdapterState,
    CoroutineAdapterResult,
    CoroutineAdapterFailure,
    FunctionAdapterSource,
    ObjectBackingProperty(DecodedPersistentId<PersistentPropertyId>),
}

impl WireEncode for DecodedGeneratedFieldKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::BoxPayload => encode_empty_sum(encoder, 1),
            Self::ClosureCapture(value) => encode_value_sum(encoder, 2, value),
            Self::CallableReferenceReceiver(value) => encode_value_sum(encoder, 3, value),
            Self::CoroutineFrameState => encode_empty_sum(encoder, 4),
            Self::CoroutineFrameCompletion => encode_empty_sum(encoder, 5),
            Self::CoroutineFrameSaved(value) => encode_value_sum(encoder, 6, value),
            Self::CoroutineFrameFailure => encode_empty_sum(encoder, 7),
            Self::CoroutineAdapterFrame => encode_empty_sum(encoder, 8),
            Self::CoroutineAdapterState => encode_empty_sum(encoder, 9),
            Self::CoroutineAdapterResult => encode_empty_sum(encoder, 10),
            Self::CoroutineAdapterFailure => encode_empty_sum(encoder, 11),
            Self::FunctionAdapterSource => encode_empty_sum(encoder, 12),
            Self::ObjectBackingProperty(property) => encode_value_sum(encoder, 13, property),
        }
    }
}

impl WireDecode for DecodedGeneratedFieldKey {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        match tag {
            1 => decode_empty_variant(decoder, fields, Self::BoxPayload),
            2 => decode_id_variant(decoder, fields, Self::ClosureCapture),
            3 => decode_id_variant(decoder, fields, Self::CallableReferenceReceiver),
            4 => decode_empty_variant(decoder, fields, Self::CoroutineFrameState),
            5 => decode_empty_variant(decoder, fields, Self::CoroutineFrameCompletion),
            6 => decode_id_variant(decoder, fields, Self::CoroutineFrameSaved),
            7 => decode_empty_variant(decoder, fields, Self::CoroutineFrameFailure),
            8 => decode_empty_variant(decoder, fields, Self::CoroutineAdapterFrame),
            9 => decode_empty_variant(decoder, fields, Self::CoroutineAdapterState),
            10 => decode_empty_variant(decoder, fields, Self::CoroutineAdapterResult),
            11 => decode_empty_variant(decoder, fields, Self::CoroutineAdapterFailure),
            12 => decode_empty_variant(decoder, fields, Self::FunctionAdapterSource),
            13 => decode_id_variant(decoder, fields, Self::ObjectBackingProperty),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedFieldIdentityKey {
    Source(DecodedSourceFieldKey),
    Generated {
        owner: DecodedPersistentId<PersistentTypeId>,
        key: DecodedGeneratedFieldKey,
    },
}

impl DecodedFieldIdentityKey {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<FieldIdentityKey, FieldIdentityResolutionError<E>>
    where
        R: PersistentKeyResolver<PersistentTypeId, SourceDeclarationKey, Error = E>
            + PersistentKeyResolver<PersistentGenericTypeId, SourceDeclarationKey, Error = E>
            + PersistentKeyResolver<PersistentTypeId, GeneratedNominalKey, Error = E>
            + PersistentIdResolver<PersistentPropertyId, Error = E>
            + PersistentIdResolver<PersistentLocalValueId, Error = E>,
    {
        match self {
            Self::Source(key) => resolve_source_field(key, resolver),
            Self::Generated { owner, key } => {
                let owner = resolver
                    .resolve_key(owner)
                    .map_err(FieldIdentityResolutionError::Reference)?;
                resolve_generated_field(&owner, key, resolver)
            }
        }
    }
}

impl WireEncode for DecodedFieldIdentityKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Source(key) => encode_value_sum(encoder, 1, key),
            Self::Generated { owner, key } => encode_two_value_sum(encoder, 2, owner, key),
        }
    }
}

impl WireDecode for DecodedFieldIdentityKey {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedSourceFieldKey::decode)
                    .map(Self::Source)
            }
            2 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::Generated {
                    owner: decoder.field(1, DecodedPersistentId::decode)?,
                    key: decoder.field(2, DecodedGeneratedFieldKey::decode)?,
                })
            }
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FieldIdentityResolutionError<E> {
    Reference(E),
    Name(CanonicalIdentifierError),
    Key(FieldIdentityError),
}

impl<E: fmt::Display> fmt::Display for FieldIdentityResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(error) => error.fmt(formatter),
            Self::Name(error) => error.fmt(formatter),
            Self::Key(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for FieldIdentityResolutionError<E> {}

fn resolve_source_field<R, E>(
    key: DecodedSourceFieldKey,
    resolver: &mut R,
) -> Result<FieldIdentityKey, FieldIdentityResolutionError<E>>
where
    R: PersistentKeyResolver<PersistentTypeId, SourceDeclarationKey, Error = E>
        + PersistentKeyResolver<PersistentGenericTypeId, SourceDeclarationKey, Error = E>
        + PersistentIdResolver<PersistentPropertyId, Error = E>,
{
    match key {
        DecodedSourceFieldKey::Declared { owner, name } => {
            let owner = resolve_source_owner(owner, resolver)?;
            let name = name
                .validate()
                .map_err(FieldIdentityResolutionError::Name)?;
            FieldIdentityKey::source_declared(&owner, name)
                .map_err(FieldIdentityResolutionError::Key)
        }
        DecodedSourceFieldKey::PropertyBacking { owner, property } => {
            let owner = resolve_source_owner(owner, resolver)?;
            let property = resolver
                .resolve(property)
                .map_err(FieldIdentityResolutionError::Reference)?;
            FieldIdentityKey::source_property_backing(&owner, property)
                .map_err(FieldIdentityResolutionError::Key)
        }
        DecodedSourceFieldKey::PropertyDelegate { owner, property } => {
            let owner = resolve_source_owner(owner, resolver)?;
            let property = resolver
                .resolve(property)
                .map_err(FieldIdentityResolutionError::Reference)?;
            FieldIdentityKey::source_property_delegate(&owner, property)
                .map_err(FieldIdentityResolutionError::Key)
        }
    }
}

fn resolve_source_owner<R, E>(
    owner: DecodedNominalDeclarationOwner,
    resolver: &mut R,
) -> Result<SourceDeclarationKey, FieldIdentityResolutionError<E>>
where
    R: PersistentKeyResolver<PersistentTypeId, SourceDeclarationKey, Error = E>
        + PersistentKeyResolver<PersistentGenericTypeId, SourceDeclarationKey, Error = E>,
{
    match owner {
        DecodedNominalDeclarationOwner::Concrete(id) => resolver.resolve_key(id),
        DecodedNominalDeclarationOwner::GenericTemplate(id) => resolver.resolve_key(id),
    }
    .map_err(FieldIdentityResolutionError::Reference)
}

fn resolve_generated_field<R, E>(
    owner: &GeneratedNominalKey,
    key: DecodedGeneratedFieldKey,
    resolver: &mut R,
) -> Result<FieldIdentityKey, FieldIdentityResolutionError<E>>
where
    R: PersistentIdResolver<PersistentPropertyId, Error = E>
        + PersistentIdResolver<PersistentLocalValueId, Error = E>,
{
    match key {
        DecodedGeneratedFieldKey::BoxPayload => {
            FieldIdentityKey::box_payload(owner).map_err(FieldIdentityResolutionError::Key)
        }
        DecodedGeneratedFieldKey::ClosureCapture(value) => resolver
            .resolve(value)
            .map_err(FieldIdentityResolutionError::Reference)
            .and_then(|value| {
                FieldIdentityKey::closure_capture(owner, value)
                    .map_err(FieldIdentityResolutionError::Key)
            }),
        DecodedGeneratedFieldKey::CallableReferenceReceiver(value) => resolver
            .resolve(value)
            .map_err(FieldIdentityResolutionError::Reference)
            .and_then(|value| {
                FieldIdentityKey::callable_reference_receiver(owner, value)
                    .map_err(FieldIdentityResolutionError::Key)
            }),
        DecodedGeneratedFieldKey::CoroutineFrameState => {
            FieldIdentityKey::coroutine_frame_state(owner)
                .map_err(FieldIdentityResolutionError::Key)
        }
        DecodedGeneratedFieldKey::CoroutineFrameCompletion => {
            FieldIdentityKey::coroutine_frame_completion(owner)
                .map_err(FieldIdentityResolutionError::Key)
        }
        DecodedGeneratedFieldKey::CoroutineFrameSaved(value) => resolver
            .resolve(value)
            .map_err(FieldIdentityResolutionError::Reference)
            .and_then(|value| {
                FieldIdentityKey::coroutine_frame_saved(owner, value)
                    .map_err(FieldIdentityResolutionError::Key)
            }),
        DecodedGeneratedFieldKey::CoroutineFrameFailure => {
            FieldIdentityKey::coroutine_frame_failure(owner)
                .map_err(FieldIdentityResolutionError::Key)
        }
        DecodedGeneratedFieldKey::CoroutineAdapterFrame => {
            FieldIdentityKey::coroutine_adapter_frame(owner)
                .map_err(FieldIdentityResolutionError::Key)
        }
        DecodedGeneratedFieldKey::CoroutineAdapterState => {
            FieldIdentityKey::coroutine_adapter_state(owner)
                .map_err(FieldIdentityResolutionError::Key)
        }
        DecodedGeneratedFieldKey::CoroutineAdapterResult => {
            FieldIdentityKey::coroutine_adapter_result(owner)
                .map_err(FieldIdentityResolutionError::Key)
        }
        DecodedGeneratedFieldKey::CoroutineAdapterFailure => {
            FieldIdentityKey::coroutine_adapter_failure(owner)
                .map_err(FieldIdentityResolutionError::Key)
        }
        DecodedGeneratedFieldKey::FunctionAdapterSource => {
            FieldIdentityKey::function_adapter_source(owner)
                .map_err(FieldIdentityResolutionError::Key)
        }
        DecodedGeneratedFieldKey::ObjectBackingProperty(property) => resolver
            .resolve(property)
            .map_err(FieldIdentityResolutionError::Reference)
            .and_then(|property| {
                FieldIdentityKey::object_backing_property(owner, property)
                    .map_err(FieldIdentityResolutionError::Key)
            }),
    }
}

fn decode_sum_header(decoder: &mut Decoder<'_, '_>) -> Result<(u64, u64), WireError> {
    let fields = decoder.map()?;
    let tag = decoder.field(0, Decoder::unsigned)?;
    Ok((fields, tag))
}

fn decode_empty_variant<T>(
    decoder: &Decoder<'_, '_>,
    fields: u64,
    value: T,
) -> Result<T, WireError> {
    expect_sum_length(decoder, fields, 1)?;
    Ok(value)
}

fn decode_id_variant<I, T>(
    decoder: &mut Decoder<'_, '_>,
    fields: u64,
    build: impl FnOnce(DecodedPersistentId<I>) -> T,
) -> Result<T, WireError>
where
    I: crate::PersistentId,
{
    expect_sum_length(decoder, fields, 2)?;
    decoder.field(1, DecodedPersistentId::decode).map(build)
}

fn expect_sum_length(
    decoder: &Decoder<'_, '_>,
    actual: u64,
    expected: u64,
) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(WireError::new(
            WireErrorKind::InvalidLength { expected, actual },
            decoder.path().clone(),
            Some(decoder.position()),
        ))
    }
}

fn unknown_tag(decoder: &Decoder<'_, '_>, tag: u64) -> WireError {
    WireError::new(
        WireErrorKind::UnknownTag { tag },
        decoder.path().clone(),
        Some(decoder.position()),
    )
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encode_tag(encoder, tag)
}

fn encode_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

fn encode_two_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    first: &impl WireEncode,
    second: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    first.encode(encoder)?;
    encoder.field(2)?;
    second.encode(encoder)
}

#[cfg(test)]
mod tests;
