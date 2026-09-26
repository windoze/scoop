use std::fmt;

use scoop_identity::CallableTemplateOrigin;

use super::CanonicalExportDefaultTemplatesV1;
use crate::{CanonicalCallableSourceInterfacesV1, ExportDefaultTemplateKeyV1};

impl CanonicalExportDefaultTemplatesV1 {
    pub fn validate_source_closure(
        &self,
        sources: &CanonicalCallableSourceInterfacesV1,
    ) -> Result<(), ExportDefaultTemplateSourceClosureValidationError> {
        for (template_index, template) in self.records().iter().enumerate() {
            let key = template.key();
            let source = sources.get(key.owner()).ok_or(
                ExportDefaultTemplateSourceClosureValidationError::MissingSourceInterface {
                    template_index,
                    owner: key.owner(),
                },
            )?;
            let Some(parameter) = usize::try_from(key.parameter_position())
                .ok()
                .and_then(|position| source.parameters().parameters().get(position))
            else {
                return Err(
                    ExportDefaultTemplateSourceClosureValidationError::ParameterOutOfRange {
                        template_index,
                        key,
                        arity: source.parameters().len_u32(),
                    },
                );
            };
            let actual = parameter.calling().template();
            if actual != Some(key) {
                return Err(
                    ExportDefaultTemplateSourceClosureValidationError::ParameterReference {
                        template_index,
                        key,
                        actual,
                    },
                );
            }
        }

        for (source_index, source) in sources.records().iter().enumerate() {
            for (parameter_position, parameter) in (0_u32..).zip(source.parameters().parameters()) {
                let Some(key) = parameter.calling().template() else {
                    continue;
                };
                if self.get(key).is_none() {
                    return Err(
                        ExportDefaultTemplateSourceClosureValidationError::MissingTemplate {
                            source_index,
                            owner: source.owner(),
                            parameter_position,
                            key,
                        },
                    );
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExportDefaultTemplateSourceClosureValidationError {
    MissingSourceInterface {
        template_index: usize,
        owner: CallableTemplateOrigin,
    },
    ParameterOutOfRange {
        template_index: usize,
        key: ExportDefaultTemplateKeyV1,
        arity: u32,
    },
    ParameterReference {
        template_index: usize,
        key: ExportDefaultTemplateKeyV1,
        actual: Option<ExportDefaultTemplateKeyV1>,
    },
    MissingTemplate {
        source_index: usize,
        owner: CallableTemplateOrigin,
        parameter_position: u32,
        key: ExportDefaultTemplateKeyV1,
    },
}

impl fmt::Display for ExportDefaultTemplateSourceClosureValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSourceInterface {
                template_index,
                owner,
            } => write!(
                formatter,
                "export default template {template_index} has no source interface for owner {owner:?}"
            ),
            Self::ParameterOutOfRange {
                template_index,
                key,
                arity,
            } => write!(
                formatter,
                "export default template {template_index} key {key:?} is outside source arity {arity}"
            ),
            Self::ParameterReference {
                template_index,
                key,
                actual,
            } => write!(
                formatter,
                "export default template {template_index} key {key:?} is referenced as {actual:?} by its source parameter"
            ),
            Self::MissingTemplate {
                source_index,
                owner,
                parameter_position,
                key,
            } => write!(
                formatter,
                "source interface {source_index} owner {owner:?} parameter {parameter_position} references missing export default template {key:?}"
            ),
        }
    }
}

impl std::error::Error for ExportDefaultTemplateSourceClosureValidationError {}

#[cfg(test)]
mod tests;
