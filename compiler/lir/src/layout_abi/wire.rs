use super::*;

macro_rules! encode_target {
    ($type:ty) => {
        impl WireEncode for $type {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                let (tag, value): (u64, &dyn WireEncode) = match self {
                    Self::Layout(value) => (1, value),
                    Self::Descriptor(value) => (2, value),
                    Self::Dispatch(value) => (3, value),
                    Self::Callable(value) => (4, value),
                    Self::ShapeSupport(value) => (5, value),
                };
                encode_tag(encoder, tag)?;
                encoder.field(1)?;
                value.encode(encoder)
            }
        }
    };
}

encode_target!(LayoutAbiSemanticTargetV1);
encode_target!(DecodedLayoutAbiSemanticTargetV1);

impl WireDecode for DecodedLayoutAbiSemanticTargetV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(match decoder.field(0, Decoder::unsigned)? {
            1 => Self::Layout(decoder.field(1, DecodedPersistentId::decode)?),
            2 => Self::Descriptor(decoder.field(1, DecodedPersistentId::decode)?),
            3 => Self::Dispatch(decoder.field(1, DecodedPersistentId::decode)?),
            4 => Self::Callable(decoder.field(1, DecodedCallableDefinitionOwner::decode)?),
            5 => Self::ShapeSupport(decoder.field(1, DecodedPersistentId::decode)?),
            tag => return Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        })
    }
}

macro_rules! encode_dependency {
    ($type:ty) => {
        impl WireEncode for $type {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.map(2)?;
                encoder.field(1)?;
                self.provider.encode(encoder)?;
                encoder.field(2)?;
                self.target.encode(encoder)
            }
        }
    };
}

encode_dependency!(LayoutAbiDependencyV1);
encode_dependency!(DecodedLayoutAbiDependencyV1);

impl WireDecode for DecodedLayoutAbiDependencyV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            provider: decoder.field(1, DecodedPersistentId::decode)?,
            target: decoder.field(2, DecodedLayoutAbiSemanticTargetV1::decode)?,
        })
    }
}
