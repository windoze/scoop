//! Explicit C projections of otherwise ordinary source nominal storage.

use super::*;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NativeBoundaryCAbiV1 {
    SourceRepresentation,
    UInt64Field {
        field: PersistentFieldId,
    },
    NullablePointer {
        none: PersistentEnumVariantId,
        payload: PersistentEnumVariantFieldId,
    },
}

impl NativeBoundaryCAbiV1 {
    pub(super) fn validate_shape(
        self,
        shape: &NativeBoundaryNominalShape,
    ) -> Result<(), NativeBoundaryDefinitionError> {
        let valid = match (self, shape) {
            (Self::SourceRepresentation, _) => true,
            (
                Self::UInt64Field { field },
                NativeBoundaryNominalShape::Struct {
                    c_layout: NativeBoundaryCLayoutPolicy::NotCLayout,
                    fields,
                },
            ) => matches!(fields.as_slice(), [only] if only.field() == field),
            (
                Self::NullablePointer { none, payload },
                NativeBoundaryNominalShape::Enum { variants },
            ) => {
                variants.len() == 2
                    && variants
                        .iter()
                        .any(|variant| variant.variant() == none && variant.fields().is_empty())
                    && variants.iter().any(|variant| {
                        variant.variant() != none
                            && matches!(variant.fields(), [only] if only.field() == payload)
                    })
            }
            _ => false,
        };
        if valid {
            Ok(())
        } else {
            Err(NativeBoundaryDefinitionError::CAbiProjectionMismatch)
        }
    }
}

impl WireEncode for NativeBoundaryCAbiV1 {
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
