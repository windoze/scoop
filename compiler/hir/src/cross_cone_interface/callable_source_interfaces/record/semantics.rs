use scoop_identity::{ConeIdentity, NonEmptyVec, PersistentGenericTypeId, SignatureTypeKey};

use super::CallableSourceInterfaceV1;
use crate::{CallableInterfaceRecordV1, CallableSourceParameterV1, ExportDefinitionSourceV1};

mod errors;

pub use errors::CallableSourceInterfaceSemanticValidationError;

/// Trusted facts needed to validate one source-call protocol.
pub trait CallableSourceInterfaceSemanticAuthority<E> {
    fn current_cone(&self) -> ConeIdentity;

    /// Returns the canonical generic `Array` declaration from trusted core.
    fn canonical_array_type(&mut self) -> Result<PersistentGenericTypeId, E>;

    /// Validates that this parameter origin belongs to the declared owner and
    /// source position, including source/context membership and point bounds.
    fn validate_source_parameter_origin(
        &mut self,
        owner: crate::CallableDeclarationId,
        position: u32,
        origin: &ExportDefinitionSourceV1,
    ) -> Result<(), E>;
}

impl CallableSourceInterfaceV1 {
    pub fn validate_semantics<A, E>(
        &self,
        callable: &CallableInterfaceRecordV1,
        authority: &mut A,
    ) -> Result<(), CallableSourceInterfaceSemanticValidationError<E>>
    where
        A: CallableSourceInterfaceSemanticAuthority<E>,
    {
        if self.owner != callable.declaration() {
            return Err(
                CallableSourceInterfaceSemanticValidationError::CallableDeclaration {
                    expected: self.owner,
                    actual: callable.declaration(),
                },
            );
        }

        let expected_parameters = callable.parameters().parameters();
        let actual_parameters = self.parameters.parameters();
        if actual_parameters.len() != expected_parameters.len() {
            return Err(
                CallableSourceInterfaceSemanticValidationError::ParameterArity {
                    expected: expected_parameters.len(),
                    actual: actual_parameters.len(),
                },
            );
        }

        for (position, (actual, expected)) in
            (0_u32..).zip(actual_parameters.iter().zip(expected_parameters))
        {
            let index = position as usize;
            if actual.name() != expected.name() {
                return Err(
                    CallableSourceInterfaceSemanticValidationError::ParameterName {
                        index,
                        expected: expected.name().clone(),
                        actual: actual.name().clone(),
                    },
                );
            }
            if actual.value_type() != expected.value_type() {
                return Err(
                    CallableSourceInterfaceSemanticValidationError::ParameterType {
                        index,
                        expected: Box::new(expected.value_type().clone()),
                        actual: Box::new(actual.value_type().clone()),
                    },
                );
            }
            self.validate_vararg(index, actual, authority)?;

            let origin_cone = actual.definition_origin().origin().source().cone();
            let current_cone = authority.current_cone();
            if origin_cone != current_cone {
                return Err(
                    CallableSourceInterfaceSemanticValidationError::DefinitionOriginCone {
                        index,
                        expected: current_cone,
                        actual: origin_cone,
                    },
                );
            }
            authority
                .validate_source_parameter_origin(self.owner, position, actual.definition_origin())
                .map_err(|error| {
                    CallableSourceInterfaceSemanticValidationError::DefinitionOrigin {
                        index,
                        error,
                    }
                })?;
        }
        Ok(())
    }

    fn validate_vararg<A, E>(
        &self,
        index: usize,
        parameter: &CallableSourceParameterV1,
        authority: &mut A,
    ) -> Result<(), CallableSourceInterfaceSemanticValidationError<E>>
    where
        A: CallableSourceInterfaceSemanticAuthority<E>,
    {
        let Some(element_type) = parameter.calling().element_type() else {
            return Ok(());
        };
        let array = authority.canonical_array_type().map_err(|error| {
            CallableSourceInterfaceSemanticValidationError::CanonicalArrayType { index, error }
        })?;
        let expected = SignatureTypeKey::NominalApplication {
            origin: array,
            arguments: NonEmptyVec::from_first(element_type.clone(), []),
        };
        if parameter.value_type() != &expected {
            return Err(
                CallableSourceInterfaceSemanticValidationError::VarargArrayType {
                    index,
                    expected: Box::new(expected),
                    actual: Box::new(parameter.value_type().clone()),
                },
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
