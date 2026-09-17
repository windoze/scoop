use std::fmt;

use scoop_identity::{LocalValueSelector, SignatureTypeKey};

use super::OptionalTemplateReceiverV1;
use crate::{CallableInterfaceRecordV1, CanonicalBooleanV1, CanonicalTemplateLocalTableV1};

impl OptionalTemplateReceiverV1 {
    pub fn validate_semantics(
        &self,
        callable: &CallableInterfaceRecordV1,
        locals: &CanonicalTemplateLocalTableV1,
    ) -> Result<(), TemplateReceiverSemanticValidationError> {
        let (receiver, expected_type) = match (self, callable.receiver()) {
            (Self::Absent, None) => return Ok(()),
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
        if receiver.value_type() != expected_type {
            return Err(TemplateReceiverSemanticValidationError::CallableType {
                expected: Box::new(expected_type.clone()),
                actual: Box::new(receiver.value_type().clone()),
            });
        }
        Ok(())
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
            Self::CallableType { expected, actual } => write!(
                formatter,
                "default-template receiver has type {actual:?}, callable interface requires {expected:?}"
            ),
        }
    }
}

impl std::error::Error for TemplateReceiverSemanticValidationError {}

#[cfg(test)]
mod tests;
