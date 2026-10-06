use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{
    CallableAdapterEnvironmentKey, ClosureEnvironmentRole, GeneratedNominalIdentityError,
    GeneratedNominalKey, validate_key,
};
use crate::{
    DecodedCallableMaterialization, DecodedExactCallableSignature, DecodedPersistentId,
    ExactCallableSignatureResolutionError, PersistentCallableApplicationId,
    PersistentConstructorId, PersistentExactTypeId, PersistentFunctionId,
    PersistentGeneratedCallableId, PersistentGenericFunctionId, PersistentIdResolver,
    PersistentInitializationUnitId, PersistentPropertyAccessorId, PersistentTypeId,
    StructuralDefinitionPath,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedCallableAdapterEnvironmentKey {
    Static {
        source: DecodedExactCallableSignature,
        target: DecodedExactCallableSignature,
    },
    Dynamic {
        target: DecodedExactCallableSignature,
    },
}

impl DecodedCallableAdapterEnvironmentKey {
    fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CallableAdapterEnvironmentKey, ExactCallableSignatureResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentExactTypeId, Error = E>,
    {
        match self {
            Self::Static { source, target } => Ok(CallableAdapterEnvironmentKey::Static {
                source: source.resolve(resolver)?,
                target: target.resolve(resolver)?,
            }),
            Self::Dynamic { target } => Ok(CallableAdapterEnvironmentKey::Dynamic {
                target: target.resolve(resolver)?,
            }),
        }
    }
}

impl WireEncode for DecodedCallableAdapterEnvironmentKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Static { source, target } => {
                encoder.map(3)?;
                encode_tag(encoder, 1)?;
                encoder.field(1)?;
                source.encode(encoder)?;
                encoder.field(2)?;
                target.encode(encoder)
            }
            Self::Dynamic { target } => encode_value_sum(encoder, 2, target),
        }
    }
}

impl WireDecode for DecodedCallableAdapterEnvironmentKey {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::Static {
                    source: decoder.field(1, DecodedExactCallableSignature::decode)?,
                    target: decoder.field(2, DecodedExactCallableSignature::decode)?,
                })
            }
            2 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedExactCallableSignature::decode)
                    .map(|target| Self::Dynamic { target })
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedGeneratedNominalKey {
    TaskContext(crate::DecodedContextStorageType),
    ClosureEnvironment {
        callable: DecodedCallableMaterialization,
        role: ClosureEnvironmentRole,
    },
    CallableAdapterEnvironment {
        key: DecodedCallableAdapterEnvironmentKey,
    },
    CoroutineFrame {
        source_callable: DecodedCallableMaterialization,
    },
    ContinuationAdapterEnvironment {
        source_callable: DecodedCallableMaterialization,
        suspension_site: StructuralDefinitionPath,
    },
    CoroutineStep {
        result: DecodedPersistentId<PersistentExactTypeId>,
    },
    BoxedValue {
        payload: DecodedPersistentId<PersistentExactTypeId>,
    },
    CoroutineSlot {
        value: DecodedPersistentId<PersistentExactTypeId>,
    },
    ObjectBackingClass {
        object: DecodedPersistentId<PersistentTypeId>,
    },
    GenericObjectBackingClass {
        object: DecodedPersistentId<crate::PersistentGenericTypeId>,
    },
}

impl DecodedGeneratedNominalKey {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<GeneratedNominalKey, GeneratedNominalResolutionError<E>>
    where
        R: PersistentIdResolver<crate::ConeIdentity, Error = E>
            + PersistentIdResolver<PersistentFunctionId, Error = E>
            + PersistentIdResolver<PersistentGenericFunctionId, Error = E>
            + PersistentIdResolver<PersistentConstructorId, Error = E>
            + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
            + PersistentIdResolver<crate::PersistentEnumVariantId, Error = E>
            + PersistentIdResolver<PersistentGeneratedCallableId, Error = E>
            + PersistentIdResolver<PersistentCallableApplicationId, Error = E>
            + PersistentIdResolver<PersistentInitializationUnitId, Error = E>
            + PersistentIdResolver<PersistentExactTypeId, Error = E>
            + PersistentIdResolver<PersistentTypeId, Error = E>
            + PersistentIdResolver<crate::PersistentGenericTypeId, Error = E>,
    {
        let key = match self {
            Self::TaskContext(storage) => GeneratedNominalKey::TaskContext(
                storage
                    .resolve(resolver)
                    .map_err(GeneratedNominalResolutionError::Reference)?,
            ),
            Self::ClosureEnvironment { callable, role } => {
                GeneratedNominalKey::ClosureEnvironment {
                    callable: callable
                        .resolve(resolver)
                        .map_err(GeneratedNominalResolutionError::Reference)?,
                    role,
                }
            }
            Self::CallableAdapterEnvironment { key } => {
                GeneratedNominalKey::CallableAdapterEnvironment {
                    key: key
                        .resolve(resolver)
                        .map_err(GeneratedNominalResolutionError::Signature)?,
                }
            }
            Self::CoroutineFrame { source_callable } => GeneratedNominalKey::CoroutineFrame {
                source_callable: source_callable
                    .resolve(resolver)
                    .map_err(GeneratedNominalResolutionError::Reference)?,
            },
            Self::ContinuationAdapterEnvironment {
                source_callable,
                suspension_site,
            } => GeneratedNominalKey::ContinuationAdapterEnvironment {
                source_callable: source_callable
                    .resolve(resolver)
                    .map_err(GeneratedNominalResolutionError::Reference)?,
                suspension_site,
            },
            Self::CoroutineStep { result } => GeneratedNominalKey::CoroutineStep {
                result: resolver
                    .resolve(result)
                    .map_err(GeneratedNominalResolutionError::Reference)?,
            },
            Self::BoxedValue { payload } => GeneratedNominalKey::BoxedValue {
                payload: resolver
                    .resolve(payload)
                    .map_err(GeneratedNominalResolutionError::Reference)?,
            },
            Self::CoroutineSlot { value } => GeneratedNominalKey::CoroutineSlot {
                value: resolver
                    .resolve(value)
                    .map_err(GeneratedNominalResolutionError::Reference)?,
            },
            Self::GenericObjectBackingClass { object } => {
                GeneratedNominalKey::GenericObjectBackingClass {
                    object: resolver
                        .resolve(object)
                        .map_err(GeneratedNominalResolutionError::Reference)?,
                }
            }
            Self::ObjectBackingClass { object } => GeneratedNominalKey::ObjectBackingClass {
                object: resolver
                    .resolve(object)
                    .map_err(GeneratedNominalResolutionError::Reference)?,
            },
        };
        validate_key(&key).map_err(GeneratedNominalResolutionError::Key)?;
        Ok(key)
    }
}

impl WireEncode for DecodedGeneratedNominalKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::TaskContext(storage) => encode_value_sum(encoder, 9, storage),
            Self::ClosureEnvironment { callable, role } => {
                encoder.map(3)?;
                encode_tag(encoder, 1)?;
                encoder.field(1)?;
                callable.encode(encoder)?;
                encoder.field(2)?;
                role.encode(encoder)
            }
            Self::CallableAdapterEnvironment { key } => encode_value_sum(encoder, 2, key),
            Self::CoroutineFrame { source_callable } => {
                encode_value_sum(encoder, 3, source_callable)
            }
            Self::ContinuationAdapterEnvironment {
                source_callable,
                suspension_site,
            } => {
                encoder.map(3)?;
                encode_tag(encoder, 4)?;
                encoder.field(1)?;
                source_callable.encode(encoder)?;
                encoder.field(2)?;
                suspension_site.encode(encoder)
            }
            Self::CoroutineStep { result } => encode_value_sum(encoder, 5, result),
            Self::BoxedValue { payload } => encode_value_sum(encoder, 6, payload),
            Self::CoroutineSlot { value } => encode_value_sum(encoder, 7, value),
            Self::ObjectBackingClass { object } => encode_value_sum(encoder, 8, object),
            Self::GenericObjectBackingClass { object } => encode_value_sum(encoder, 10, object),
        }
    }
}

impl WireDecode for DecodedGeneratedNominalKey {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::ClosureEnvironment {
                    callable: decoder.field(1, DecodedCallableMaterialization::decode)?,
                    role: decoder.field(2, ClosureEnvironmentRole::decode)?,
                })
            }
            2 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedCallableAdapterEnvironmentKey::decode)
                    .map(|key| Self::CallableAdapterEnvironment { key })
            }
            3 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedCallableMaterialization::decode)
                    .map(|source_callable| Self::CoroutineFrame { source_callable })
            }
            4 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::ContinuationAdapterEnvironment {
                    source_callable: decoder.field(1, DecodedCallableMaterialization::decode)?,
                    suspension_site: decoder.field(2, StructuralDefinitionPath::decode)?,
                })
            }
            5 => decode_id_variant(decoder, fields, |result| Self::CoroutineStep { result }),
            6 => decode_id_variant(decoder, fields, |payload| Self::BoxedValue { payload }),
            7 => decode_id_variant(decoder, fields, |value| Self::CoroutineSlot { value }),
            8 => decode_id_variant(decoder, fields, |object| Self::ObjectBackingClass {
                object,
            }),
            10 => decode_id_variant(decoder, fields, |object| Self::GenericObjectBackingClass {
                object,
            }),
            9 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, crate::DecodedContextStorageType::decode)
                    .map(Self::TaskContext)
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

impl WireDecode for ClosureEnvironmentRole {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::Lambda),
            2 => Ok(Self::AnonymousFunction),
            3 => Ok(Self::CallableReference),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GeneratedNominalResolutionError<E> {
    Reference(E),
    Signature(ExactCallableSignatureResolutionError<E>),
    Key(GeneratedNominalIdentityError),
}

impl<E: fmt::Display> fmt::Display for GeneratedNominalResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(error) => error.fmt(formatter),
            Self::Signature(error) => error.fmt(formatter),
            Self::Key(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for GeneratedNominalResolutionError<E> {}

fn decode_id_variant<I, T>(
    decoder: &mut Decoder<'_>,
    fields: u64,
    build: impl FnOnce(DecodedPersistentId<I>) -> T,
) -> Result<T, WireError>
where
    I: crate::PersistentId,
{
    expect_sum_length(decoder, fields, 2)?;
    decoder.field(1, DecodedPersistentId::decode).map(build)
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

#[cfg(test)]
mod tests;
