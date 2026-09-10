use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{
    CallbackApplicationIdentityError, CallbackApplicationKey, CallbackParameterIndex,
    CallbackRegistrationKey, SignatureCallableShape,
};
use crate::{
    DecodedCallableMaterializationContext, DecodedLexicalCallableParent,
    DecodedOptionalSignatureType, DecodedPersistentId, DecodedSignatureTypeKey,
    DecodedSourceCAbiFunctionSignature, Effect, GeneratedCallableKey,
    GeneratedCallableResolutionError, PersistentCallableApplicationId,
    PersistentCallbackRegistrationId, PersistentConstructorId, PersistentFunctionId,
    PersistentGeneratedCallableId, PersistentGenericFunctionId, PersistentGenericTypeId,
    PersistentIdResolver, PersistentInitializationUnitId, PersistentKeyResolver,
    PersistentPropertyAccessorId, PersistentTypeId, SignatureTypeKey, StructuralDefinitionPath,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedSignatureCallableShape {
    effect: Effect,
    receiver: DecodedOptionalSignatureType,
    parameters: Vec<DecodedSignatureTypeKey>,
    result: DecodedSignatureTypeKey,
}

impl DecodedSignatureCallableShape {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<SignatureCallableShape, E>
    where
        R: PersistentIdResolver<PersistentTypeId, Error = E>
            + PersistentIdResolver<PersistentGenericTypeId, Error = E>,
    {
        let receiver = match self.receiver {
            DecodedOptionalSignatureType::Absent => None,
            DecodedOptionalSignatureType::Present(receiver) => Some(receiver.resolve(resolver)?),
        };
        let parameters = resolve_signatures(self.parameters, resolver)?;
        let result = self.result.resolve(resolver)?;
        Ok(SignatureCallableShape::new(
            self.effect,
            receiver,
            parameters,
            result,
        ))
    }
}

impl WireEncode for DecodedSignatureCallableShape {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.effect.encode(encoder)?;
        encoder.field(2)?;
        self.receiver.encode(encoder)?;
        encoder.field(3)?;
        encode_signatures(encoder, &self.parameters)?;
        encoder.field(4)?;
        self.result.encode(encoder)
    }
}

impl WireDecode for DecodedSignatureCallableShape {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            effect: decoder.field(1, decode_effect)?,
            receiver: decoder.field(2, DecodedOptionalSignatureType::decode)?,
            parameters: decoder.field(3, decode_signatures)?,
            result: decoder.field(4, DecodedSignatureTypeKey::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCallbackRegistrationKey {
    parent: DecodedLexicalCallableParent,
    path: StructuralDefinitionPath,
    source_signature: DecodedSourceCAbiFunctionSignature,
    context_index: CallbackParameterIndex,
    managed_signature: DecodedSignatureCallableShape,
    mode: crate::CallbackMode,
}

impl DecodedCallbackRegistrationKey {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CallbackRegistrationKey, CallbackIdentityResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentFunctionId, Error = E>
            + PersistentIdResolver<PersistentGenericFunctionId, Error = E>
            + PersistentIdResolver<PersistentConstructorId, Error = E>
            + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
            + PersistentIdResolver<crate::PersistentEnumVariantId, Error = E>
            + PersistentKeyResolver<PersistentGeneratedCallableId, GeneratedCallableKey, Error = E>
            + PersistentIdResolver<PersistentTypeId, Error = E>
            + PersistentIdResolver<PersistentGenericTypeId, Error = E>,
    {
        let parent = self
            .parent
            .resolve(resolver)
            .map_err(CallbackIdentityResolutionError::Parent)?;
        let source_signature = self
            .source_signature
            .resolve(resolver)
            .map_err(CallbackIdentityResolutionError::Reference)?;
        let managed_signature = self
            .managed_signature
            .resolve(resolver)
            .map_err(CallbackIdentityResolutionError::Reference)?;
        Ok(CallbackRegistrationKey::new(
            parent,
            self.path,
            source_signature,
            self.context_index,
            managed_signature,
            self.mode,
        ))
    }
}

impl WireEncode for DecodedCallbackRegistrationKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.parent.encode(encoder)?;
        encoder.field(2)?;
        self.path.encode(encoder)?;
        encoder.field(3)?;
        self.source_signature.encode(encoder)?;
        encoder.field(4)?;
        self.context_index.encode(encoder)?;
        encoder.field(5)?;
        self.managed_signature.encode(encoder)?;
        encoder.field(6)?;
        self.mode.encode(encoder)
    }
}

impl WireDecode for DecodedCallbackRegistrationKey {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(6)?;
        Ok(Self {
            parent: decoder.field(1, DecodedLexicalCallableParent::decode)?,
            path: decoder.field(2, StructuralDefinitionPath::decode)?,
            source_signature: decoder.field(3, DecodedSourceCAbiFunctionSignature::decode)?,
            context_index: decoder.field(4, CallbackParameterIndex::decode)?,
            managed_signature: decoder.field(5, DecodedSignatureCallableShape::decode)?,
            mode: decoder.field(6, crate::CallbackMode::decode)?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedCallbackApplicationKey {
    registration: DecodedPersistentId<PersistentCallbackRegistrationId>,
    context: DecodedCallableMaterializationContext,
}

impl DecodedCallbackApplicationKey {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CallbackApplicationKey, CallbackIdentityResolutionError<E>>
    where
        R: PersistentKeyResolver<
                PersistentCallbackRegistrationId,
                CallbackRegistrationKey,
                Error = E,
            > + PersistentIdResolver<PersistentCallableApplicationId, Error = E>
            + PersistentIdResolver<PersistentInitializationUnitId, Error = E>,
    {
        let registration = resolver
            .resolve_key(self.registration)
            .map_err(CallbackIdentityResolutionError::Reference)?;
        let context = self
            .context
            .resolve(resolver)
            .map_err(CallbackIdentityResolutionError::Reference)?;
        CallbackApplicationKey::new(&registration, context)
            .map_err(CallbackIdentityResolutionError::Application)
    }
}

impl WireEncode for DecodedCallbackApplicationKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.registration.encode(encoder)?;
        encoder.field(2)?;
        self.context.encode(encoder)
    }
}

impl WireDecode for DecodedCallbackApplicationKey {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            registration: decoder.field(1, DecodedPersistentId::decode)?,
            context: decoder.field(2, DecodedCallableMaterializationContext::decode)?,
        })
    }
}

impl WireDecode for CallbackParameterIndex {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.u32().map(Self::new)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallbackIdentityResolutionError<E> {
    Reference(E),
    Parent(GeneratedCallableResolutionError<E>),
    Application(CallbackApplicationIdentityError),
}

impl<E: fmt::Display> fmt::Display for CallbackIdentityResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(error) => error.fmt(formatter),
            Self::Parent(error) => error.fmt(formatter),
            Self::Application(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for CallbackIdentityResolutionError<E> {}

fn resolve_signatures<R, E>(
    values: Vec<DecodedSignatureTypeKey>,
    resolver: &mut R,
) -> Result<Vec<SignatureTypeKey>, E>
where
    R: PersistentIdResolver<PersistentTypeId, Error = E>
        + PersistentIdResolver<PersistentGenericTypeId, Error = E>,
{
    values
        .into_iter()
        .map(|value| value.resolve(resolver))
        .collect()
}

fn decode_signatures(
    decoder: &mut Decoder<'_, '_>,
) -> Result<Vec<DecodedSignatureTypeKey>, WireError> {
    decoder.decode_array(|decoder, _| DecodedSignatureTypeKey::decode(decoder))
}

fn encode_signatures(
    encoder: &mut Encoder,
    values: &[DecodedSignatureTypeKey],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

fn decode_effect(decoder: &mut Decoder<'_, '_>) -> Result<Effect, WireError> {
    match decoder.unsigned()? {
        1 => Ok(Effect::Ordinary),
        2 => Ok(Effect::Suspend),
        tag => Err(WireError::new(
            WireErrorKind::UnknownTag { tag },
            decoder.path().clone(),
            Some(decoder.position()),
        )),
    }
}

#[cfg(test)]
mod tests;
