use std::fmt;

use scoop_identity::ConeIdentity;

use super::ExportDefinitionSourceV1;

/// Supplies foundation source/context/point facts for inline definition
/// origins in the cross-Cone interface.
pub trait ExportDefinitionSourceSemanticAuthority<E> {
    fn current_cone(&self) -> ConeIdentity;

    fn validate_export_definition_source(
        &mut self,
        source: &ExportDefinitionSourceV1,
    ) -> Result<(), E>;
}

impl ExportDefinitionSourceV1 {
    pub fn validate_semantics<A, E>(
        &self,
        authority: &mut A,
    ) -> Result<(), ExportDefinitionSourceSemanticValidationError<E>>
    where
        A: ExportDefinitionSourceSemanticAuthority<E>,
    {
        let expected = authority.current_cone();
        let actual = self.origin().source().cone();
        if actual != expected {
            return Err(ExportDefinitionSourceSemanticValidationError::Cone { expected, actual });
        }
        authority
            .validate_export_definition_source(self)
            .map_err(ExportDefinitionSourceSemanticValidationError::Foundation)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum ExportDefinitionSourceSemanticValidationError<E> {
    Cone {
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    Foundation(E),
}

impl<E: fmt::Display> fmt::Display for ExportDefinitionSourceSemanticValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cone { expected, actual } => write!(
                formatter,
                "definition source belongs to Cone {actual}, expected current Cone {expected}"
            ),
            Self::Foundation(error) => {
                write!(formatter, "invalid foundation definition source: {error}")
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for ExportDefinitionSourceSemanticValidationError<E>
{
}

#[cfg(test)]
mod tests;
