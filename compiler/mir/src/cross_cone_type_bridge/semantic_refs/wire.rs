use super::*;
use scoop_identity::DecodedCallableDefinitionOwner;

macro_rules! encode_target {
    ($ty:ty) => {
        impl WireEncode for $ty {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                let (kind, value): (u64, &dyn WireEncode) = match self {
                    Self::Type(value) => (1, value),
                    Self::Callable(value) => (2, value),
                    Self::Dispatch(value) => (3, value),
                    Self::Object(value) => (4, value),
                    Self::ShapeSupport(value) => (5, value),
                    Self::InitializationUnit(value) => (6, value),
                };
                tag(encoder, 2, kind)?;
                encoder.field(1)?;
                value.encode(encoder)
            }
        }
    };
}
encode_target!(MirTypeBridgeTargetV1);
encode_target!(DecodedMirTypeBridgeTargetV1);

impl WireDecode for DecodedMirTypeBridgeTargetV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let count = decoder.map()?;
        fields(decoder, count, 2)?;
        Ok(match decoder.field(0, Decoder::unsigned)? {
            1 => Self::Type(decoder.field(1, DecodedPersistentId::decode)?),
            2 => Self::Callable(decoder.field(1, DecodedCallableDefinitionOwner::decode)?),
            3 => Self::Dispatch(decoder.field(1, DecodedPersistentId::decode)?),
            4 => Self::Object(decoder.field(1, DecodedPersistentId::decode)?),
            5 => Self::ShapeSupport(decoder.field(1, DecodedPersistentId::decode)?),
            6 => Self::InitializationUnit(decoder.field(1, DecodedPersistentId::decode)?),
            tag => return Err(error(decoder, WireErrorKind::UnknownTag { tag })),
        })
    }
}
