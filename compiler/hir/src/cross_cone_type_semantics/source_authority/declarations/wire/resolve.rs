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
        meter: &mut BudgetMeter,
    ) -> Result<TypeDeclarationSourceAuthorityV1, TypeDeclarationSourceResolutionError<E>> {
        use TypeDeclarationSourceResolutionError as Error;
        let path = WirePath::root();
        meter
            .check_semantic_depth(1, &path)
            .map_err(Error::Resource)?;
        meter.charge_nodes(1, &path).map_err(Error::Resource)?;
        meter.charge_work(9, &path).map_err(Error::Resource)?;
        self.required_protected
            .charge_resolution_at(meter, &path.clone().field(1))
            .map_err(Error::Resource)?;
        Ok(TypeDeclarationSourceAuthorityV1::new(
            TypeDeclarationSourceEntriesV1 {
                required_protected: self
                    .required_protected
                    .resolve(resolver, meter)
                    .map_err(Error::Required)?,
                nominals: self
                    .nominals
                    .resolve(resolver, meter)
                    .map_err(|e| Error::inventory(2, e))?,
                constructors: self
                    .constructors
                    .resolve(resolver, meter)
                    .map_err(Error::Constructors)?,
                properties: self
                    .properties
                    .resolve(resolver, meter)
                    .map_err(Error::Properties)?,
                callables: self
                    .callables
                    .resolve(resolver, meter)
                    .map_err(Error::Callables)?,
                inheritance: self
                    .inheritance
                    .resolve(resolver, meter)
                    .map_err(|e| Error::inventory(6, e))?,
                interfaces: self
                    .interfaces
                    .resolve(resolver, meter)
                    .map_err(|e| Error::inventory(7, e))?,
                selections: self
                    .selections
                    .resolve(resolver, meter)
                    .map_err(|e| Error::inventory(8, e))?,
                dispatch_callables: self
                    .dispatch_callables
                    .resolve(resolver, meter)
                    .map_err(Error::Dispatch)?,
            },
        ))
    }
}
