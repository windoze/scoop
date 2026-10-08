use super::*;
use crate::DecodedInheritanceCallableDeclarationV1;
use scoop_identity::{
    DecodedPersistentId, PersistentFunctionId, PersistentIdResolver, PersistentPropertyAccessorId,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

mod edges;
mod usage;
pub use edges::*;
pub use usage::*;

pub trait SelectedTypeUseResolver<E>:
    PersistentIdResolver<ConeIdentity, Error = E>
    + PersistentIdResolver<PersistentExactTypeId, Error = E>
    + PersistentIdResolver<PersistentConstructorId, Error = E>
    + PersistentIdResolver<PersistentEnumVariantId, Error = E>
    + PersistentIdResolver<PersistentFunctionId, Error = E>
    + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
    + PersistentIdResolver<PersistentDispatchSlotId, Error = E>
    + PersistentIdResolver<PersistentObjectValueId, Error = E>
{
}
impl<R, E> SelectedTypeUseResolver<E> for R where
    R: PersistentIdResolver<ConeIdentity, Error = E>
        + PersistentIdResolver<PersistentExactTypeId, Error = E>
        + PersistentIdResolver<PersistentConstructorId, Error = E>
        + PersistentIdResolver<PersistentEnumVariantId, Error = E>
        + PersistentIdResolver<PersistentFunctionId, Error = E>
        + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
        + PersistentIdResolver<PersistentDispatchSlotId, Error = E>
        + PersistentIdResolver<PersistentObjectValueId, Error = E>
{
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedSelectedExternalTypeUseV1 {
    provider: DecodedPersistentId<ConeIdentity>,
    usage: DecodedSelectedTypeUseV1,
}
impl DecodedSelectedExternalTypeUseV1 {
    pub fn resolve<R: SelectedTypeUseResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<SelectedExternalTypeUseV1, SelectedTypeUseResolutionError<E>> {
        let provider = resolver
            .resolve(self.provider)
            .map_err(SelectedTypeUseResolutionError::Reference)?;
        let usage = self.usage.resolve_at_depth(resolver)?;
        Ok(SelectedExternalTypeUseV1::new(provider, usage))
    }
}
impl WireEncode for DecodedSelectedExternalTypeUseV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.provider.encode(encoder)?;
        encoder.field(2)?;
        self.usage.encode(encoder)
    }
}
impl WireDecode for DecodedSelectedExternalTypeUseV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            provider: decoder.field(1, DecodedPersistentId::decode)?,
            usage: decoder.field(2, DecodedSelectedTypeUseV1::decode)?,
        })
    }
}
