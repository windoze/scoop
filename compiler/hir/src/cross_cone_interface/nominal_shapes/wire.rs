use super::*;

impl WireEncode for NominalSourceShapeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Class(fields) => {
                encoder.map(2)?;
                encode_tag(encoder, 7)?;
                encoder.field(1)?;
                fields.encode(encoder)
            }
            Self::Interface => encode_empty_sum(encoder, 2),
            Self::Struct(shape) => {
                encoder.map(3)?;
                encode_tag(encoder, 3)?;
                encoder.field(1)?;
                shape.fields.encode(encoder)?;
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
                encoder.map(3)?;
                encode_tag(encoder, 8)?;
                encoder.field(1)?;
                shape.value.encode(encoder)?;
                encoder.field(2)?;
                shape.fields.encode(encoder)
            }
            Self::Intrinsic(representation) => encode_intrinsic(encoder, *representation),
        }
    }
}

impl WireEncode for DecodedNominalSourceShapeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Class(fields) => {
                encoder.map(2)?;
                encode_tag(encoder, 7)?;
                encoder.field(1)?;
                encode_sequence(encoder, fields)
            }
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
            Self::Object { value, fields } => {
                encoder.map(3)?;
                encode_tag(encoder, 8)?;
                encoder.field(1)?;
                value.encode(encoder)?;
                encoder.field(2)?;
                encode_sequence(encoder, fields)
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
            7 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, |decoder| {
                        decoder
                            .decode_array(|decoder, _| DecodedNominalSourceFieldV1::decode(decoder))
                    })
                    .map(Self::Class)
            }
            2 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::Interface)
            }
            3 => {
                expect_sum_length(decoder, fields, 3)?;
                let fields = decoder.field(1, |decoder| {
                    decoder.decode_array(|decoder, _| DecodedNominalSourceFieldV1::decode(decoder))
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
            8 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::Object {
                    value: decoder.field(1, DecodedPersistentId::decode)?,
                    fields: decoder.field(2, |decoder| {
                        decoder
                            .decode_array(|decoder, _| DecodedNominalSourceFieldV1::decode(decoder))
                    })?,
                })
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
