use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{OdrIdentityResolutionError, SpecializationResolver};
use crate::{
    DecodedCallableApplicationKey, DecodedPersistentId, NonEmptyVec, PersistentExactTypeId,
    PersistentExtensionPropertyId, PersistentGenericTypeId, PersistentIdResolver,
    SpecializationKey,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedSpecializationKey {
    Nominal {
        origin: DecodedPersistentId<PersistentGenericTypeId>,
        arguments: NonEmptyVec<DecodedPersistentId<PersistentExactTypeId>>,
    },
    Callable {
        application: DecodedCallableApplicationKey,
    },
    DelegatedProperty {
        origin: DecodedPersistentId<PersistentExtensionPropertyId>,
        receiver_arguments: NonEmptyVec<DecodedPersistentId<PersistentExactTypeId>>,
    },
    StructuralType {
        exact_type: DecodedPersistentId<PersistentExactTypeId>,
    },
}

impl DecodedSpecializationKey {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<SpecializationKey, OdrIdentityResolutionError<E>>
    where
        R: SpecializationResolver<E>,
    {
        match self {
            Self::Nominal { origin, arguments } => Ok(SpecializationKey::Nominal {
                origin: resolver
                    .resolve(origin)
                    .map_err(OdrIdentityResolutionError::Reference)?,
                arguments: resolve_exact_types(arguments, resolver)?,
            }),
            Self::Callable { application } => application
                .resolve(resolver)
                .map(|application| SpecializationKey::Callable { application })
                .map_err(OdrIdentityResolutionError::Callable),
            Self::DelegatedProperty {
                origin,
                receiver_arguments,
            } => Ok(SpecializationKey::DelegatedProperty {
                origin: resolver
                    .resolve(origin)
                    .map_err(OdrIdentityResolutionError::Reference)?,
                receiver_arguments: resolve_exact_types(receiver_arguments, resolver)?,
            }),
            Self::StructuralType { exact_type } => resolver
                .resolve(exact_type)
                .map(|exact_type| SpecializationKey::StructuralType { exact_type })
                .map_err(OdrIdentityResolutionError::Reference),
        }
    }
}

impl WireEncode for DecodedSpecializationKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Nominal { origin, arguments } => {
                encode_specialization_arguments(encoder, 1, origin, arguments)
            }
            Self::Callable { application } => encode_value_sum(encoder, 2, application),
            Self::DelegatedProperty {
                origin,
                receiver_arguments,
            } => encode_specialization_arguments(encoder, 3, origin, receiver_arguments),
            Self::StructuralType { exact_type } => encode_value_sum(encoder, 4, exact_type),
        }
    }
}

impl WireDecode for DecodedSpecializationKey {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                let (origin, arguments) = decode_specialization_arguments(decoder, fields)?;
                Ok(Self::Nominal { origin, arguments })
            }
            2 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedCallableApplicationKey::decode)
                    .map(|application| Self::Callable { application })
            }
            3 => {
                let (origin, receiver_arguments) =
                    decode_specialization_arguments(decoder, fields)?;
                Ok(Self::DelegatedProperty {
                    origin,
                    receiver_arguments,
                })
            }
            4 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedPersistentId::decode)
                    .map(|exact_type| Self::StructuralType { exact_type })
            }
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

fn resolve_exact_types<R, E>(
    values: NonEmptyVec<DecodedPersistentId<PersistentExactTypeId>>,
    resolver: &mut R,
) -> Result<NonEmptyVec<PersistentExactTypeId>, OdrIdentityResolutionError<E>>
where
    R: PersistentIdResolver<PersistentExactTypeId, Error = E>,
{
    let mut resolved = Vec::new();
    resolved
        .try_reserve_exact(values.as_slice().len())
        .map_err(|_| OdrIdentityResolutionError::Allocation)?;
    for value in values.as_slice() {
        resolved.push(
            resolver
                .resolve(*value)
                .map_err(OdrIdentityResolutionError::Reference)?,
        );
    }
    NonEmptyVec::new(resolved).map_err(|_| OdrIdentityResolutionError::EmptyArguments)
}

fn decode_specialization_arguments<I>(
    decoder: &mut Decoder<'_>,
    fields: u64,
) -> Result<
    (
        DecodedPersistentId<I>,
        NonEmptyVec<DecodedPersistentId<PersistentExactTypeId>>,
    ),
    WireError,
>
where
    I: crate::PersistentId,
{
    expect_sum_length(decoder, fields, 3)?;
    let origin = decoder.field(1, DecodedPersistentId::decode)?;
    let arguments = decoder.field(2, |decoder| {
        decoder.decode_array(|decoder, _| DecodedPersistentId::decode(decoder))
    })?;
    let arguments = NonEmptyVec::new(arguments).map_err(|_| invalid_empty_sequence(decoder))?;
    Ok((origin, arguments))
}

fn encode_specialization_arguments<I>(
    encoder: &mut Encoder,
    tag: u64,
    origin: &DecodedPersistentId<I>,
    arguments: &NonEmptyVec<DecodedPersistentId<PersistentExactTypeId>>,
) -> Result<(), scoop_wire::cbor::EncodeError>
where
    I: crate::PersistentId,
{
    encoder.map(3)?;
    encoder.field(0)?;
    encoder.unsigned(tag)?;
    encoder.field(1)?;
    origin.encode(encoder)?;
    encoder.field(2)?;
    encoder.array(arguments.as_slice().len() as u64)?;
    for argument in arguments.as_slice() {
        argument.encode(encoder)?;
    }
    Ok(())
}

fn expect_sum_length(decoder: &Decoder<'_>, actual: u64, expected: u64) -> Result<(), WireError> {
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

fn invalid_empty_sequence(decoder: &Decoder<'_>) -> WireError {
    WireError::new(
        WireErrorKind::InvalidLength {
            expected: 1,
            actual: 0,
        },
        decoder.path().clone(),
        Some(decoder.position()),
    )
}

fn unknown_tag(decoder: &Decoder<'_>, tag: u64) -> WireError {
    WireError::new(
        WireErrorKind::UnknownTag { tag },
        decoder.path().clone(),
        Some(decoder.position()),
    )
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
