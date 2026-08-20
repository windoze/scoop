use std::fmt;

use crate::{
    ConeIdentity, GeneratedBridgeAtomId, GeneratedBridgeAtomKey, ObjectDefinitionIdentityError,
    ObjectDefinitionPlanId, OdrMemberId, PersistentCallableBodyId, PersistentDispatchSlotId,
    PersistentDispatchTableId, PersistentExactTypeId, PersistentIdResolver,
    PersistentImmortalObjectId, PersistentInitializationUnitId, PersistentKeyResolver,
    PersistentLayoutId, PersistentSafepointSiteId, PersistentScanId, PersistentStaticStorageId,
};

mod atom;
mod plan;

pub use atom::{DecodedDefinitionAtomSubkey, DecodedObjectDefinitionAtomKey};
pub use plan::{
    DecodedObjectDefinitionPlanKey, DecodedObjectDefinitionPlanOwner, DecodedStrongDefinitionEntity,
};

pub trait DefinitionAtomResolver<E>:
    PersistentIdResolver<ObjectDefinitionPlanId, Error = E>
    + PersistentIdResolver<PersistentCallableBodyId, Error = E>
    + PersistentIdResolver<PersistentStaticStorageId, Error = E>
    + PersistentIdResolver<PersistentImmortalObjectId, Error = E>
    + PersistentIdResolver<PersistentInitializationUnitId, Error = E>
    + PersistentIdResolver<PersistentExactTypeId, Error = E>
    + PersistentIdResolver<PersistentSafepointSiteId, Error = E>
{
}

impl<T, E> DefinitionAtomResolver<E> for T where
    T: PersistentIdResolver<ObjectDefinitionPlanId, Error = E>
        + PersistentIdResolver<PersistentCallableBodyId, Error = E>
        + PersistentIdResolver<PersistentStaticStorageId, Error = E>
        + PersistentIdResolver<PersistentImmortalObjectId, Error = E>
        + PersistentIdResolver<PersistentInitializationUnitId, Error = E>
        + PersistentIdResolver<PersistentExactTypeId, Error = E>
        + PersistentIdResolver<PersistentSafepointSiteId, Error = E>
{
}

pub trait StrongDefinitionResolver<E>:
    PersistentIdResolver<ConeIdentity, Error = E>
    + PersistentIdResolver<PersistentCallableBodyId, Error = E>
    + PersistentIdResolver<PersistentStaticStorageId, Error = E>
    + PersistentIdResolver<PersistentImmortalObjectId, Error = E>
    + PersistentIdResolver<PersistentExactTypeId, Error = E>
    + PersistentIdResolver<PersistentLayoutId, Error = E>
    + PersistentIdResolver<PersistentScanId, Error = E>
    + PersistentIdResolver<PersistentDispatchTableId, Error = E>
    + PersistentIdResolver<PersistentDispatchSlotId, Error = E>
    + PersistentIdResolver<PersistentInitializationUnitId, Error = E>
    + PersistentIdResolver<PersistentSafepointSiteId, Error = E>
    + PersistentKeyResolver<GeneratedBridgeAtomId, GeneratedBridgeAtomKey, Error = E>
    + PersistentIdResolver<OdrMemberId, Error = E>
{
}

impl<T, E> StrongDefinitionResolver<E> for T where
    T: PersistentIdResolver<ConeIdentity, Error = E>
        + PersistentIdResolver<PersistentCallableBodyId, Error = E>
        + PersistentIdResolver<PersistentStaticStorageId, Error = E>
        + PersistentIdResolver<PersistentImmortalObjectId, Error = E>
        + PersistentIdResolver<PersistentExactTypeId, Error = E>
        + PersistentIdResolver<PersistentLayoutId, Error = E>
        + PersistentIdResolver<PersistentScanId, Error = E>
        + PersistentIdResolver<PersistentDispatchTableId, Error = E>
        + PersistentIdResolver<PersistentDispatchSlotId, Error = E>
        + PersistentIdResolver<PersistentInitializationUnitId, Error = E>
        + PersistentIdResolver<PersistentSafepointSiteId, Error = E>
        + PersistentKeyResolver<GeneratedBridgeAtomId, GeneratedBridgeAtomKey, Error = E>
        + PersistentIdResolver<OdrMemberId, Error = E>
{
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ObjectDefinitionResolutionError<E> {
    Reference(E),
    Definition(ObjectDefinitionIdentityError),
}

impl<E: fmt::Display> fmt::Display for ObjectDefinitionResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(error) => error.fmt(formatter),
            Self::Definition(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for ObjectDefinitionResolutionError<E> {}

#[cfg(test)]
mod tests;
