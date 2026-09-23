use scoop_identity::{LocalValueSelector, StructuralPathSegment};
use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::DefaultTemplateContractViewV1;
use crate::{
    DefaultBodyProviderEnvelopeSemanticValidationError, DefaultLocalFunctionSignatureAuthority,
    DefaultTemplateProviderShapeV1, MeteredSignatureTypeSemanticError,
    NominalInterfaceShapeAuthority, TemplateLocalScopeValidationError,
};

impl DefaultTemplateContractViewV1<'_> {
    /// Checks local paths and every provider-scoped body type after origins and
    /// declaration contracts have been validated. It grants no execution rights.
    pub fn validate_provider_types<A, E>(
        &self,
        provider: DefaultTemplateProviderShapeV1,
        authority: &mut A,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), DefaultTemplateSourceEnvelopeError<E>>
    where
        A: NominalInterfaceShapeAuthority<E> + DefaultLocalFunctionSignatureAuthority<E>,
    {
        let definition_len = self.definition_path.segments().len() as u64;
        meter.charge_work(definition_len, path)?;
        for local in self.locals.records() {
            let length = match local.selector() {
                LocalValueSelector::This | LocalValueSelector::Parameter { .. } => 0,
                LocalValueSelector::LocalDeclaration { path }
                | LocalValueSelector::BoundReceiver { path }
                | LocalValueSelector::Synthetic { path, .. }
                | LocalValueSelector::SuspensionResult { site: path } => {
                    path.segments().len() as u64
                }
            };
            meter.charge_work(
                length.saturating_add(definition_len).saturating_add(1),
                path,
            )?;
            // Reserve the existing scope diagnostic's owned selector before checking.
            meter.charge_nodes(length.saturating_add(1), path)?;
            meter.charge_collection_slots(length, path)?;
            let bytes = length.saturating_mul(std::mem::size_of::<StructuralPathSegment>() as u64);
            meter.charge_owned_bytes(bytes, path)?;
        }
        self.locals
            .validate_definition_path(self.definition_path)
            .map_err(DefaultTemplateSourceEnvelopeError::LocalScope)?;
        let scope = provider.signature_scope();
        for (index, local) in self.locals.records().iter().enumerate() {
            scope
                .validate_signature_semantics_metered(local.value_type(), authority, meter, path)
                .map_err(|error| DefaultTemplateSourceEnvelopeError::LocalType {
                    index,
                    error: Box::new(error),
                })?;
        }
        self.body
            .validate_provider_types_semantics(provider, authority, meter, path)
            .map_err(|error| DefaultTemplateSourceEnvelopeError::Body(Box::new(error)))
    }
}

#[derive(Debug)]
pub enum DefaultTemplateSourceEnvelopeError<E> {
    Resource(WireError),
    LocalScope(TemplateLocalScopeValidationError),
    LocalType {
        index: usize,
        error: Box<MeteredSignatureTypeSemanticError<E>>,
    },
    Body(Box<DefaultBodyProviderEnvelopeSemanticValidationError<E>>),
}
impl<E> From<WireError> for DefaultTemplateSourceEnvelopeError<E> {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl<E: std::fmt::Display> std::fmt::Display for DefaultTemplateSourceEnvelopeError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::LocalScope(error) => error.fmt(f),
            Self::LocalType { index, error } => write!(
                f,
                "default local[{index}] has an invalid provider type: {error}"
            ),
            Self::Body(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for DefaultTemplateSourceEnvelopeError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::LocalScope(error) => Some(error),
            Self::LocalType { error, .. } => Some(error.as_ref()),
            Self::Body(error) => Some(error.as_ref()),
        }
    }
}
