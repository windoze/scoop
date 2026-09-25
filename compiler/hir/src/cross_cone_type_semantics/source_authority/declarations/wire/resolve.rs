use super::*;
use std::fmt;

pub trait TypeDeclarationSourceResolver<E>:
    NestedSourceInterfaceResolver<E>
    + SourceInheritanceInventoryResolver<E>
    + InheritanceSourceCallableResolver<E>
{
}
impl<R, E> TypeDeclarationSourceResolver<E> for R where
    R: NestedSourceInterfaceResolver<E>
        + SourceInheritanceInventoryResolver<E>
        + InheritanceSourceCallableResolver<E>
{
}
impl DecodedTypeDeclarationSourceAuthorityV1 {
    pub fn resolve<R: TypeDeclarationSourceResolver<E>, E: fmt::Display>(
        self,
        resolver: &mut R,
    ) -> Result<TypeDeclarationSourceAuthorityV1, TypeDeclarationSourceResolutionError<E>> {
        use TypeDeclarationSourceResolutionError as Error;

        Ok(TypeDeclarationSourceAuthorityV1::new(
            TypeDeclarationSourceEntriesV1 {
                required_protected: self
                    .required_protected
                    .resolve(resolver)
                    .map_err(Error::Required)?,
                nominals: self
                    .nominals
                    .resolve(resolver)
                    .map_err(|e| Error::inventory(2, e))?,
                constructors: self
                    .constructors
                    .resolve(resolver)
                    .map_err(Error::Constructors)?,
                properties: self
                    .properties
                    .resolve(resolver)
                    .map_err(Error::Properties)?,
                callables: self.callables.resolve(resolver).map_err(Error::Callables)?,
                inheritance: self
                    .inheritance
                    .resolve(resolver)
                    .map_err(|e| Error::inventory(6, e))?,
                interfaces: self
                    .interfaces
                    .resolve(resolver)
                    .map_err(|e| Error::inventory(7, e))?,
                selections: self
                    .selections
                    .resolve(resolver)
                    .map_err(|e| Error::inventory(8, e))?,
                dispatch_callables: self
                    .dispatch_callables
                    .resolve(resolver)
                    .map_err(Error::Dispatch)?,
            },
        ))
    }
}
