use super::ProtectedDefaultTemplateV1;
use crate::{
    ExportDefinitionSourceSemanticAuthority, ExportDefinitionSourceSemanticValidationError,
    ExportDefinitionSourceV1, PersistentLexicalRootV1, ProtectedDefaultTemplateKeyV1,
    TemplateLocalDefinitionV1,
};
use scoop_identity::{LocalValueSelector, StructuralDefinitionPath};
use scoop_wire::{BudgetMeter, WireError, WirePath};

/// Establishes source subject relations without converting protected keys into
/// the separate public-default namespace.
pub trait ProtectedDefaultOriginSemanticAuthority<E>:
    ExportDefinitionSourceSemanticAuthority<E>
{
    fn validate_protected_default_origin(
        &mut self,
        key: ProtectedDefaultTemplateKeyV1,
        root: PersistentLexicalRootV1,
        path: &StructuralDefinitionPath,
        origin: &ExportDefinitionSourceV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), E>;
    fn validate_protected_default_local_origin(
        &mut self,
        key: ProtectedDefaultTemplateKeyV1,
        root: PersistentLexicalRootV1,
        path: &StructuralDefinitionPath,
        selector: &LocalValueSelector,
        origin: &ExportDefinitionSourceV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), E>;
}
impl ProtectedDefaultTemplateV1 {
    /// Validates template and source-backed local origins. Complete body and
    /// reference origin replay remains the enclosing semantic pass's duty.
    pub fn validate_origin_semantics<A: ProtectedDefaultOriginSemanticAuthority<E>, E>(
        &self,
        authority: &mut A,
        meter: &mut BudgetMeter,
    ) -> Result<(), ProtectedDefaultTemplateOriginSemanticError<E>> {
        use ProtectedDefaultTemplateOriginSemanticError as Error;
        charge(self.definition_origin(), meter)?;
        self.definition_origin()
            .validate_semantics(authority)
            .map_err(Error::DefinitionSource)?;
        authority
            .validate_protected_default_origin(
                self.key(),
                self.definition_root(),
                self.definition_path(),
                self.definition_origin(),
                meter,
            )
            .map_err(Error::DefinitionRelation)?;
        for (index, local) in self.locals().records().iter().enumerate() {
            meter
                .charge_work(1, &WirePath::root())
                .map_err(Error::Resource)?;
            let TemplateLocalDefinitionV1::Source(origin) = local.definition() else {
                continue;
            };
            charge(origin, meter)?;
            origin
                .validate_semantics(authority)
                .map_err(|error| Error::LocalSource { index, error })?;
            authority
                .validate_protected_default_local_origin(
                    self.key(),
                    self.definition_root(),
                    self.definition_path(),
                    local.selector(),
                    origin,
                    meter,
                )
                .map_err(|error| Error::LocalRelation { index, error })?;
        }
        Ok(())
    }
}
fn charge<E>(
    source: &ExportDefinitionSourceV1,
    meter: &mut BudgetMeter,
) -> Result<(), ProtectedDefaultTemplateOriginSemanticError<E>> {
    use ProtectedDefaultTemplateOriginSemanticError as Error;
    meter
        .charge_work(
            scoop_wire::encoded_length(source).map_err(Error::Encoding)?,
            &WirePath::root(),
        )
        .map_err(Error::Resource)
}

#[derive(Debug)]
pub enum ProtectedDefaultTemplateOriginSemanticError<E> {
    Resource(WireError),
    Encoding(scoop_wire::cbor::EncodeError),
    DefinitionSource(ExportDefinitionSourceSemanticValidationError<E>),
    DefinitionRelation(E),
    LocalSource {
        index: usize,
        error: ExportDefinitionSourceSemanticValidationError<E>,
    },
    LocalRelation {
        index: usize,
        error: E,
    },
}
impl<E: std::fmt::Display> std::fmt::Display for ProtectedDefaultTemplateOriginSemanticError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Encoding(error) => error.fmt(f),
            Self::DefinitionSource(error) => error.fmt(f),
            Self::DefinitionRelation(error) => write!(
                f,
                "protected default origin does not match its source subject: {error}"
            ),
            Self::LocalSource { index, error } => write!(
                f,
                "protected default local {index} origin is invalid: {error}"
            ),
            Self::LocalRelation { index, error } => write!(
                f,
                "protected default local {index} origin does not match its source subject: {error}"
            ),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error
    for ProtectedDefaultTemplateOriginSemanticError<E>
{
}
