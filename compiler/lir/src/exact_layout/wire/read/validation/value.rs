use super::*;

impl RawValue {
    pub(super) fn validate_against(
        self,
        expected: &ExactRepresentationLayoutV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), ExactLayoutWireError> {
        use ExactRepresentationKindV1 as E;
        match (self, expected.kind()) {
            (Self::Scalar(raw), E::Scalar(expected)) if raw == expected => Ok(()),
            (Self::QualifiedPointer(raw), E::QualifiedPointer(expected)) if raw == expected => {
                Ok(())
            }
            (Self::Unit, E::IntrinsicValue(IntrinsicValueFamilyV1::Unit)) => Ok(()),
            (
                Self::Struct {
                    policy,
                    interior_mutable,
                    fields: raw,
                },
                E::Struct(expected),
            ) => {
                if interior_mutable != expected.interior_mutable() {
                    return Err(ExactLayoutWireError::RepresentationMismatch);
                }
                policy.validate_against(expected.policy())?;
                fields::nominal_fields(raw, expected.fields(), meter)
            }
            (Self::Tuple(raw), E::Tuple(expected)) => {
                table_length(raw.len(), expected.elements().len(), meter)?;
                for (raw, expected) in raw.into_iter().zip(expected.elements()) {
                    raw.validate_against(expected)?;
                }
                Ok(())
            }
            (
                Self::TaggedEnum {
                    tag,
                    pure,
                    variants,
                },
                E::TaggedEnum(expected),
            ) => {
                tag.validate_against(expected.geometry().tag_layout())?;
                pure.validate_against(expected.geometry().pure_region())?;
                table_length(variants.len(), expected.variants().len(), meter)?;
                for ((raw, expected), geometry) in variants
                    .into_iter()
                    .zip(expected.variants())
                    .zip(expected.geometry().variants())
                {
                    raw.slot.validate_against(geometry)?;
                    raw.variant.validate_against(expected, meter)?;
                }
                Ok(())
            }
            (
                Self::NicheEnum {
                    pointer,
                    variants,
                    payload,
                },
                E::NicheEnum(expected),
            ) => {
                if pointer != expected.pointer_kind() {
                    return Err(ExactLayoutWireError::RepresentationMismatch);
                }
                verify(payload, expected.payload_variant())?;
                table_length(variants.len(), expected.variants().len(), meter)?;
                for (raw, expected) in variants.into_iter().zip(expected.variants()) {
                    raw.validate_against(expected, meter)?;
                }
                Ok(())
            }
            _ => Err(ExactLayoutWireError::RepresentationMismatch),
        }
    }
}

impl RawPolicy {
    fn validate_against(self, expected: &StructLayoutPolicyV1) -> Result<(), ExactLayoutWireError> {
        match (self, expected) {
            (Self::Ordinary, StructLayoutPolicyV1::Ordinary(_)) => Ok(()),
            (
                Self::CLayout {
                    aligned,
                    packed,
                    contract,
                },
                StructLayoutPolicyV1::CLayout(expected),
            ) if aligned == expected.contract().layout().aligned()
                && packed == expected.contract().layout().packed() =>
            {
                verify(contract, expected.contract().fingerprint())
            }
            _ => Err(ExactLayoutWireError::RepresentationMismatch),
        }
    }
}
