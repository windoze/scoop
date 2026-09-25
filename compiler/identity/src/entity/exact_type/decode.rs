use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::ExactTypeKey;
use crate::{
    CallingConvention, DecodedPersistentId, Effect, NonEmptyVec, PersistentExactTypeId,
    PersistentGenericTypeId, PersistentIdResolver, PersistentTypeId,
};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DecodedExactTypeKey {
    Nominal(DecodedPersistentId<PersistentTypeId>),
    NominalApplication {
        origin: DecodedPersistentId<PersistentGenericTypeId>,
        arguments: NonEmptyVec<DecodedPersistentId<PersistentExactTypeId>>,
    },
    Tuple(NonEmptyVec<DecodedPersistentId<PersistentExactTypeId>>),
    Function {
        effect: Effect,
        parameters: Vec<DecodedPersistentId<PersistentExactTypeId>>,
        result: DecodedPersistentId<PersistentExactTypeId>,
    },
    RawPointer(DecodedPersistentId<PersistentExactTypeId>),
    NativeFunctionPointer {
        calling_convention: CallingConvention,
        parameters: Vec<DecodedPersistentId<PersistentExactTypeId>>,
        result: DecodedPersistentId<PersistentExactTypeId>,
    },
}

impl DecodedExactTypeKey {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<ExactTypeKey, ExactTypeResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentTypeId, Error = E>
            + PersistentIdResolver<PersistentGenericTypeId, Error = E>
            + PersistentIdResolver<PersistentExactTypeId, Error = E>,
    {
        match self {
            Self::Nominal(id) => resolver
                .resolve(id)
                .map(ExactTypeKey::Nominal)
                .map_err(ExactTypeResolutionError::Reference),
            Self::NominalApplication { origin, arguments } => {
                Ok(ExactTypeKey::NominalApplication {
                    origin: resolver
                        .resolve(origin)
                        .map_err(ExactTypeResolutionError::Reference)?,
                    arguments: resolve_non_empty(arguments, resolver)?,
                })
            }
            Self::Tuple(elements) => resolve_non_empty(elements, resolver).map(ExactTypeKey::Tuple),
            Self::Function {
                effect,
                parameters,
                result,
            } => Ok(ExactTypeKey::Function {
                effect,
                parameters: resolve_sequence(parameters, resolver)?,
                result: resolver
                    .resolve(result)
                    .map_err(ExactTypeResolutionError::Reference)?,
            }),
            Self::RawPointer(pointee) => resolver
                .resolve(pointee)
                .map(ExactTypeKey::RawPointer)
                .map_err(ExactTypeResolutionError::Reference),
            Self::NativeFunctionPointer {
                calling_convention,
                parameters,
                result,
            } => Ok(ExactTypeKey::NativeFunctionPointer {
                calling_convention,
                parameters: resolve_sequence(parameters, resolver)?,
                result: resolver
                    .resolve(result)
                    .map_err(ExactTypeResolutionError::Reference)?,
            }),
        }
    }
}

impl WireEncode for DecodedExactTypeKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Nominal(id) => encode_value_sum(encoder, 1, id),
            Self::NominalApplication { origin, arguments } => {
                encoder.map(3)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                origin.encode(encoder)?;
                encoder.field(2)?;
                encode_sequence(encoder, arguments.as_slice())
            }
            Self::Tuple(elements) => {
                encoder.map(2)?;
                encode_tag(encoder, 3)?;
                encoder.field(1)?;
                encode_sequence(encoder, elements.as_slice())
            }
            Self::Function {
                effect,
                parameters,
                result,
            } => {
                encoder.map(4)?;
                encode_tag(encoder, 4)?;
                encoder.field(1)?;
                effect.encode(encoder)?;
                encoder.field(2)?;
                encode_sequence(encoder, parameters)?;
                encoder.field(3)?;
                result.encode(encoder)
            }
            Self::RawPointer(pointee) => encode_value_sum(encoder, 5, pointee),
            Self::NativeFunctionPointer {
                calling_convention,
                parameters,
                result,
            } => {
                encoder.map(4)?;
                encode_tag(encoder, 6)?;
                encoder.field(1)?;
                calling_convention.encode(encoder)?;
                encoder.field(2)?;
                encode_sequence(encoder, parameters)?;
                encoder.field(3)?;
                result.encode(encoder)
            }
        }
    }
}

impl WireDecode for DecodedExactTypeKey {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedPersistentId::decode)
                    .map(Self::Nominal)
            }
            2 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::NominalApplication {
                    origin: decoder.field(1, DecodedPersistentId::decode)?,
                    arguments: decoder.field(2, decode_non_empty_exact_ids)?,
                })
            }
            3 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, decode_non_empty_exact_ids)
                    .map(Self::Tuple)
            }
            4 => {
                expect_sum_length(decoder, fields, 4)?;
                Ok(Self::Function {
                    effect: decoder.field(1, decode_effect)?,
                    parameters: decoder.field(2, decode_exact_ids)?,
                    result: decoder.field(3, DecodedPersistentId::decode)?,
                })
            }
            5 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedPersistentId::decode)
                    .map(Self::RawPointer)
            }
            6 => {
                expect_sum_length(decoder, fields, 4)?;
                Ok(Self::NativeFunctionPointer {
                    calling_convention: decoder.field(1, decode_calling_convention)?,
                    parameters: decoder.field(2, decode_exact_ids)?,
                    result: decoder.field(3, DecodedPersistentId::decode)?,
                })
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExactTypeResolutionError<E> {
    Reference(E),
    Allocation,
    EmptySequence,
}

impl<E: fmt::Display> fmt::Display for ExactTypeResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(error) => error.fmt(formatter),
            Self::Allocation => formatter.write_str("failed to allocate resolved exact type ids"),
            Self::EmptySequence => formatter.write_str("exact type sequence must not be empty"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for ExactTypeResolutionError<E> {}

fn resolve_non_empty<R, E>(
    values: NonEmptyVec<DecodedPersistentId<PersistentExactTypeId>>,
    resolver: &mut R,
) -> Result<NonEmptyVec<PersistentExactTypeId>, ExactTypeResolutionError<E>>
where
    R: PersistentIdResolver<PersistentExactTypeId, Error = E>,
{
    let resolved = resolve_sequence(values.as_slice().iter().copied(), resolver)?;
    NonEmptyVec::new(resolved).map_err(|_| ExactTypeResolutionError::EmptySequence)
}

fn resolve_sequence<R, E, I>(
    values: I,
    resolver: &mut R,
) -> Result<Vec<PersistentExactTypeId>, ExactTypeResolutionError<E>>
where
    R: PersistentIdResolver<PersistentExactTypeId, Error = E>,
    I: IntoIterator<Item = DecodedPersistentId<PersistentExactTypeId>>,
    I::IntoIter: ExactSizeIterator,
{
    let values = values.into_iter();
    let mut resolved = Vec::new();
    resolved
        .try_reserve_exact(values.len())
        .map_err(|_| ExactTypeResolutionError::Allocation)?;
    for value in values {
        resolved.push(
            resolver
                .resolve(value)
                .map_err(ExactTypeResolutionError::Reference)?,
        );
    }
    Ok(resolved)
}

fn decode_exact_ids(
    decoder: &mut Decoder<'_>,
) -> Result<Vec<DecodedPersistentId<PersistentExactTypeId>>, WireError> {
    decoder.decode_array(|decoder, _| DecodedPersistentId::decode(decoder))
}

fn decode_non_empty_exact_ids(
    decoder: &mut Decoder<'_>,
) -> Result<NonEmptyVec<DecodedPersistentId<PersistentExactTypeId>>, WireError> {
    NonEmptyVec::new(decode_exact_ids(decoder)?).map_err(|_| {
        wire_error(
            decoder,
            WireErrorKind::InvalidLength {
                expected: 1,
                actual: 0,
            },
        )
    })
}

fn decode_effect(decoder: &mut Decoder<'_>) -> Result<Effect, WireError> {
    match decoder.unsigned()? {
        1 => Ok(Effect::Ordinary),
        2 => Ok(Effect::Suspend),
        tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
    }
}

fn decode_calling_convention(decoder: &mut Decoder<'_>) -> Result<CallingConvention, WireError> {
    match decoder.unsigned()? {
        1 => Ok(CallingConvention::C),
        tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
    }
}

fn expect_sum_length(decoder: &Decoder<'_>, actual: u64, expected: u64) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(wire_error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}

fn wire_error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
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
mod tests {
    use scoop_wire::{WireErrorKind, decode_canonical, encode};

    use super::{DecodedExactTypeKey, ExactTypeResolutionError};
    use crate::{
        CborIdentityRecord, DecodedCborIdentityRecord, Effect, ExactTypeKey, NonEmptyVec,
        PersistentExactTypeId, PersistentGenericTypeId, PersistentIdMismatch, PersistentIdResolver,
        PersistentTypeId,
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

                fn resolve(
                    &mut self,
                    id: crate::DecodedPersistentId<$id>,
                ) -> Result<$id, Self::Error> {
                    id.verify(<$id>::expected())
                        .map_err(|_: PersistentIdMismatch<$id>| ResolutionError)
                }
            }
        };
    }

    test_identity!(PersistentTypeId);
    test_identity!(PersistentGenericTypeId);
    test_identity!(PersistentExactTypeId);

    #[test]
    fn all_exact_type_shapes_round_trip_and_resolve() {
        let nominal = PersistentTypeId::expected();
        let generic = PersistentGenericTypeId::expected();
        let exact = PersistentExactTypeId::expected();
        let keys = [
            ExactTypeKey::Nominal(nominal),
            ExactTypeKey::NominalApplication {
                origin: generic,
                arguments: NonEmptyVec::from_first(exact, []),
            },
            ExactTypeKey::Tuple(NonEmptyVec::from_first(exact, [exact])),
            ExactTypeKey::Function {
                effect: Effect::Suspend,
                parameters: vec![exact],
                result: exact,
            },
            ExactTypeKey::RawPointer(exact),
            ExactTypeKey::NativeFunctionPointer {
                calling_convention: crate::CallingConvention::C,
                parameters: vec![exact],
                result: exact,
            },
        ];

        for key in keys {
            let decoded = decode_canonical::<DecodedExactTypeKey>(&encode(&key).unwrap()).unwrap();
            assert_eq!(decoded.resolve(&mut Resolver).unwrap(), key);
        }
    }

    #[test]
    fn exact_type_record_resolves_children_before_recomputing_identity() {
        let key = ExactTypeKey::Tuple(NonEmptyVec::from_first(
            PersistentExactTypeId::expected(),
            [],
        ));
        let record = CborIdentityRecord::<PersistentExactTypeId, _>::from_key(key).unwrap();
        let decoded = decode_canonical::<
            DecodedCborIdentityRecord<PersistentExactTypeId, DecodedExactTypeKey>,
        >(&encode(&record).unwrap())
        .unwrap();

        assert_eq!(
            decoded.resolve(|key| key.resolve(&mut Resolver)).unwrap(),
            record
        );
    }

    #[test]
    fn exact_type_decoder_rejects_empty_and_unknown_shapes() {
        let empty_tuple =
            decode_canonical::<DecodedExactTypeKey>(b"\xa2\x00\x03\x01\x80").unwrap_err();
        assert_eq!(
            empty_tuple.kind(),
            &WireErrorKind::InvalidLength {
                expected: 1,
                actual: 0,
            }
        );

        let unknown = decode_canonical::<DecodedExactTypeKey>(b"\xa1\x00\x07").unwrap_err();
        assert_eq!(unknown.kind(), &WireErrorKind::UnknownTag { tag: 7 });
    }

    #[test]
    fn exact_type_resolution_keeps_references_typed() {
        let key = ExactTypeKey::Nominal(PersistentTypeId::expected());
        let mut bytes = encode(&key).unwrap();
        *bytes.last_mut().unwrap() = 8;
        let decoded = decode_canonical::<DecodedExactTypeKey>(&bytes).unwrap();
        assert_eq!(
            decoded.resolve(&mut Resolver),
            Err(ExactTypeResolutionError::Reference(ResolutionError))
        );
    }
}
