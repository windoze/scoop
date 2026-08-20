use super::NestedSourceMemberRefV1;
use scoop_identity::{
    DecodedPersistentId, PersistentFunctionId, PersistentGenericFunctionId, PersistentIdResolver,
    PersistentPropertyId,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedNestedSourceMemberRefV1 {
    Function(DecodedPersistentId<PersistentFunctionId>),
    GenericFunction(DecodedPersistentId<PersistentGenericFunctionId>),
    Property(DecodedPersistentId<PersistentPropertyId>),
}
impl DecodedNestedSourceMemberRefV1 {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<NestedSourceMemberRefV1, E>
    where
        R: PersistentIdResolver<PersistentFunctionId, Error = E>
            + PersistentIdResolver<PersistentGenericFunctionId, Error = E>
            + PersistentIdResolver<PersistentPropertyId, Error = E>,
    {
        Ok(match self {
            Self::Function(id) => NestedSourceMemberRefV1::Function(resolver.resolve(id)?),
            Self::GenericFunction(id) => {
                NestedSourceMemberRefV1::GenericFunction(resolver.resolve(id)?)
            }
            Self::Property(id) => NestedSourceMemberRefV1::Property(resolver.resolve(id)?),
        })
    }
}
impl WireEncode for DecodedNestedSourceMemberRefV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Function(id) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(1)?;
                encoder.field(1)?;
                id.encode(encoder)
            }
            Self::GenericFunction(id) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                id.encode(encoder)
            }
            Self::Property(id) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(3)?;
                encoder.field(1)?;
                id.encode(encoder)
            }
        }
    }
}
impl WireDecode for DecodedNestedSourceMemberRefV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Function),
            2 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::GenericFunction),
            3 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Property),
            tag => Err(WireError::new(
                WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        }
    }
}
