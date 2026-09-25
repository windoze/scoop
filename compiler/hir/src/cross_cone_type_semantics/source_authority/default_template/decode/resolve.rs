use super::super::{DefaultSourceTemplateResolutionError, DefaultSourceTemplateV1};
use super::DecodedDefaultSourceTemplateV1;
use crate::{DefaultSourceReferenceResolver, DefaultStatementReferenceResolver};

impl DecodedDefaultSourceTemplateV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<DefaultSourceTemplateV1, DefaultSourceTemplateResolutionError<E>>
    where
        R: DefaultStatementReferenceResolver<E> + DefaultSourceReferenceResolver<E>,
    {
        use DefaultSourceTemplateResolutionError as Error;

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
        let template = DefaultSourceTemplateV1 {
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
        };
        template.validate_shape().map_err(Error::Record)?;
        Ok(template)
    }
}
