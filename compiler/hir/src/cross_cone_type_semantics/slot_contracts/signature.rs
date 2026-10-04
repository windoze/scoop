use std::fmt;

use scoop_identity::{
    ContextKey, DecodedExactCallableSignature, DecodedPersistentId, ExactCallableSignature,
    ExactCallableSignatureResolutionError, PersistentExactTypeId, PersistentIdResolver,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use crate::{
    CallableImplementationV1, CallableSourceEffectsBuildError, CallableSourceEffectsV1,
    DecodedCallableSourceEffectsV1,
};

/// Complete exact call shape and source effects; receiver identity is retained
/// separately for the root declaration and each implementation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InheritanceCallableSignatureV1 {
    exact_signature: ExactCallableSignature,
    effects: CallableSourceEffectsV1,
    context_keys: Vec<ContextKey>,
}
impl InheritanceCallableSignatureV1 {
    pub fn try_new(
        exact_signature: ExactCallableSignature,
        effects: CallableSourceEffectsV1,
        context_keys: Vec<ContextKey>,
    ) -> Result<Self, InheritanceCallableSignatureBuildError> {
        if exact_signature.effect() != effects.execution() {
            return Err(InheritanceCallableSignatureBuildError::Execution);
        }
        if !exact_signature.receiver().is_present() {
            return Err(InheritanceCallableSignatureBuildError::MissingReceiver);
        }
        if matches!(
            effects.implementation(),
            CallableImplementationV1::SourceExternScoop | CallableImplementationV1::SourceExternC
        ) {
            return Err(InheritanceCallableSignatureBuildError::SourceExtern);
        }
        Ok(Self {
            exact_signature,
            effects,
            context_keys,
        })
    }
    pub const fn exact_signature(&self) -> &ExactCallableSignature {
        &self.exact_signature
    }
    pub fn context_keys(&self) -> &[ContextKey] {
        &self.context_keys
    }

    pub fn effects(&self) -> CallableSourceEffectsV1 {
        self.effects.clone()
    }

    pub fn receiver(&self) -> PersistentExactTypeId {
        self.exact_signature
            .receiver()
            .into_option()
            .expect("inheritance signatures are constructed with a receiver")
    }

    pub fn matches_slot(&self, slot: &Self) -> bool {
        self.exact_signature.parameters() == slot.exact_signature.parameters()
            && self.exact_signature.result() == slot.exact_signature.result()
            && self.effects.execution() == slot.effects.execution()
            && self.effects.safety() == slot.effects.safety()
            && self.effects.gc_effect() == slot.effects.gc_effect()
            && self.effects.operator_role() == slot.effects.operator_role()
            && self.effects.infix() == slot.effects.infix()
            && self.context_keys == slot.context_keys
    }
}
impl WireEncode for InheritanceCallableSignatureV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.exact_signature.encode(encoder)?;
        encoder.field(2)?;
        self.effects.encode(encoder)?;
        encoder.field(3)?;
        encoder.array(self.context_keys.len() as u64)?;
        for key in &self.context_keys {
            key.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedInheritanceCallableSignatureV1 {
    exact_signature: DecodedExactCallableSignature,
    effects: DecodedCallableSourceEffectsV1,
    context_keys: Vec<DecodedPersistentId<PersistentExactTypeId>>,
}
impl DecodedInheritanceCallableSignatureV1 {
    pub fn resolve<R: PersistentIdResolver<PersistentExactTypeId, Error = E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<InheritanceCallableSignatureV1, InheritanceCallableSignatureResolutionError<E>>
    {
        let exact = self
            .exact_signature
            .resolve(resolver)
            .map_err(InheritanceCallableSignatureResolutionError::Exact)?;
        let effects = self
            .effects
            .validate()
            .map_err(InheritanceCallableSignatureResolutionError::Effects)?;
        InheritanceCallableSignatureV1::try_new(
            exact,
            effects,
            self.context_keys
                .into_iter()
                .map(|key| {
                    resolver
                        .resolve(key)
                        .map(ContextKey)
                        .map_err(InheritanceCallableSignatureResolutionError::ContextKey)
                })
                .collect::<Result<_, _>>()?,
        )
        .map_err(InheritanceCallableSignatureResolutionError::Signature)
    }
}
impl WireEncode for DecodedInheritanceCallableSignatureV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.exact_signature.encode(encoder)?;
        encoder.field(2)?;
        self.effects.encode(encoder)?;
        encoder.field(3)?;
        encoder.array(self.context_keys.len() as u64)?;
        for key in &self.context_keys {
            key.encode(encoder)?;
        }
        Ok(())
    }
}
impl WireDecode for DecodedInheritanceCallableSignatureV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            exact_signature: decoder.field(1, DecodedExactCallableSignature::decode)?,
            effects: decoder.field(2, DecodedCallableSourceEffectsV1::decode)?,
            context_keys: decoder.field(3, |decoder| {
                decoder.decode_array(|decoder, _| DecodedPersistentId::decode(decoder))
            })?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InheritanceCallableSignatureBuildError {
    Execution,
    MissingReceiver,
    SourceExtern,
}
impl fmt::Display for InheritanceCallableSignatureBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Execution => "inheritance signature execution disagrees with source effects",
            Self::MissingReceiver => "inheritance callable requires an exact receiver",
            Self::SourceExtern => "source extern callable cannot provide dispatch support",
        })
    }
}
impl std::error::Error for InheritanceCallableSignatureBuildError {}

#[derive(Debug)]
pub enum InheritanceCallableSignatureResolutionError<E> {
    Exact(ExactCallableSignatureResolutionError<E>),
    ContextKey(E),
    Effects(CallableSourceEffectsBuildError),
    Signature(InheritanceCallableSignatureBuildError),
}
impl<E: fmt::Display> fmt::Display for InheritanceCallableSignatureResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exact(error) => error.fmt(f),
            Self::ContextKey(error) => error.fmt(f),
            Self::Effects(error) => error.fmt(f),
            Self::Signature(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error
    for InheritanceCallableSignatureResolutionError<E>
{
}
