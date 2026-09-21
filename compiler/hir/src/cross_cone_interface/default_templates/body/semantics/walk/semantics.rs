use super::*;
use crate::MeteredSignatureTypeSemanticError;

type OriginValidator<A, E> =
    fn(
        &mut A,
        &ExportDefinitionSourceV1,
        DefaultBodyOriginSiteV1,
    ) -> Result<(), DefaultBodyProviderEnvelopeSemanticValidationError<E>>;

pub(super) struct SemanticValidation<'a, A, E> {
    pub(super) scope: crate::SignatureBinderScopeV1,
    pub(super) authority: &'a mut A,
    pub(super) origin: OriginValidator<A, E>,
}

impl<A, E> BodyWalkMode for SemanticValidation<'_, A, E>
where
    A: NominalInterfaceShapeAuthority<E>,
{
    type Error = DefaultBodyProviderEnvelopeSemanticValidationError<E>;

    fn resource(error: WireError) -> Self::Error {
        DefaultBodyProviderEnvelopeSemanticValidationError::Resource(error)
    }

    fn validate_type(
        &mut self,
        signature: &SignatureTypeKey,
        site: DefaultBodyProviderTypeSiteV1,
        definition_origin: &ExportDefinitionSourceV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), Self::Error> {
        match self.scope.validate_signature_semantics_metered(
            signature,
            self.authority,
            meter,
            path,
        ) {
            Ok(()) => Ok(()),
            Err(MeteredSignatureTypeSemanticError::Semantic(error)) => {
                Err(DefaultBodyProviderEnvelopeSemanticValidationError::Type {
                    site,
                    definition_origin: Box::new(definition_origin.clone()),
                    error: Box::new(error),
                })
            }
            Err(MeteredSignatureTypeSemanticError::Resource(error)) => {
                Err(DefaultBodyProviderEnvelopeSemanticValidationError::Resource(error))
            }
        }
    }

    fn validate_binder(
        &mut self,
        depth: u32,
        index: u32,
        site: DefaultBodyProviderTypeSiteV1,
        definition_origin: &ExportDefinitionSourceV1,
    ) -> Result<(), Self::Error> {
        self.scope
            .validate(&SignatureTypeKey::Binder { depth, index })
            .map_err(
                |error| DefaultBodyProviderEnvelopeSemanticValidationError::Binder {
                    site,
                    definition_origin: Box::new(definition_origin.clone()),
                    error,
                },
            )
    }

    fn visit_origin(
        &mut self,
        source: &ExportDefinitionSourceV1,
        site: DefaultBodyOriginSiteV1,
        _meter: &mut BudgetMeter,
        _path: &WirePath,
    ) -> Result<(), Self::Error> {
        (self.origin)(self.authority, source, site)
    }
}
