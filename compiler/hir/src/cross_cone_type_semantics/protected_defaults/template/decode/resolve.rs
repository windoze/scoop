use super::super::{ProtectedDefaultTemplateResolutionError, ProtectedDefaultTemplateV1};
use super::DecodedProtectedDefaultTemplateV1;
use crate::{DefaultStatementReferenceResolver, ProtectedDefaultReferenceResolver};

impl DecodedProtectedDefaultTemplateV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<ProtectedDefaultTemplateV1, ProtectedDefaultTemplateResolutionError<E>>
    where
        R: DefaultStatementReferenceResolver<E> + ProtectedDefaultReferenceResolver<E>,
    {
        use ProtectedDefaultTemplateResolutionError as Error;

        let key = self.key.resolve(resolver).map_err(Error::Key)?;
        let definition_root = self
            .definition_root
            .resolve(resolver)
            .map_err(Error::DefinitionRoot)?;

        let mut locals = self.locals.resolve(resolver).map_err(Error::Locals)?;

        let body = self
            .body
            .resolve(resolver, &mut locals)
            .map_err(Error::Body)?;

        let result = self.result.resolve(resolver).map_err(Error::Result)?;

        let type_parameters = self
            .type_parameters
            .resolve(resolver)
            .map_err(Error::TypeParameters)?;

        let receiver = self
            .receiver
            .resolve(resolver, &mut locals)
            .map_err(Error::Receiver)?;

        let value_parameters = self
            .value_parameters
            .resolve(&mut locals)
            .map_err(Error::ValueParameters)?;
        let references = self
            .references
            .resolve(resolver)
            .map_err(Error::References)?;

        let definition_origin = self
            .definition_origin
            .resolve(resolver)
            .map_err(Error::DefinitionOrigin)?;
        ProtectedDefaultTemplateV1 {
            key,
            definition_root,
            definition_path: self.definition_path,
            locals,
            body,
            result,
            allows_suspend: self.allows_suspend,
            type_parameters,
            receiver,
            value_parameters,
            references,
            definition_origin,
        }
        .finish()
        .map_err(ProtectedDefaultTemplateResolutionError::Record)
    }
}
