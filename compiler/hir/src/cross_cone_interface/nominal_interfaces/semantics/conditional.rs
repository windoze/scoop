//! Conditional relations use the original nominal's binder domain.

use super::*;

impl NominalInterfaceRecordV1 {
    pub(super) fn validate_conditional_interfaces<A, E>(
        &self,
        authority: &mut A,
    ) -> Result<(), NominalInterfaceSemanticValidationError<E>>
    where
        A: NominalInterfaceSemanticAuthority<E>,
    {
        for (index, implementation) in self.details.conditional_interfaces().iter().enumerate() {
            implementation
                .parameters()
                .validate_bound_semantics(None, authority)
                .map_err(|error| {
                    NominalInterfaceSemanticValidationError::ConditionalParameters { index, error }
                })?;
            let error = |error| NominalInterfaceSemanticValidationError::ConditionalInterface {
                index,
                error,
            };
            let shape = implementation
                .parameters()
                .signature_scope(None)
                .validate_nominal_signature_semantics(implementation.interface(), authority)
                .map_err(|source| error(ExactSupertypeSemanticError::Signature(source)))?;
            if shape.kind() != PublicNominalKindV1::Interface {
                return Err(error(ExactSupertypeSemanticError::InvalidTargetKind(
                    shape.kind(),
                )));
            }
        }
        Ok(())
    }
}
