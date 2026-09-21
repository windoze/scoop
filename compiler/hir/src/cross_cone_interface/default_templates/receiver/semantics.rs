use std::fmt;

use scoop_identity::{LocalValueSelector, SignatureTypeKey};

use super::OptionalTemplateReceiverV1;
use crate::{
    CallableInterfaceRecordV1, CanonicalBinderUseListV1, CanonicalBooleanV1,
    CanonicalTemplateLocalTableV1, DefaultTemplateProviderShapeV1,
    DefaultTemplateTypeSubstitutionError,
};

impl OptionalTemplateReceiverV1 {
    pub fn validate_semantics(
        &self,
        callable: &CallableInterfaceRecordV1,
        locals: &CanonicalTemplateLocalTableV1,
        provider: DefaultTemplateProviderShapeV1,
        type_parameters: &CanonicalBinderUseListV1,
    ) -> Result<(), TemplateReceiverSemanticValidationError> {
        let Some((receiver, expected_type)) =
            self.checked_receiver_unmetered(callable.receiver(), locals)?
        else {
            return Ok(());
        };
        let mapped_type = type_parameters
            .substitute_provider_type(provider, receiver.value_type())
            .map_err(TemplateReceiverSemanticValidationError::TypeSubstitution)?;
        if &mapped_type != expected_type {
            return Err(TemplateReceiverSemanticValidationError::CallableType {
                expected: Box::new(expected_type.clone()),
                actual: Box::new(mapped_type),
            });
        }
        Ok(())
    }

    /// Checks the unchanged source receiver in the original provider scope.
    pub fn validate_provider_semantics(
        &self,
        expected: Option<&SignatureTypeKey>,
        locals: &CanonicalTemplateLocalTableV1,
    ) -> Result<(), TemplateReceiverSemanticValidationError> {
        let Some((receiver, expected)) = self.checked_receiver_unmetered(expected, locals)? else {
            return Ok(());
        };
        if receiver.value_type() != expected {
            return Err(TemplateReceiverSemanticValidationError::CallableType {
                expected: Box::new(expected.clone()),
                actual: Box::new(receiver.value_type().clone()),
            });
        }
        Ok(())
    }

    fn checked_receiver_unmetered<'r, 'e>(
        &'r self,
        expected: Option<&'e SignatureTypeKey>,
        locals: &CanonicalTemplateLocalTableV1,
    ) -> Result<
        Option<(&'r super::TemplateReceiverV1, &'e SignatureTypeKey)>,
        TemplateReceiverSemanticValidationError,
    > {
        let (receiver, expected_type) = match (self, expected) {
            (Self::Absent, None) => return Ok(None),
            (Self::Absent, Some(expected)) => {
                return Err(TemplateReceiverSemanticValidationError::Missing {
                    expected: Box::new(expected.clone()),
                });
            }
            (Self::Present(receiver), None) => {
                return Err(TemplateReceiverSemanticValidationError::Unexpected {
                    actual: Box::new(receiver.value_type().clone()),
                });
            }
            (Self::Present(receiver), Some(expected)) => (receiver, expected),
        };

        let local = locals.get(receiver.local()).ok_or_else(|| {
            TemplateReceiverSemanticValidationError::MissingLocal(receiver.local().clone())
        })?;
        if local.mutable() != CanonicalBooleanV1::False {
            return Err(TemplateReceiverSemanticValidationError::MutableLocal);
        }
        if local.value_type() != receiver.value_type() {
            return Err(TemplateReceiverSemanticValidationError::LocalType {
                expected: Box::new(receiver.value_type().clone()),
                actual: Box::new(local.value_type().clone()),
            });
        }
        Ok(Some((receiver, expected_type)))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TemplateReceiverSemanticValidationError {
    Missing {
        expected: Box<SignatureTypeKey>,
    },
    Unexpected {
        actual: Box<SignatureTypeKey>,
    },
    MissingLocal(LocalValueSelector),
    MutableLocal,
    LocalType {
        expected: Box<SignatureTypeKey>,
        actual: Box<SignatureTypeKey>,
    },
    TypeSubstitution(DefaultTemplateTypeSubstitutionError),
    CallableType {
        expected: Box<SignatureTypeKey>,
        actual: Box<SignatureTypeKey>,
    },
}

impl fmt::Display for TemplateReceiverSemanticValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { expected } => write!(
                formatter,
                "default template is missing callable receiver of type {expected:?}"
            ),
            Self::Unexpected { actual } => write!(
                formatter,
                "default template has unexpected receiver of type {actual:?}"
            ),
            Self::MissingLocal(selector) => write!(
                formatter,
                "default-template receiver local {selector:?} is absent from the local table"
            ),
            Self::MutableLocal => {
                formatter.write_str("default-template receiver local must be immutable")
            }
            Self::LocalType { expected, actual } => write!(
                formatter,
                "default-template receiver local has type {actual:?}, expected {expected:?}"
            ),
            Self::TypeSubstitution(error) => write!(
                formatter,
                "cannot map default-template receiver type into owner scope: {error}"
            ),
            Self::CallableType { expected, actual } => write!(
                formatter,
                "default-template receiver has type {actual:?}, checked source contract requires {expected:?}"
            ),
        }
    }
}

impl std::error::Error for TemplateReceiverSemanticValidationError {}

#[cfg(test)]
mod tests;
