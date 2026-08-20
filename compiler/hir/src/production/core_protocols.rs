//! Arena-independent callable contracts referenced by compiler protocols.

use la_arena::{Arena, Idx};
use scoop_identity::{
    DecodedPersistentId, DecodedSignatureCallableShape, DefinitionOriginSubject,
    DefinitionOwnerAtom, DuplicateSignatureKey, Effect, GeneratedCallableKey,
    OptionalSignatureType, PersistentConstructorId, PersistentFunctionId,
    PersistentGeneratedCallableId, PersistentGenericFunctionId, PersistentGenericTypeId,
    PersistentIdResolver, PersistentTypeId, SignatureCallableShape, SignatureTypeKey,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use crate::{
    CanonicalHirFoundation, ClassConstructorId, ExportHir, ExportParameterCalling,
    ExportParameterOwner, FunctionGenericity, FunctionId, HirClassConstructorIdentity,
    HirFunctionIdentity, HirSignatureBinder, HirSignatureTypeMapper, HirSourceFunctionIdentity,
    HirTypeIdentityInputs, TypeId,
};

mod error;
mod production;
mod validation;

pub use error::{
    CoreProtocolBinderError, CoreProtocolCallableBuildError, CoreProtocolCallableValidationError,
    CoreProtocolSignatureReferenceError,
};

/// Kind-specific persistent identity of a callable named by a compiler
/// protocol. Generic templates, source constructors, and generated adapters
/// remain distinct entities all the way through the artifact.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CoreProtocolCallableDefinitionV1 {
    Function(PersistentFunctionId),
    GenericFunction(PersistentGenericFunctionId),
    Constructor(PersistentConstructorId),
    GeneratedCallable(PersistentGeneratedCallableId),
}

impl CoreProtocolCallableDefinitionV1 {
    pub(crate) const fn origin_subject(self) -> DefinitionOriginSubject {
        match self {
            Self::Function(id) => DefinitionOriginSubject::Function(id),
            Self::GenericFunction(id) => DefinitionOriginSubject::GenericFunction(id),
            Self::Constructor(id) => DefinitionOriginSubject::Constructor(id),
            Self::GeneratedCallable(id) => DefinitionOriginSubject::GeneratedCallable(id),
        }
    }
}

impl WireEncode for CoreProtocolCallableDefinitionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Function(id) => encode_value_sum(encoder, 1, id),
            Self::GenericFunction(id) => encode_value_sum(encoder, 2, id),
            Self::Constructor(id) => encode_value_sum(encoder, 3, id),
            Self::GeneratedCallable(id) => encode_value_sum(encoder, 4, id),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedCoreProtocolCallableDefinitionV1 {
    Function(DecodedPersistentId<PersistentFunctionId>),
    GenericFunction(DecodedPersistentId<PersistentGenericFunctionId>),
    Constructor(DecodedPersistentId<PersistentConstructorId>),
    GeneratedCallable(DecodedPersistentId<PersistentGeneratedCallableId>),
}

impl WireEncode for DecodedCoreProtocolCallableDefinitionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Function(id) => encode_value_sum(encoder, 1, id),
            Self::GenericFunction(id) => encode_value_sum(encoder, 2, id),
            Self::Constructor(id) => encode_value_sum(encoder, 3, id),
            Self::GeneratedCallable(id) => encode_value_sum(encoder, 4, id),
        }
    }
}

impl WireDecode for DecodedCoreProtocolCallableDefinitionV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        if fields == 0 {
            return Err(wire_error(
                decoder,
                WireErrorKind::MissingField { field: 0 },
            ));
        }
        let tag = decoder.field(0, Decoder::unsigned)?;
        require_sum_length(decoder, fields, 2)?;
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
                .map(Self::GeneratedCallable),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

/// Complete source-level signature of one compiler protocol callable. The
/// signature is repeated here because internal callables are not part of
/// the direct-public target surface.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CoreProtocolCallableV1 {
    definition: CoreProtocolCallableDefinitionV1,
    signature: SignatureCallableShape,
}

impl CoreProtocolCallableV1 {
    pub const fn definition(&self) -> CoreProtocolCallableDefinitionV1 {
        self.definition
    }

    pub const fn signature(&self) -> &SignatureCallableShape {
        &self.signature
    }

    #[cfg(test)]
    pub(crate) const fn for_test(
        definition: CoreProtocolCallableDefinitionV1,
        signature: SignatureCallableShape,
    ) -> Self {
        Self {
            definition,
            signature,
        }
    }
}

impl WireEncode for CoreProtocolCallableV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.definition.encode(encoder)?;
        encoder.field(2)?;
        self.signature.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCoreProtocolCallableV1 {
    definition: DecodedCoreProtocolCallableDefinitionV1,
    signature: DecodedSignatureCallableShape,
}

impl WireEncode for DecodedCoreProtocolCallableV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.definition.encode(encoder)?;
        encoder.field(2)?;
        self.signature.encode(encoder)
    }
}

impl WireDecode for DecodedCoreProtocolCallableV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            definition: decoder.field(1, DecodedCoreProtocolCallableDefinitionV1::decode)?,
            signature: decoder.field(2, DecodedSignatureCallableShape::decode)?,
        })
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

fn require_sum_length(decoder: &Decoder<'_>, actual: u64, expected: u64) -> Result<(), WireError> {
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

#[cfg(test)]
mod tests;
