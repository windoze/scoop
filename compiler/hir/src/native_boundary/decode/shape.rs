use super::*;
use crate::NominalIntrinsicRepresentationV1;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DecodedNativeBoundaryNominalShape {
    Reference,
    Intrinsic(NominalIntrinsicRepresentationV1),
    Struct {
        c_layout: DecodedNativeBoundaryCLayoutPolicy,
        fields: Vec<DecodedNativeBoundaryFieldDefinition>,
    },
    Enum {
        variants: Vec<DecodedNativeBoundaryVariantDefinition>,
    },
}

impl DecodedNativeBoundaryNominalShape {
    pub(super) fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<NativeBoundaryNominalShape, NativeBoundaryResolutionError<E>>
    where
        R: NativeBoundaryResolver<E>,
    {
        match self {
            Self::Reference => Ok(NativeBoundaryNominalShape::Reference),
            Self::Intrinsic(representation) => {
                Ok(NativeBoundaryNominalShape::Intrinsic(representation))
            }
            Self::Struct { c_layout, fields } => Ok(NativeBoundaryNominalShape::Struct {
                c_layout: c_layout.into(),
                fields: resolve_sequence(fields, |field| field.resolve(resolver))?,
            }),
            Self::Enum { variants } => Ok(NativeBoundaryNominalShape::Enum {
                variants: resolve_sequence(variants, |variant| variant.resolve(resolver))?,
            }),
        }
    }
}

impl WireEncode for DecodedNativeBoundaryNominalShape {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Reference => encode_empty_sum(encoder, 1),
            Self::Intrinsic(representation) => encode_value_sum(encoder, 4, representation),
            Self::Struct { c_layout, fields } => {
                encoder.map(3)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                c_layout.encode(encoder)?;
                encoder.field(2)?;
                encode_sequence(encoder, fields)
            }
            Self::Enum { variants } => {
                encoder.map(2)?;
                encode_tag(encoder, 3)?;
                encoder.field(1)?;
                encode_sequence(encoder, variants)
            }
        }
    }
}

impl WireDecode for DecodedNativeBoundaryNominalShape {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::Reference)
            }
            2 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::Struct {
                    c_layout: decoder.field(1, DecodedNativeBoundaryCLayoutPolicy::decode)?,
                    fields: decoder.field(2, |decoder| {
                        decoder.decode_array(|decoder, _| {
                            DecodedNativeBoundaryFieldDefinition::decode(decoder)
                        })
                    })?,
                })
            }
            3 => {
                expect_sum_length(decoder, fields, 2)?;
                Ok(Self::Enum {
                    variants: decoder.field(1, |decoder| {
                        decoder.decode_array(|decoder, _| {
                            DecodedNativeBoundaryVariantDefinition::decode(decoder)
                        })
                    })?,
                })
            }
            4 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, NominalIntrinsicRepresentationV1::decode)
                    .map(Self::Intrinsic)
            }
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}
