use std::fmt;

use scoop_identity::{LocalValueSelector, StructuralDefinitionPath, StructuralDefinitionSiteRole};

use super::CanonicalTemplateLocalTableV1;

impl CanonicalTemplateLocalTableV1 {
    pub fn validate_definition_path(
        &self,
        definition_path: &StructuralDefinitionPath,
    ) -> Result<(), TemplateLocalScopeValidationError> {
        let Some(last) = definition_path.segments().last() else {
            return Err(TemplateLocalScopeValidationError::EmptyDefinitionPath);
        };
        if last.site_role() != StructuralDefinitionSiteRole::DefaultValue {
            return Err(
                TemplateLocalScopeValidationError::InvalidDefinitionPathRole {
                    actual: last.site_role(),
                },
            );
        }

        for (index, record) in self.records().iter().enumerate() {
            let Some(path) = selector_path(record.selector()) else {
                continue;
            };
            let definition_segments = definition_path.segments();
            let local_segments = path.segments();
            if local_segments.len() <= definition_segments.len()
                || !local_segments.starts_with(definition_segments)
            {
                return Err(
                    TemplateLocalScopeValidationError::LocalOutsideDefinitionPath {
                        index,
                        selector: record.selector().clone(),
                    },
                );
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TemplateLocalScopeValidationError {
    EmptyDefinitionPath,
    InvalidDefinitionPathRole {
        actual: StructuralDefinitionSiteRole,
    },
    LocalOutsideDefinitionPath {
        index: usize,
        selector: LocalValueSelector,
    },
}

impl fmt::Display for TemplateLocalScopeValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyDefinitionPath => {
                formatter.write_str("default-template definition path is empty")
            }
            Self::InvalidDefinitionPathRole { actual } => write!(
                formatter,
                "default-template definition path ends in {actual:?}, expected DefaultValue"
            ),
            Self::LocalOutsideDefinitionPath { index, selector } => write!(
                formatter,
                "default-template local {selector:?} at index {index} is not below the definition path"
            ),
        }
    }
}

impl std::error::Error for TemplateLocalScopeValidationError {}

fn selector_path(selector: &LocalValueSelector) -> Option<&StructuralDefinitionPath> {
    match selector {
        LocalValueSelector::This | LocalValueSelector::Parameter { .. } => None,
        LocalValueSelector::LocalDeclaration { path }
        | LocalValueSelector::BoundReceiver { path }
        | LocalValueSelector::Synthetic { path, .. } => Some(path),
        LocalValueSelector::SuspensionResult { site } => Some(site),
    }
}

#[cfg(test)]
mod tests;
