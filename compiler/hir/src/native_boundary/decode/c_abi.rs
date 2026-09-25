use super::*;
use crate::NativeBoundaryCAbiV1;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) enum DecodedNativeBoundaryCAbiV1 {
    SourceRepresentation,
    UInt64Field {
        field: DecodedPersistentId<PersistentFieldId>,
    },
    NullablePointer {
        none: DecodedPersistentId<PersistentEnumVariantId>,
        payload: DecodedPersistentId<PersistentEnumVariantFieldId>,
    },
}

impl DecodedNativeBoundaryCAbiV1 {
    pub(super) fn resolve(
        self,
        shape: &NativeBoundaryNominalShape,
    ) -> Result<NativeBoundaryCAbiV1, NativeBoundaryDefinitionError> {
        let mismatch = NativeBoundaryDefinitionError::CAbiProjectionMismatch;
        let projection = match (self, shape) {
            (Self::SourceRepresentation, _) => NativeBoundaryCAbiV1::SourceRepresentation,
            (Self::UInt64Field { field }, NativeBoundaryNominalShape::Struct { fields, .. }) => {
                let [only] = fields.as_slice() else {
                    return Err(mismatch);
                };
                NativeBoundaryCAbiV1::UInt64Field {
                    field: field.verify(only.field()).map_err(|_| mismatch)?,
                }
            }
            (
                Self::NullablePointer { none, payload },
                NativeBoundaryNominalShape::Enum { variants },
            ) => {
                let [first, second] = variants.as_slice() else {
                    return Err(mismatch);
                };
                let (empty, present) = if none.verify(first.variant()).is_ok() {
                    (first, second)
                } else {
                    (second, first)
                };
                let [field] = present.fields() else {
                    return Err(mismatch);
                };
                NativeBoundaryCAbiV1::NullablePointer {
                    none: none.verify(empty.variant()).map_err(|_| mismatch.clone())?,
                    payload: payload.verify(field.field()).map_err(|_| mismatch)?,
                }
            }
            _ => return Err(mismatch),
        };
        projection.validate_shape(shape)?;
        Ok(projection)
    }
}

impl WireEncode for DecodedNativeBoundaryCAbiV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::SourceRepresentation => encode_empty_sum(encoder, 1),
            Self::UInt64Field { field } => encode_value_sum(encoder, 2, field),
            Self::NullablePointer { none, payload } => {
                encode_two_value_sum(encoder, 3, none, payload)
            }
        }
    }
}

impl WireDecode for DecodedNativeBoundaryCAbiV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::SourceRepresentation)
            }
            2 => {
                expect_sum_length(decoder, fields, 2)?;
                Ok(Self::UInt64Field {
                    field: decoder.field(1, DecodedPersistentId::decode)?,
                })
            }
            3 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::NullablePointer {
                    none: decoder.field(1, DecodedPersistentId::decode)?,
                    payload: decoder.field(2, DecodedPersistentId::decode)?,
                })
            }
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}
