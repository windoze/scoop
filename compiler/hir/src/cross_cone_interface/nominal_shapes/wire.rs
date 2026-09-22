use super::*;

impl WireEncode for NominalSourceShapeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Class => encode_empty_sum(encoder, 1),
            Self::Interface => encode_empty_sum(encoder, 2),
            Self::Struct(shape) => {
                encoder.map(3)?;
                encode_tag(encoder, 3)?;
                encoder.field(1)?;
                encode_sequence(encoder, &shape.fields)?;
                encoder.field(2)?;
                shape.c_layout_policy.encode(encoder)
            }
            Self::Enum(shape) => {
                encoder.map(2)?;
                encode_tag(encoder, 4)?;
                encoder.field(1)?;
                encode_sequence(encoder, &shape.variants)
            }
            Self::Object(shape) => {
                encoder.map(2)?;
                encode_tag(encoder, 5)?;
                encoder.field(1)?;
                shape.value.encode(encoder)
            }
            Self::Intrinsic(representation) => encode_intrinsic(encoder, *representation),
        }
    }
}

impl WireEncode for DecodedNominalSourceShapeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Class => encode_empty_sum(encoder, 1),
            Self::Interface => encode_empty_sum(encoder, 2),
            Self::Struct {
                fields,
                c_layout_policy,
            } => {
                encoder.map(3)?;
                encode_tag(encoder, 3)?;
                encoder.field(1)?;
                encode_sequence(encoder, fields)?;
                encoder.field(2)?;
                c_layout_policy.encode(encoder)
            }
            Self::Enum(variants) => {
                encoder.map(2)?;
                encode_tag(encoder, 4)?;
                encoder.field(1)?;
                encode_sequence(encoder, variants)
            }
            Self::Object(value) => {
                encoder.map(2)?;
                encode_tag(encoder, 5)?;
                encoder.field(1)?;
                value.encode(encoder)
            }
            Self::Intrinsic(representation) => encode_intrinsic(encoder, *representation),
        }
    }
}

impl WireDecode for DecodedNominalSourceShapeV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::Class)
            }
            2 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::Interface)
            }
            3 => {
                expect_sum_length(decoder, fields, 3)?;
                let fields = decoder.field(1, |decoder| {
                    decoder.decode_array(|decoder, _| DecodedStructSourceFieldV1::decode(decoder))
                })?;
                let c_layout_policy = decoder.field(2, NominalCLayoutPolicyV1::decode)?;
                Ok(Self::Struct {
                    fields,
                    c_layout_policy,
                })
            }
            4 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, |decoder| {
                        decoder
                            .decode_array(|decoder, _| DecodedEnumSourceVariantV1::decode(decoder))
                    })
                    .map(Self::Enum)
            }
            5 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedPersistentId::decode)
                    .map(Self::Object)
            }
            6 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, NominalIntrinsicRepresentationV1::decode)
                    .map(Self::Intrinsic)
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

fn encode_intrinsic(
    encoder: &mut Encoder,
    representation: NominalIntrinsicRepresentationV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, 6)?;
    encoder.field(1)?;
    representation.encode(encoder)
}
