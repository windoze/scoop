use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{
    CoroutineAdapterRole, GeneratedCallableIdentityError, GeneratedCallableKey,
    InitializationCallableRole, LexicalCallableParent, LexicalCallableRole, LexicalParentError,
};
use crate::{
    DecodedCallableMaterialization, DecodedExactCallableSignature, DecodedPersistentId,
    ExactCallableSignatureResolutionError, PersistentCallableApplicationId,
    PersistentCallbackApplicationId, PersistentConstructorId, PersistentDispatchSlotId,
    PersistentEnumVariantId, PersistentExactTypeId, PersistentFunctionId,
    PersistentGeneratedCallableId, PersistentGenericFunctionId, PersistentIdResolver,
    PersistentInitializationUnitId, PersistentKeyResolver, PersistentPropertyAccessorId,
    PersistentTypeId, StructuralDefinitionPath,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedLexicalCallableParent {
    Function(DecodedPersistentId<PersistentFunctionId>),
    GenericFunction(DecodedPersistentId<PersistentGenericFunctionId>),
    Constructor(DecodedPersistentId<PersistentConstructorId>),
    Accessor(DecodedPersistentId<PersistentPropertyAccessorId>),
    Generated(DecodedPersistentId<PersistentGeneratedCallableId>),
    VariantConstructor(DecodedPersistentId<PersistentEnumVariantId>),
}

impl DecodedLexicalCallableParent {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<LexicalCallableParent, GeneratedCallableResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentFunctionId, Error = E>
            + PersistentIdResolver<PersistentGenericFunctionId, Error = E>
            + PersistentIdResolver<PersistentConstructorId, Error = E>
            + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
            + PersistentIdResolver<PersistentEnumVariantId, Error = E>
            + PersistentKeyResolver<PersistentGeneratedCallableId, GeneratedCallableKey, Error = E>,
    {
        match self {
            Self::Function(id) => resolver
                .resolve(id)
                .map(LexicalCallableParent::function)
                .map_err(GeneratedCallableResolutionError::Reference),
            Self::GenericFunction(id) => resolver
                .resolve(id)
                .map(LexicalCallableParent::generic_function)
                .map_err(GeneratedCallableResolutionError::Reference),
            Self::Constructor(id) => resolver
                .resolve(id)
                .map(LexicalCallableParent::constructor)
                .map_err(GeneratedCallableResolutionError::Reference),
            Self::Accessor(id) => resolver
                .resolve(id)
                .map(LexicalCallableParent::accessor)
                .map_err(GeneratedCallableResolutionError::Reference),
            Self::Generated(id) => {
                let key = resolver
                    .resolve_key(id)
                    .map_err(GeneratedCallableResolutionError::Reference)?;
                LexicalCallableParent::from_generated_key(&key)
                    .map_err(GeneratedCallableResolutionError::Parent)
            }
            Self::VariantConstructor(id) => resolver
                .resolve(id)
                .map(LexicalCallableParent::variant_constructor)
                .map_err(GeneratedCallableResolutionError::Reference),
        }
    }
}

impl WireEncode for DecodedLexicalCallableParent {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Function(id) => encode_value_sum(encoder, 1, id),
            Self::GenericFunction(id) => encode_value_sum(encoder, 2, id),
            Self::Constructor(id) => encode_value_sum(encoder, 3, id),
            Self::Accessor(id) => encode_value_sum(encoder, 4, id),
            Self::Generated(id) => encode_value_sum(encoder, 5, id),
            Self::VariantConstructor(id) => encode_value_sum(encoder, 6, id),
        }
    }
}

impl WireDecode for DecodedLexicalCallableParent {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        expect_sum_length(decoder, fields, 2)?;
        match tag {
            1 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Function),
            2 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::GenericFunction),
            3 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Constructor),
            4 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Accessor),
            5 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Generated),
            6 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::VariantConstructor),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedGeneratedCallableKey {
    Lexical {
        parent: DecodedLexicalCallableParent,
        role: LexicalCallableRole,
        path: StructuralDefinitionPath,
    },
    Initialization {
        unit: DecodedPersistentId<PersistentInitializationUnitId>,
        role: InitializationCallableRole,
    },
    DerivedEquality {
        exact_owner: DecodedPersistentId<PersistentExactTypeId>,
    },
    FunctionAdapter {
        source: DecodedExactCallableSignature,
        target: DecodedExactCallableSignature,
    },
    DynamicFunctionAdapter {
        target: DecodedExactCallableSignature,
    },
    CallableReferenceInvoke {
        parent: DecodedLexicalCallableParent,
        path: StructuralDefinitionPath,
    },
    StaticNoGcCallbackStorageBridge {
        source: DecodedCallableMaterialization,
        signature: DecodedExactCallableSignature,
    },
    ForeignCallbackManagedAdapter {
        application: DecodedPersistentId<PersistentCallbackApplicationId>,
    },
    CoroutineDriver {
        source_callable: DecodedCallableMaterialization,
    },
    CoroutineStart {
        result: DecodedPersistentId<PersistentExactTypeId>,
    },
    CoroutineAdapter {
        source_callable: DecodedCallableMaterialization,
        suspension_site: StructuralDefinitionPath,
        role: CoroutineAdapterRole,
    },
    FunctionBridge {
        environment: DecodedPersistentId<PersistentTypeId>,
        target: DecodedExactCallableSignature,
    },
    DispatchAdjust {
        slot: DecodedPersistentId<PersistentDispatchSlotId>,
        implementor: DecodedPersistentId<PersistentExactTypeId>,
        target: DecodedCallableMaterialization,
    },
    BoxingAdjust {
        slot: DecodedPersistentId<PersistentDispatchSlotId>,
        payload: DecodedPersistentId<PersistentExactTypeId>,
        interface: DecodedPersistentId<PersistentExactTypeId>,
    },
    ZeroArgumentConstructorAdapter {
        constructor: DecodedPersistentId<PersistentConstructorId>,
    },
}

impl DecodedGeneratedCallableKey {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<GeneratedCallableKey, GeneratedCallableResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentFunctionId, Error = E>
            + PersistentIdResolver<PersistentGenericFunctionId, Error = E>
            + PersistentIdResolver<PersistentConstructorId, Error = E>
            + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
            + PersistentIdResolver<PersistentEnumVariantId, Error = E>
            + PersistentKeyResolver<PersistentGeneratedCallableId, GeneratedCallableKey, Error = E>
            + PersistentIdResolver<PersistentInitializationUnitId, Error = E>
            + PersistentIdResolver<PersistentExactTypeId, Error = E>
            + PersistentIdResolver<PersistentGeneratedCallableId, Error = E>
            + PersistentIdResolver<PersistentCallableApplicationId, Error = E>
            + PersistentIdResolver<PersistentCallbackApplicationId, Error = E>
            + PersistentIdResolver<PersistentTypeId, Error = E>
            + PersistentIdResolver<PersistentDispatchSlotId, Error = E>,
    {
        let key = match self {
            Self::Lexical { parent, role, path } => GeneratedCallableKey::Lexical {
                parent: parent.resolve(resolver)?,
                role,
                path,
            },
            Self::Initialization { unit, role } => GeneratedCallableKey::Initialization {
                unit: resolve_id(resolver, unit)?,
                role,
            },
            Self::DerivedEquality { exact_owner } => GeneratedCallableKey::DerivedEquality {
                exact_owner: resolve_id(resolver, exact_owner)?,
            },
            Self::FunctionAdapter { source, target } => GeneratedCallableKey::FunctionAdapter {
                source: resolve_signature(source, resolver)?,
                target: resolve_signature(target, resolver)?,
            },
            Self::DynamicFunctionAdapter { target } => {
                GeneratedCallableKey::DynamicFunctionAdapter {
                    target: resolve_signature(target, resolver)?,
                }
            }
            Self::CallableReferenceInvoke { parent, path } => {
                GeneratedCallableKey::CallableReferenceInvoke {
                    parent: parent.resolve(resolver)?,
                    path,
                }
            }
            Self::StaticNoGcCallbackStorageBridge { source, signature } => {
                GeneratedCallableKey::StaticNoGcCallbackStorageBridge {
                    source: source
                        .resolve(resolver)
                        .map_err(GeneratedCallableResolutionError::Reference)?,
                    signature: resolve_signature(signature, resolver)?,
                }
            }
            Self::ForeignCallbackManagedAdapter { application } => {
                GeneratedCallableKey::ForeignCallbackManagedAdapter {
                    application: resolve_id(resolver, application)?,
                }
            }
            Self::CoroutineDriver { source_callable } => GeneratedCallableKey::CoroutineDriver {
                source_callable: source_callable
                    .resolve(resolver)
                    .map_err(GeneratedCallableResolutionError::Reference)?,
            },
            Self::CoroutineStart { result } => GeneratedCallableKey::CoroutineStart {
                result: resolve_id(resolver, result)?,
            },
            Self::CoroutineAdapter {
                source_callable,
                suspension_site,
                role,
            } => GeneratedCallableKey::CoroutineAdapter {
                source_callable: source_callable
                    .resolve(resolver)
                    .map_err(GeneratedCallableResolutionError::Reference)?,
                suspension_site,
                role,
            },
            Self::FunctionBridge {
                environment,
                target,
            } => GeneratedCallableKey::FunctionBridge {
                environment: resolve_id(resolver, environment)?,
                target: resolve_signature(target, resolver)?,
            },
            Self::DispatchAdjust {
                slot,
                implementor,
                target,
            } => GeneratedCallableKey::DispatchAdjust {
                slot: resolve_id(resolver, slot)?,
                implementor: resolve_id(resolver, implementor)?,
                target: target
                    .resolve(resolver)
                    .map_err(GeneratedCallableResolutionError::Reference)?,
            },
            Self::BoxingAdjust {
                slot,
                payload,
                interface,
            } => GeneratedCallableKey::BoxingAdjust {
                slot: resolve_id(resolver, slot)?,
                payload: resolve_id(resolver, payload)?,
                interface: resolve_id(resolver, interface)?,
            },
            Self::ZeroArgumentConstructorAdapter { constructor } => {
                GeneratedCallableKey::ZeroArgumentConstructorAdapter {
                    constructor: resolve_id(resolver, constructor)?,
                }
            }
        };
        key.validate()
            .map_err(GeneratedCallableResolutionError::Key)?;
        Ok(key)
    }
}

impl WireEncode for DecodedGeneratedCallableKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Lexical { parent, role, path } => {
                encoder.map(4)?;
                encode_tag(encoder, 1)?;
                encoder.field(1)?;
                parent.encode(encoder)?;
                encoder.field(2)?;
                role.encode(encoder)?;
                encoder.field(3)?;
                path.encode(encoder)
            }
            Self::Initialization { unit, role } => encode_two_value_sum(encoder, 2, unit, role),
            Self::DerivedEquality { exact_owner } => encode_value_sum(encoder, 3, exact_owner),
            Self::FunctionAdapter { source, target } => {
                encode_two_value_sum(encoder, 4, source, target)
            }
            Self::DynamicFunctionAdapter { target } => encode_value_sum(encoder, 5, target),
            Self::CallableReferenceInvoke { parent, path } => {
                encode_two_value_sum(encoder, 6, parent, path)
            }
            Self::StaticNoGcCallbackStorageBridge { source, signature } => {
                encode_two_value_sum(encoder, 7, source, signature)
            }
            Self::ForeignCallbackManagedAdapter { application } => {
                encode_value_sum(encoder, 8, application)
            }
            Self::CoroutineDriver { source_callable } => {
                encode_value_sum(encoder, 9, source_callable)
            }
            Self::CoroutineStart { result } => encode_value_sum(encoder, 11, result),
            Self::CoroutineAdapter {
                source_callable,
                suspension_site,
                role,
            } => encode_three_value_sum(encoder, 12, source_callable, suspension_site, role),
            Self::FunctionBridge {
                environment,
                target,
            } => encode_two_value_sum(encoder, 13, environment, target),
            Self::DispatchAdjust {
                slot,
                implementor,
                target,
            } => encode_three_value_sum(encoder, 14, slot, implementor, target),
            Self::BoxingAdjust {
                slot,
                payload,
                interface,
            } => encode_three_value_sum(encoder, 15, slot, payload, interface),
            Self::ZeroArgumentConstructorAdapter { constructor } => {
                encode_value_sum(encoder, 16, constructor)
            }
        }
    }
}

impl WireDecode for DecodedGeneratedCallableKey {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 4)?;
                Ok(Self::Lexical {
                    parent: decoder.field(1, DecodedLexicalCallableParent::decode)?,
                    role: decoder.field(2, LexicalCallableRole::decode)?,
                    path: decoder.field(3, StructuralDefinitionPath::decode)?,
                })
            }
            2 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::Initialization {
                    unit: decoder.field(1, DecodedPersistentId::decode)?,
                    role: decoder.field(2, InitializationCallableRole::decode)?,
                })
            }
            3 => decode_id_variant(decoder, fields, |exact_owner| Self::DerivedEquality {
                exact_owner,
            }),
            4 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::FunctionAdapter {
                    source: decoder.field(1, DecodedExactCallableSignature::decode)?,
                    target: decoder.field(2, DecodedExactCallableSignature::decode)?,
                })
            }
            5 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedExactCallableSignature::decode)
                    .map(|target| Self::DynamicFunctionAdapter { target })
            }
            6 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::CallableReferenceInvoke {
                    parent: decoder.field(1, DecodedLexicalCallableParent::decode)?,
                    path: decoder.field(2, StructuralDefinitionPath::decode)?,
                })
            }
            7 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::StaticNoGcCallbackStorageBridge {
                    source: decoder.field(1, DecodedCallableMaterialization::decode)?,
                    signature: decoder.field(2, DecodedExactCallableSignature::decode)?,
                })
            }
            8 => decode_id_variant(decoder, fields, |application| {
                Self::ForeignCallbackManagedAdapter { application }
            }),
            9 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedCallableMaterialization::decode)
                    .map(|source_callable| Self::CoroutineDriver { source_callable })
            }
            // Tag 10 (ContinuationShell) is retired.
            11 => decode_id_variant(decoder, fields, |result| Self::CoroutineStart { result }),
            12 => {
                expect_sum_length(decoder, fields, 4)?;
                Ok(Self::CoroutineAdapter {
                    source_callable: decoder.field(1, DecodedCallableMaterialization::decode)?,
                    suspension_site: decoder.field(2, StructuralDefinitionPath::decode)?,
                    role: decoder.field(3, CoroutineAdapterRole::decode)?,
                })
            }
            13 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::FunctionBridge {
                    environment: decoder.field(1, DecodedPersistentId::decode)?,
                    target: decoder.field(2, DecodedExactCallableSignature::decode)?,
                })
            }
            14 => {
                expect_sum_length(decoder, fields, 4)?;
                Ok(Self::DispatchAdjust {
                    slot: decoder.field(1, DecodedPersistentId::decode)?,
                    implementor: decoder.field(2, DecodedPersistentId::decode)?,
                    target: decoder.field(3, DecodedCallableMaterialization::decode)?,
                })
            }
            15 => {
                expect_sum_length(decoder, fields, 4)?;
                Ok(Self::BoxingAdjust {
                    slot: decoder.field(1, DecodedPersistentId::decode)?,
                    payload: decoder.field(2, DecodedPersistentId::decode)?,
                    interface: decoder.field(3, DecodedPersistentId::decode)?,
                })
            }
            16 => decode_id_variant(decoder, fields, |constructor| {
                Self::ZeroArgumentConstructorAdapter { constructor }
            }),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

impl WireDecode for LexicalCallableRole {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decode_binary_role(decoder, Self::LambdaBody, Self::AnonymousFunctionBody)
    }
}

impl WireDecode for InitializationCallableRole {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decode_binary_role(decoder, Self::Initializer, Self::Ensure)
    }
}

impl WireDecode for CoroutineAdapterRole {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decode_binary_role(decoder, Self::Success, Self::Failure)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GeneratedCallableResolutionError<E> {
    Reference(E),
    Parent(LexicalParentError),
    Signature(ExactCallableSignatureResolutionError<E>),
    Key(GeneratedCallableIdentityError),
}

impl<E: fmt::Display> fmt::Display for GeneratedCallableResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(error) => error.fmt(formatter),
            Self::Parent(error) => error.fmt(formatter),
            Self::Signature(error) => error.fmt(formatter),
            Self::Key(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for GeneratedCallableResolutionError<E> {}

fn resolve_id<R, I, E>(
    resolver: &mut R,
    id: DecodedPersistentId<I>,
) -> Result<I, GeneratedCallableResolutionError<E>>
where
    I: crate::PersistentId,
    R: PersistentIdResolver<I, Error = E>,
{
    resolver
        .resolve(id)
        .map_err(GeneratedCallableResolutionError::Reference)
}

fn resolve_signature<R, E>(
    signature: DecodedExactCallableSignature,
    resolver: &mut R,
) -> Result<crate::ExactCallableSignature, GeneratedCallableResolutionError<E>>
where
    R: PersistentIdResolver<PersistentExactTypeId, Error = E>,
{
    signature
        .resolve(resolver)
        .map_err(GeneratedCallableResolutionError::Signature)
}

fn decode_binary_role<T>(decoder: &mut Decoder<'_>, first: T, second: T) -> Result<T, WireError> {
    match decoder.unsigned()? {
        1 => Ok(first),
        2 => Ok(second),
        tag => Err(unknown_tag(decoder, tag)),
    }
}

fn decode_sum_header(decoder: &mut Decoder<'_>) -> Result<(u64, u64), WireError> {
    let fields = decoder.map()?;
    let tag = decoder.field(0, Decoder::unsigned)?;
    Ok((fields, tag))
}

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
        Err(WireError::new(
            WireErrorKind::InvalidLength { expected, actual },
            decoder.path().clone(),
            Some(decoder.position()),
        ))
    }
}

fn unknown_tag(decoder: &Decoder<'_>, tag: u64) -> WireError {
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

fn encode_three_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    first: &impl WireEncode,
    second: &impl WireEncode,
    third: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(4)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    first.encode(encoder)?;
    encoder.field(2)?;
    second.encode(encoder)?;
    encoder.field(3)?;
    third.encode(encoder)
}

#[cfg(test)]
mod tests;
