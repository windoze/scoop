use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{InitializationUnitKey, LocalValueKey, LocalValueSelector, SyntheticLocalRole};
use crate::{
    DecodedCallableMaterialization, DecodedPersistentId, NonEmptyVec,
    PersistentCallableApplicationId, PersistentConstructorId, PersistentExactTypeId,
    PersistentExtensionPropertyId, PersistentFunctionId, PersistentGeneratedCallableId,
    PersistentGenericFunctionId, PersistentIdResolver, PersistentInitializationUnitId,
    PersistentPropertyAccessorId, PersistentPropertyId, PersistentTypeId, StructuralDefinitionPath,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedInitializationUnitKey {
    TopLevelProperty(DecodedPersistentId<PersistentPropertyId>),
    ExtensionProperty(DecodedPersistentId<PersistentExtensionPropertyId>),
    Object(DecodedPersistentId<PersistentTypeId>),
    Companion(DecodedPersistentId<PersistentTypeId>),
    GenericDelegatedExtensionApplication {
        property: DecodedPersistentId<PersistentExtensionPropertyId>,
        receiver_arguments: NonEmptyVec<DecodedPersistentId<PersistentExactTypeId>>,
    },
}

impl DecodedInitializationUnitKey {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<InitializationUnitKey, InitializationUnitResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentPropertyId, Error = E>
            + PersistentIdResolver<PersistentExtensionPropertyId, Error = E>
            + PersistentIdResolver<PersistentTypeId, Error = E>
            + PersistentIdResolver<PersistentExactTypeId, Error = E>,
    {
        match self {
            Self::TopLevelProperty(id) => {
                resolve_id(resolver, id).map(InitializationUnitKey::TopLevelProperty)
            }
            Self::ExtensionProperty(id) => {
                resolve_id(resolver, id).map(InitializationUnitKey::ExtensionProperty)
            }
            Self::Object(id) => resolve_id(resolver, id).map(InitializationUnitKey::Object),
            Self::Companion(id) => resolve_id(resolver, id).map(InitializationUnitKey::Companion),
            Self::GenericDelegatedExtensionApplication {
                property,
                receiver_arguments,
            } => Ok(
                InitializationUnitKey::GenericDelegatedExtensionApplication {
                    property: resolve_id(resolver, property)?,
                    receiver_arguments: resolve_non_empty(receiver_arguments, resolver)?,
                },
            ),
        }
    }
}

impl WireEncode for DecodedInitializationUnitKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::TopLevelProperty(id) => encode_value_sum(encoder, 1, id),
            Self::ExtensionProperty(id) => encode_value_sum(encoder, 2, id),
            Self::Object(id) => encode_value_sum(encoder, 3, id),
            Self::Companion(id) => encode_value_sum(encoder, 4, id),
            Self::GenericDelegatedExtensionApplication {
                property,
                receiver_arguments,
            } => {
                encoder.map(3)?;
                encode_tag(encoder, 5)?;
                encoder.field(1)?;
                property.encode(encoder)?;
                encoder.field(2)?;
                encode_sequence(encoder, receiver_arguments.as_slice())
            }
        }
    }
}

impl WireDecode for DecodedInitializationUnitKey {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        match tag {
            1 => decode_id_variant(decoder, fields, Self::TopLevelProperty),
            2 => decode_id_variant(decoder, fields, Self::ExtensionProperty),
            3 => decode_id_variant(decoder, fields, Self::Object),
            4 => decode_id_variant(decoder, fields, Self::Companion),
            5 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::GenericDelegatedExtensionApplication {
                    property: decoder.field(1, DecodedPersistentId::decode)?,
                    receiver_arguments: decoder.field(2, decode_non_empty_exact_ids)?,
                })
            }
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedLocalValueKey {
    owner: DecodedCallableMaterialization,
    selector: LocalValueSelector,
}

impl DecodedLocalValueKey {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<LocalValueKey, LocalValueResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentFunctionId, Error = E>
            + PersistentIdResolver<PersistentGenericFunctionId, Error = E>
            + PersistentIdResolver<PersistentConstructorId, Error = E>
            + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
            + PersistentIdResolver<crate::PersistentEnumVariantId, Error = E>
            + PersistentIdResolver<PersistentGeneratedCallableId, Error = E>
            + PersistentIdResolver<PersistentCallableApplicationId, Error = E>
            + PersistentIdResolver<PersistentInitializationUnitId, Error = E>,
    {
        let owner = self
            .owner
            .resolve(resolver)
            .map_err(LocalValueResolutionError::Reference)?;
        Ok(LocalValueKey::new(owner, self.selector))
    }
}

impl WireEncode for DecodedLocalValueKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.selector.encode(encoder)
    }
}

impl WireDecode for DecodedLocalValueKey {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            owner: decoder.field(1, DecodedCallableMaterialization::decode)?,
            selector: decoder.field(2, LocalValueSelector::decode)?,
        })
    }
}

impl WireDecode for LocalValueSelector {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::This)
            }
            2 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, Decoder::u32)
                    .map(|declaration_index| Self::Parameter { declaration_index })
            }
            3 => decode_path_variant(decoder, fields, |path| Self::LocalDeclaration { path }),
            4 => decode_path_variant(decoder, fields, |path| Self::BoundReceiver { path }),
            5 => decode_path_variant(decoder, fields, |site| Self::SuspensionResult { site }),
            6 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::Synthetic {
                    path: decoder.field(1, StructuralDefinitionPath::decode)?,
                    role: decoder.field(2, SyntheticLocalRole::decode)?,
                })
            }
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

impl WireDecode for SyntheticLocalRole {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::Temporary),
            2 => Ok(Self::DefaultValue),
            3 => Ok(Self::DesugaredIterator),
            4 => Ok(Self::CoroutineProtocol),
            5 => Ok(Self::CallbackContext),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InitializationUnitResolutionError<E> {
    Reference(E),
    Allocation,
    EmptyArguments,
}

impl<E: fmt::Display> fmt::Display for InitializationUnitResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(error) => error.fmt(formatter),
            Self::Allocation => {
                formatter.write_str("failed to allocate resolved initialization arguments")
            }
            Self::EmptyArguments => {
                formatter.write_str("generic delegated initialization arguments must not be empty")
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for InitializationUnitResolutionError<E> {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LocalValueResolutionError<E> {
    Reference(E),
}

impl<E: fmt::Display> fmt::Display for LocalValueResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for LocalValueResolutionError<E> {}

fn resolve_id<R, I, E>(
    resolver: &mut R,
    id: DecodedPersistentId<I>,
) -> Result<I, InitializationUnitResolutionError<E>>
where
    I: crate::PersistentId,
    R: PersistentIdResolver<I, Error = E>,
{
    resolver
        .resolve(id)
        .map_err(InitializationUnitResolutionError::Reference)
}

fn resolve_non_empty<R, E>(
    values: NonEmptyVec<DecodedPersistentId<PersistentExactTypeId>>,
    resolver: &mut R,
) -> Result<NonEmptyVec<PersistentExactTypeId>, InitializationUnitResolutionError<E>>
where
    R: PersistentIdResolver<PersistentExactTypeId, Error = E>,
{
    let values = values.as_slice();
    let mut resolved = Vec::new();
    resolved
        .try_reserve_exact(values.len())
        .map_err(|_| InitializationUnitResolutionError::Allocation)?;
    for value in values {
        resolved.push(
            resolver
                .resolve(*value)
                .map_err(InitializationUnitResolutionError::Reference)?,
        );
    }
    NonEmptyVec::new(resolved).map_err(|_| InitializationUnitResolutionError::EmptyArguments)
}

fn decode_non_empty_exact_ids(
    decoder: &mut Decoder<'_, '_>,
) -> Result<NonEmptyVec<DecodedPersistentId<PersistentExactTypeId>>, WireError> {
    let values = decoder.decode_array(|decoder, _| DecodedPersistentId::decode(decoder))?;
    NonEmptyVec::new(values).map_err(|_| {
        WireError::new(
            WireErrorKind::InvalidLength {
                expected: 1,
                actual: 0,
            },
            decoder.path().clone(),
            Some(decoder.position()),
        )
    })
}

fn decode_sum_header(decoder: &mut Decoder<'_, '_>) -> Result<(u64, u64), WireError> {
    let fields = decoder.map()?;
    let tag = decoder.field(0, Decoder::unsigned)?;
    Ok((fields, tag))
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

fn decode_path_variant<T>(
    decoder: &mut Decoder<'_, '_>,
    fields: u64,
    build: impl FnOnce(StructuralDefinitionPath) -> T,
) -> Result<T, WireError> {
    expect_sum_length(decoder, fields, 2)?;
    decoder
        .field(1, StructuralDefinitionPath::decode)
        .map(build)
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

fn encode_sequence<T: WireEncode>(
    encoder: &mut Encoder,
    values: &[T],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
