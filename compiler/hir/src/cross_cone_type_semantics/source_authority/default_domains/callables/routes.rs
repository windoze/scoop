use super::*;

impl<'b, 's, 'a, 'f> DefaultSourceDomainsV1<'b, 's, 'a, 'f> {
    pub(super) fn callable_provider(
        &self,
        target: View<'_>,
        context: &Context<'_, '_>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<&'b Declarations<'s, 'a, 'f>, Error> {
        let declaration = target.source_declaration().map_err(Error::target)?;
        let provider = if let Declaration::Generated(id) = declaration {
            let source = nested::attached(context, meter, path)?;
            let actual = match source.descriptor().identity() {
                Nested::Lambda(id)
                | Nested::AnonymousFunction(id)
                | Nested::CallableReference(id) => id,
                Nested::LocalFunction(_) => return Err(Error::CallableDescriptor(declaration)),
            };
            if id != actual {
                return Err(Error::CallableDescriptor(declaration));
            }
            source.definition_origin().origin().source().cone()
        } else {
            declaration
                .source_provider(self.current.foundation.identities, meter, path)
                .map_err(Error::target)?
        };
        self.provider(provider, meter, path)
    }
}
