use super::*;
use crate::{
    DeclarationAccessSourceResolutionError, InheritanceCallableSignatureResolutionError,
    PersistentAccessResolver,
};
use scoop_identity::{
    PersistentFunctionId, PersistentIdResolver, PersistentKeyResolver,
    PersistentPropertyAccessorId, PersistentSourceContextId, SourceContextKey,
};

pub trait InheritanceSourceCallableResolver<E>:
    PersistentAccessResolver<E>
    + PersistentIdResolver<PersistentFunctionId, Error = E>
    + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
    + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>
{
}
impl<R, E> InheritanceSourceCallableResolver<E> for R where
    R: PersistentAccessResolver<E>
        + PersistentIdResolver<PersistentFunctionId, Error = E>
        + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
        + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>
{
}

impl DecodedCanonicalInheritanceSourceCallablesV1 {
    pub fn resolve<R: InheritanceSourceCallableResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<CanonicalInheritanceSourceCallablesV1, InheritanceSourceCallableResolutionError<E>>
    {
        use InheritanceSourceCallableResolutionError as Error;
        let mut records = reserve(self.records.len(), meter).map_err(Error::Inventory)?;
        for (index, source) in self.records.into_iter().enumerate() {
            let path = WirePath::root().index(index as u64);
            meter
                .check_semantic_depth(3, &path)
                .map_err(Error::Resource)?;
            meter.charge_edges(4, &path).map_err(Error::Resource)?;
            source
                .declaration_access
                .charge_resolution_at(meter, &path.clone().field(4), 3)
                .map_err(Error::Resource)?;
            let signature = source
                .signature
                .resolve(resolver, meter)
                .map_err(Error::Signature)?;
            let declaration = source
                .declaration
                .resolve(resolver)
                .map_err(Error::Identity)?;
            let access = source
                .declaration_access
                .resolve(resolver)
                .map_err(Error::Access)?;
            records.push(InheritanceSourceCallableV1::new(
                declaration,
                signature,
                source.modality,
                access,
            ));
        }
        CanonicalInheritanceSourceCallablesV1::from_ordered(records, meter)
            .map_err(Error::Inventory)
    }
}

#[derive(Debug)]
pub enum InheritanceSourceCallableResolutionError<E> {
    Resource(WireError),
    Inventory(SourceInventoryError),
    Identity(E),
    Signature(InheritanceCallableSignatureResolutionError<E>),
    Access(DeclarationAccessSourceResolutionError<E>),
}
impl<E: fmt::Display> fmt::Display for InheritanceSourceCallableResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Inventory(error) => error.fmt(f),
            Self::Identity(error) => error.fmt(f),
            Self::Signature(error) => error.fmt(f),
            Self::Access(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error
    for InheritanceSourceCallableResolutionError<E>
{
}
