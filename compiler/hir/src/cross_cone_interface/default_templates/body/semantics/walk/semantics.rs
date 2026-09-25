use super::*;
use crate::SignatureTypeSemanticError;

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
    A: NominalInterfaceShapeAuthority<E> + DefaultLocalFunctionSignatureAuthority<E>,
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

        _path: &WirePath,
    ) -> Result<(), Self::Error> {
        match self
            .scope
            .validate_signature_semantics(signature, self.authority)
        {
            Err(SignatureTypeSemanticError::Allocation(error)) => {
                Err(DefaultBodyProviderEnvelopeSemanticValidationError::Resource(error))
            }
            Ok(()) => Ok(()),
            Err(error) => Err(DefaultBodyProviderEnvelopeSemanticValidationError::Type {
                site,
                definition_origin: Box::new(definition_origin.clone()),
                error: Box::new(error),
            }),
        }
    }

    fn validate_local_function_signature(
        &mut self,
        function: &DefaultLocalFunctionV1,
        definition_origin: &ExportDefinitionSourceV1,

        path: &WirePath,
    ) -> Result<(), Self::Error> {
        let own_arity = self
            .authority
            .default_local_function_own_binder_arity(function.declaration())
            .map_err(|error| Self::Error::LocalFunctionBinders {
                declaration: function.declaration(),
                definition_origin: Box::new(definition_origin.clone()),
                error,
            })?;
        let scope = self
            .scope
            .with_inner_frame(own_arity, path)
            .map_err(Self::resource)?;
        match scope.validate_signature_semantics(function.function_type(), self.authority) {
            Err(SignatureTypeSemanticError::Allocation(error)) => Err(Self::resource(error)),
            Ok(()) => Ok(()),
            Err(error) => Err(Self::Error::Type {
                site: DefaultBodyProviderTypeSiteV1::NestedCallableFunction,
                definition_origin: Box::new(definition_origin.clone()),
                error: Box::new(error),
            }),
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

        _path: &WirePath,
    ) -> Result<(), Self::Error> {
        (self.origin)(self.authority, source, site)
    }
}
