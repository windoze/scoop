use std::fmt;

use scoop_identity::{CallableTemplateOrigin, LocalValueSelector, SignatureTypeKey};

use super::CanonicalTemplateValueParametersV1;
use crate::{
    CallableSourceInterfaceV1, CanonicalBooleanV1, CanonicalTemplateLocalTableV1,
    ExportDefaultTemplateKeyV1,
};

impl CanonicalTemplateValueParametersV1 {
    pub fn validate_semantics(
        &self,
        key: ExportDefaultTemplateKeyV1,
        source: &CallableSourceInterfaceV1,
        locals: &CanonicalTemplateLocalTableV1,
    ) -> Result<(), TemplateValueParameterSemanticValidationError> {
        if source.owner() != key.owner() {
            return Err(TemplateValueParameterSemanticValidationError::SourceOwner {
                expected: key.owner(),
                actual: source.owner(),
            });
        }

        let position = key.parameter_position();
        let source_parameters = source.parameters().parameters();
        let Ok(position_index) = usize::try_from(position) else {
            return Err(
                TemplateValueParameterSemanticValidationError::ParameterOutOfRange {
                    position,
                    arity: source.parameters().len_u32(),
                },
            );
        };
        let Some(current) = source_parameters.get(position_index) else {
            return Err(
                TemplateValueParameterSemanticValidationError::ParameterOutOfRange {
                    position,
                    arity: source.parameters().len_u32(),
                },
            );
        };
        let actual_template = current.calling().template();
        if actual_template != Some(key) {
            return Err(
                TemplateValueParameterSemanticValidationError::ParameterTemplate {
                    position,
                    expected: key,
                    actual: actual_template,
                },
            );
        }
        if self.len_u32() != position {
            return Err(TemplateValueParameterSemanticValidationError::PrefixArity {
                expected: position,
                actual: self.len_u32(),
            });
        }

        for (parameter, source_parameter) in self.parameters().iter().zip(source_parameters) {
            let position = parameter.position();
            let local = locals.get(parameter.local()).ok_or_else(|| {
                TemplateValueParameterSemanticValidationError::MissingLocal {
                    position,
                    selector: parameter.local().clone(),
                }
            })?;
            if local.mutable() != CanonicalBooleanV1::False {
                return Err(
                    TemplateValueParameterSemanticValidationError::MutableLocal { position },
                );
            }
            if local.value_type() != source_parameter.value_type() {
                return Err(TemplateValueParameterSemanticValidationError::LocalType {
                    position,
                    expected: Box::new(source_parameter.value_type().clone()),
                    actual: Box::new(local.value_type().clone()),
                });
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TemplateValueParameterSemanticValidationError {
    SourceOwner {
        expected: CallableTemplateOrigin,
        actual: CallableTemplateOrigin,
    },
    ParameterOutOfRange {
        position: u32,
        arity: u32,
    },
    ParameterTemplate {
        position: u32,
        expected: ExportDefaultTemplateKeyV1,
        actual: Option<ExportDefaultTemplateKeyV1>,
    },
    PrefixArity {
        expected: u32,
        actual: u32,
    },
    MissingLocal {
        position: u32,
        selector: LocalValueSelector,
    },
    MutableLocal {
        position: u32,
    },
    LocalType {
        position: u32,
        expected: Box<SignatureTypeKey>,
        actual: Box<SignatureTypeKey>,
    },
}

impl fmt::Display for TemplateValueParameterSemanticValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceOwner { expected, actual } => write!(
                formatter,
                "default-template owner {expected:?} does not match source interface {actual:?}"
            ),
            Self::ParameterOutOfRange { position, arity } => write!(
                formatter,
                "default-template parameter position {position} is out of range for source arity {arity}"
            ),
            Self::ParameterTemplate {
                position,
                expected,
                actual,
            } => write!(
                formatter,
                "source parameter {position} references default template {actual:?}, expected {expected:?}"
            ),
            Self::PrefixArity { expected, actual } => write!(
                formatter,
                "default template has {actual} preceding value parameters, expected {expected}"
            ),
            Self::MissingLocal { position, selector } => write!(
                formatter,
                "default-template value parameter {position} local {selector:?} is absent from the local table"
            ),
            Self::MutableLocal { position } => write!(
                formatter,
                "default-template value parameter {position} local must be immutable"
            ),
            Self::LocalType {
                position,
                expected,
                actual,
            } => write!(
                formatter,
                "default-template value parameter {position} local has type {actual:?}, expected {expected:?}"
            ),
        }
    }
}

impl std::error::Error for TemplateValueParameterSemanticValidationError {}

#[cfg(test)]
mod tests;
