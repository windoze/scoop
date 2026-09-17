use std::fmt;

use scoop_identity::{
    CallableTemplateOrigin, StructuralDefinitionPath, StructuralDefinitionSiteRole,
};

use super::PersistentLexicalRootV1;
use crate::ExportDefaultTemplateKeyV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DefaultTemplateProviderShapeV1 {
    binder_arity: u32,
}

impl DefaultTemplateProviderShapeV1 {
    pub const fn new(binder_arity: u32) -> Self {
        Self { binder_arity }
    }

    pub const fn binder_arity(self) -> u32 {
        self.binder_arity
    }
}

/// Supplies source-declaration facts for a default template's true provider.
pub trait DefaultTemplateRootSemanticAuthority<E> {
    /// Verifies that `path` identifies one default expression under `root`
    /// and returns the provider's flattened binder arity.
    fn default_template_provider_shape(
        &mut self,
        root: PersistentLexicalRootV1,
        path: &StructuralDefinitionPath,
    ) -> Result<DefaultTemplateProviderShapeV1, E>;

    /// Verifies the unique override/default-source relation when the
    /// publishing owner differs from the source provider.
    fn validate_inherited_default_provider(
        &mut self,
        key: ExportDefaultTemplateKeyV1,
        root: PersistentLexicalRootV1,
        path: &StructuralDefinitionPath,
    ) -> Result<(), E>;
}

impl PersistentLexicalRootV1 {
    pub fn validate_semantics<A, E>(
        self,
        key: ExportDefaultTemplateKeyV1,
        definition_path: &StructuralDefinitionPath,
        authority: &mut A,
    ) -> Result<DefaultTemplateProviderShapeV1, DefaultTemplateRootSemanticValidationError<E>>
    where
        A: DefaultTemplateRootSemanticAuthority<E>,
    {
        if matches!(key.owner(), CallableTemplateOrigin::Accessor(_)) {
            return Err(DefaultTemplateRootSemanticValidationError::PropertyAccessorOwner);
        }
        let Some(last) = definition_path.segments().last() else {
            return Err(DefaultTemplateRootSemanticValidationError::EmptyDefinitionPath);
        };
        if last.site_role() != StructuralDefinitionSiteRole::DefaultValue {
            return Err(
                DefaultTemplateRootSemanticValidationError::InvalidDefinitionPathRole {
                    actual: last.site_role(),
                },
            );
        }

        let shape = authority
            .default_template_provider_shape(self, definition_path)
            .map_err(DefaultTemplateRootSemanticValidationError::Provider)?;
        if self.declaration() != key.owner() {
            authority
                .validate_inherited_default_provider(key, self, definition_path)
                .map_err(DefaultTemplateRootSemanticValidationError::InheritedRelation)?;
        }
        Ok(shape)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultTemplateRootSemanticValidationError<E> {
    PropertyAccessorOwner,
    EmptyDefinitionPath,
    InvalidDefinitionPathRole {
        actual: StructuralDefinitionSiteRole,
    },
    Provider(E),
    InheritedRelation(E),
}

impl<E: fmt::Display> fmt::Display for DefaultTemplateRootSemanticValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PropertyAccessorOwner => {
                formatter.write_str("property accessor cannot own a default template")
            }
            Self::EmptyDefinitionPath => {
                formatter.write_str("default-template definition path is empty")
            }
            Self::InvalidDefinitionPathRole { actual } => write!(
                formatter,
                "default-template definition path ends in {actual:?}, expected DefaultValue"
            ),
            Self::Provider(error) => {
                write!(formatter, "invalid default-template provider: {error}")
            }
            Self::InheritedRelation(error) => write!(
                formatter,
                "invalid inherited default-template provider relation: {error}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for DefaultTemplateRootSemanticValidationError<E>
{
}

#[cfg(test)]
mod tests;
