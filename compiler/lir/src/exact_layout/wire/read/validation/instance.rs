use super::*;

impl RawInstance {
    pub(super) fn validate_against(
        self,
        expected: &InstanceRepresentationV1,
    ) -> Result<(), ExactLayoutWireError> {
        use InstanceRepresentationKindV1 as E;
        match (self, expected.kind()) {
            (
                Self::Class {
                    base,
                    declared,
                    complete,
                },
                E::ClassObject(expected),
            ) => {
                base.validate_against(expected.base_prefix())?;
                fields::nominal_fields(declared, expected.declared_fields())?;
                fields::nominal_fields(complete, expected.complete_fields())
            }
            (Self::Box { exact, layout }, E::BoxedPayload(expected)) => {
                verify(exact, expected.exact())?;
                verify(layout, expected.layout())
            }
            (Self::InlineBytes, E::InlineBytes)
            | (Self::AbstractReference, E::AbstractReference) => Ok(()),
            (
                Self::InlineArray { exact, storage },
                E::InlineArray {
                    element,
                    storage: expected,
                },
            ) => {
                verify(exact, element.exact())?;
                storage.validate_against(expected)?;
                Ok(())
            }
            _ => Err(ExactLayoutWireError::RepresentationMismatch),
        }
    }
}

impl RawBase {
    fn validate_against(
        self,
        expected: crate::ClassBasePrefixV1,
    ) -> Result<(), ExactLayoutWireError> {
        match (self, expected) {
            (Self::NoBase, crate::ClassBasePrefixV1::NoBase) => Ok(()),
            (
                Self::Prefix {
                    exact,
                    layout,
                    size,
                    alignment,
                },
                crate::ClassBasePrefixV1::BasePrefix {
                    exact: expected_exact,
                    layout: expected_layout,
                    byte_size,
                    alignment: expected_alignment,
                },
            ) if size == byte_size && alignment == expected_alignment.get() => {
                verify(exact, expected_exact)?;
                verify(layout, expected_layout)
            }
            _ => Err(ExactLayoutWireError::RepresentationMismatch),
        }
    }
}
