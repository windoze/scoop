use std::fmt;

use scoop_identity::{LocalValueSelector, StructuralDefinitionPath, StructuralDefinitionSiteRole};

use super::CanonicalTemplateLocalTableV1;
use crate::{
    DefaultTemplateProviderShapeV1, NominalInterfaceShapeAuthority, SignatureTypeSemanticError,
};

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

    pub fn validate_type_semantics<A, E>(
        &self,
        provider: DefaultTemplateProviderShapeV1,
        authority: &mut A,
    ) -> Result<(), TemplateLocalTypeSemanticValidationError<E>>
    where
        A: NominalInterfaceShapeAuthority<E>,
    {
        let scope = provider.signature_scope();
        for (index, record) in self.records().iter().enumerate() {
            scope
                .validate_signature_semantics(record.value_type(), authority)
                .map_err(|error| TemplateLocalTypeSemanticValidationError {
                    index,
                    selector: record.selector().clone(),
                    error,
                })?;
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

#[derive(Debug, Eq, PartialEq)]
pub struct TemplateLocalTypeSemanticValidationError<E> {
    pub index: usize,
    pub selector: LocalValueSelector,
    pub error: SignatureTypeSemanticError<E>,
}

impl<E: fmt::Display> fmt::Display for TemplateLocalTypeSemanticValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "default-template local {:?} at index {} has an invalid provider-scope type: {}",
            self.selector, self.index, self.error
        )
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for TemplateLocalTypeSemanticValidationError<E>
{
}

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
