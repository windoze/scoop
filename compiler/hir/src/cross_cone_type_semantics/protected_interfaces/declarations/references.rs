use super::*;
use crate::{CallableDeclarationIdResolver, DecodedSourceNominalId};
use scoop_identity::{
    CallableTemplateOrigin, DecodedCallableTemplateOrigin, DecodedPersistentId,
    PersistentConstructorId, PersistentIdResolver, PersistentPropertyId,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProtectedCallableDeclarationRefV1(pub(super) CallableTemplateOrigin);
impl ProtectedCallableDeclarationRefV1 {
    pub fn try_new(
        declaration: CallableTemplateOrigin,
    ) -> Result<Self, ProtectedDeclarationTableError> {
        match declaration {
            CallableTemplateOrigin::Function(_)
            | CallableTemplateOrigin::GenericFunction(_)
            | CallableTemplateOrigin::Accessor(_) => Ok(Self(declaration)),
            CallableTemplateOrigin::Constructor(_)
            | CallableTemplateOrigin::VariantConstructor(_) => {
                Err(ProtectedDeclarationTableError::CallableKind)
            }
        }
    }
    pub const fn declaration(self) -> CallableTemplateOrigin {
        self.0
    }
}
impl WireEncode for ProtectedCallableDeclarationRefV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.0.encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProtectedDeclarationRefV1 {
    Callable(ProtectedCallableDeclarationRefV1),
    Constructor(PersistentConstructorId),
    Property(PersistentPropertyId),
    NestedNominal(SourceNominalId),
}
impl WireEncode for ProtectedDeclarationRefV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let (tag, id): (u64, &dyn WireEncode) = match self {
            Self::Callable(id) => (1, id),
            Self::Constructor(id) => (2, id),
            Self::Property(id) => (3, id),
            Self::NestedNominal(id) => (4, id),
        };
        wire::tag(encoder, 2, tag)?;
        encoder.field(1)?;
        id.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedProtectedDeclarationRefV1 {
    Callable(DecodedCallableTemplateOrigin),
    Constructor(DecodedPersistentId<PersistentConstructorId>),
    Property(DecodedPersistentId<PersistentPropertyId>),
    NestedNominal(DecodedSourceNominalId),
}
impl DecodedProtectedDeclarationRefV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<ProtectedDeclarationRefV1, ProtectedDeclarationResolutionError<E>>
    where
        R: CallableDeclarationIdResolver<E>
            + crate::SignatureTypeReferenceResolver<E>
            + PersistentIdResolver<PersistentPropertyId, Error = E>,
    {
        use ProtectedDeclarationResolutionError as Error;
        Ok(match self {
            Self::Callable(id) => ProtectedDeclarationRefV1::Callable(
                ProtectedCallableDeclarationRefV1::try_new(
                    id.resolve(resolver).map_err(Error::Foundation)?,
                )
                .map_err(Error::Table)?,
            ),
            Self::Constructor(id) => ProtectedDeclarationRefV1::Constructor(
                resolver.resolve(id).map_err(Error::Foundation)?,
            ),
            Self::Property(id) => ProtectedDeclarationRefV1::Property(
                resolver.resolve(id).map_err(Error::Foundation)?,
            ),
            Self::NestedNominal(id) => ProtectedDeclarationRefV1::NestedNominal(
                id.resolve(resolver).map_err(Error::Foundation)?,
            ),
        })
    }
}
impl WireDecode for DecodedProtectedDeclarationRefV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => decoder
                .field(1, DecodedCallableTemplateOrigin::decode)
                .map(Self::Callable),
            2 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Constructor),
            3 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Property),
            4 => decoder
                .field(1, DecodedSourceNominalId::decode)
                .map(Self::NestedNominal),
            tag => Err(wire::error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}
impl WireEncode for DecodedProtectedDeclarationRefV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let (tag, id): (u64, &dyn WireEncode) = match self {
            Self::Callable(id) => (1, id),
            Self::Constructor(id) => (2, id),
            Self::Property(id) => (3, id),
            Self::NestedNominal(id) => (4, id),
        };
        wire::tag(encoder, 2, tag)?;
        encoder.field(1)?;
        id.encode(encoder)
    }
}
