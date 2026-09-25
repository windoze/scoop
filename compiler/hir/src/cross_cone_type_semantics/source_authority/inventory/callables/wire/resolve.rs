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
    ) -> Result<CanonicalInheritanceSourceCallablesV1, InheritanceSourceCallableResolutionError<E>>
    {
        use InheritanceSourceCallableResolutionError as Error;
        let mut records = reserve(self.records.len()).map_err(Error::Inventory)?;
        for source in self.records.into_iter() {
            let signature = source
                .signature
                .resolve(resolver)
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
        CanonicalInheritanceSourceCallablesV1::from_ordered(records).map_err(Error::Inventory)
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
