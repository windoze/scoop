mod contract;
mod receiver;
pub use contract::ExportDefaultTemplateContractSemanticValidationError;

use std::fmt;

use scoop_identity::{
    CallableTemplateOrigin, Effect, LocalValueSelector, SignatureTypeKey, StructuralDefinitionPath,
};

use super::ExportDefaultTemplateV1;
use crate::{
    BinderUseListSemanticValidationError, CallableInterfaceRecordV1,
    CallableInterfaceSemanticAuthority, CallableInterfaceSemanticValidationError,
    CallableSourceInterfaceV1, CanonicalBooleanV1, DefaultTemplateProviderShapeV1,
    DefaultTemplateRootSemanticAuthority, DefaultTemplateRootSemanticValidationError,
    DefaultTemplateTypeSubstitutionError, ExportDefaultTemplateKeyV1,
    ExportDefinitionSourceSemanticAuthority, ExportDefinitionSourceSemanticValidationError,
    ExportDefinitionSourceV1, PersistentLexicalRootV1, SignatureTypeSemanticError,
    TemplateLocalDefinitionV1, TemplateLocalScopeValidationError,
    TemplateLocalTypeSemanticValidationError, TemplateReceiverSemanticValidationError,
    TemplateValueParameterSemanticValidationError,
};

/// Supplies foundation subject relations for definition origins owned by one
/// exported default template.
pub trait DefaultTemplateOriginSemanticAuthority<E>:
    ExportDefinitionSourceSemanticAuthority<E>
{
    fn validate_default_template_origin(
        &mut self,
        key: ExportDefaultTemplateKeyV1,
        root: PersistentLexicalRootV1,
        path: &StructuralDefinitionPath,
        origin: &ExportDefinitionSourceV1,
    ) -> Result<(), E>;

    fn validate_default_template_local_origin(
        &mut self,
        key: ExportDefaultTemplateKeyV1,
        root: PersistentLexicalRootV1,
        path: &StructuralDefinitionPath,
        selector: &LocalValueSelector,
        origin: &ExportDefinitionSourceV1,
    ) -> Result<(), E>;
}

impl ExportDefaultTemplateV1 {
    /// Validates the template-level origin and source-backed local origins.
    /// Body-node and reference-record origins are validated by their own
    /// recursive semantic passes.
    pub fn validate_origin_semantics<A, E>(
        &self,
        authority: &mut A,
    ) -> Result<(), ExportDefaultTemplateOriginSemanticValidationError<E>>
    where
        A: DefaultTemplateOriginSemanticAuthority<E>,
    {
        self.definition_origin()
            .validate_semantics(authority)
            .map_err(ExportDefaultTemplateOriginSemanticValidationError::DefinitionSource)?;
        authority
            .validate_default_template_origin(
                self.key(),
                self.definition_root(),
                self.definition_path(),
                self.definition_origin(),
            )
            .map_err(ExportDefaultTemplateOriginSemanticValidationError::DefinitionRelation)?;

        for (index, local) in self.locals().records().iter().enumerate() {
            let TemplateLocalDefinitionV1::Source(origin) = local.definition() else {
                continue;
            };
            origin.validate_semantics(authority).map_err(|error| {
                ExportDefaultTemplateOriginSemanticValidationError::LocalSource {
                    index,
                    selector: local.selector().clone(),
                    error,
                }
            })?;
            authority
                .validate_default_template_local_origin(
                    self.key(),
                    self.definition_root(),
                    self.definition_path(),
                    local.selector(),
                    origin,
                )
                .map_err(|error| {
                    ExportDefaultTemplateOriginSemanticValidationError::LocalRelation {
                        index,
                        selector: local.selector().clone(),
                        error,
                    }
                })?;
        }
        Ok(())
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum ExportDefaultTemplateOriginSemanticValidationError<E> {
    DefinitionSource(ExportDefinitionSourceSemanticValidationError<E>),
    DefinitionRelation(E),
    LocalSource {
        index: usize,
        selector: LocalValueSelector,
        error: ExportDefinitionSourceSemanticValidationError<E>,
    },
    LocalRelation {
        index: usize,
        selector: LocalValueSelector,
        error: E,
    },
}

impl<E: fmt::Display> fmt::Display for ExportDefaultTemplateOriginSemanticValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DefinitionSource(error) => {
                write!(
                    formatter,
                    "invalid default-template definition source: {error}"
                )
            }
            Self::DefinitionRelation(error) => write!(
                formatter,
                "default-template definition source does not match its foundation subject: {error}"
            ),
            Self::LocalSource {
                index,
                selector,
                error,
            } => write!(
                formatter,
                "invalid default-template local {selector:?} definition source at index {index}: {error}"
            ),
            Self::LocalRelation {
                index,
                selector,
                error,
            } => write!(
                formatter,
                "default-template local {selector:?} definition source at index {index} does not match its foundation subject: {error}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for ExportDefaultTemplateOriginSemanticValidationError<E>
{
}

#[cfg(test)]
mod tests;
