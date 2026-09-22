use super::*;
use scoop_identity::{PropertyAccessorKey, PropertyOwner};

impl<'b, 's, 'a, 'f> DefaultSourceDomainsV1<'b, 's, 'a, 'f> {
    pub(super) fn callable_provider(
        &self,
        target: View<'_>,
        context: &Context<'_, '_>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<&'b Declarations<'s, 'a, 'f>, Error> {
        let declaration = declaration(target)?;
        let provider = match declaration {
            Declaration::Function(id) => self.source_provider(id, meter, path)?,
            Declaration::GenericFunction(id) => self.source_provider(id, meter, path)?,
            Declaration::PropertyAccessor(id) => {
                let key = self.identity_key::<_, PropertyAccessorKey>(id, meter, path)?;
                match key.owner() {
                    PropertyOwner::Property(id) => self.source_provider(id, meter, path)?,
                    PropertyOwner::ExtensionProperty(id) => {
                        self.source_provider(id, meter, path)?
                    }
                }
            }
            Declaration::Generated(id) => {
                // The declaration transaction already bound this exact borrowed
                // descriptor's artifact key, role, parent, source and path.
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
            }
        };
        self.provider(provider, meter, path)
    }
}

pub(super) fn declaration(target: View<'_>) -> Result<Declaration, Error> {
    match target {
        View::Callable(callee) => Ok(callee.declaration()),
        View::FunctionAddress(declaration) => Ok(declaration),
        View::Bound(bound) => match bound.source() {
            DefaultBoundCallableSourceV1::Class { callable, .. } => Ok(callable.declaration()),
            DefaultBoundCallableSourceV1::Interface { member, .. } => local_declaration(*member),
        },
        View::LocalFunction(origin) => local_declaration(origin),
        View::Lambda(id) | View::AnonymousFunction(id) | View::CallableReference(id) => {
            Ok(Declaration::Generated(id))
        }
        View::DerivedEquality(_) => Err(Error::EqualityShape),
    }
}
