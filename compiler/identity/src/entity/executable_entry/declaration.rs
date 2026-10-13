use crate::{
    CallableOwner, CallableTemplateOwner, DecodedPersistentId, PersistentFunctionId,
    PersistentGeneratedCallableId, PersistentIdResolver, StrongCallableDefinitionOwner,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

/// Entry declarations keep source and generated callable identities distinct.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExecutableEntryDeclaration {
    Source(PersistentFunctionId),
    Generated(PersistentGeneratedCallableId),
}

impl ExecutableEntryDeclaration {
    pub const fn owner(self) -> CallableOwner {
        match self {
            Self::Source(id) => CallableOwner::Function(id),
            Self::Generated(id) => CallableOwner::Generated(id),
        }
    }

    pub const fn strong_owner(self) -> StrongCallableDefinitionOwner {
        match self {
            Self::Source(id) => StrongCallableDefinitionOwner::Function(id),
            Self::Generated(id) => StrongCallableDefinitionOwner::GeneratedCallable(id),
        }
    }

    pub const fn template(self) -> CallableTemplateOwner {
        match self {
            Self::Source(id) => CallableTemplateOwner::Function(id),
            Self::Generated(id) => CallableTemplateOwner::Generated(id),
        }
    }
}

impl WireEncode for ExecutableEntryDeclaration {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        match self {
            Self::Source(id) => {
                encoder.unsigned(1)?;
                encoder.field(1)?;
                id.encode(encoder)
            }
            Self::Generated(id) => {
                encoder.unsigned(2)?;
                encoder.field(1)?;
                id.encode(encoder)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedExecutableEntryDeclaration {
    Source(DecodedPersistentId<PersistentFunctionId>),
    Generated(DecodedPersistentId<PersistentGeneratedCallableId>),
}

impl DecodedExecutableEntryDeclaration {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<ExecutableEntryDeclaration, E>
    where
        R: PersistentIdResolver<PersistentFunctionId, Error = E>
            + PersistentIdResolver<PersistentGeneratedCallableId, Error = E>,
    {
        match self {
            Self::Source(id) => resolver.resolve(id).map(ExecutableEntryDeclaration::Source),
            Self::Generated(id) => resolver
                .resolve(id)
                .map(ExecutableEntryDeclaration::Generated),
        }
    }

    pub fn as_array(&self) -> &[u8; 32] {
        match self {
            Self::Source(id) => id.as_array(),
            Self::Generated(id) => id.as_array(),
        }
    }
}

impl WireEncode for DecodedExecutableEntryDeclaration {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        match self {
            Self::Source(id) => {
                encoder.unsigned(1)?;
                encoder.field(1)?;
                id.encode(encoder)
            }
            Self::Generated(id) => {
                encoder.unsigned(2)?;
                encoder.field(1)?;
                id.encode(encoder)
            }
        }
    }
}

impl WireDecode for DecodedExecutableEntryDeclaration {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Source),
            2 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Generated),
            tag => Err(WireError::new(
                WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        }
    }
}
