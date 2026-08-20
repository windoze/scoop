use std::fmt;

use crate::{
    CallableApplicationResolutionError, OdrGroupId, OdrMemberIdentityError,
    PersistentCallableApplicationId, PersistentCallableBodyId, PersistentConstructorId,
    PersistentDispatchSlotId, PersistentDispatchTableId, PersistentEnumVariantId,
    PersistentExactTypeId, PersistentExtensionPropertyId, PersistentFunctionId,
    PersistentGeneratedCallableId, PersistentGenericFunctionId, PersistentGenericTypeId,
    PersistentIdResolver, PersistentImmortalObjectId, PersistentInitializationUnitId,
    PersistentLayoutId, PersistentPropertyAccessorId, PersistentSafepointSiteId, PersistentScanId,
    PersistentStaticStorageId, PersistentTypeId,
};

mod member;
mod specialization;

pub use member::{DecodedOdrMemberDiscriminator, DecodedOdrMemberKey};
pub use specialization::DecodedSpecializationKey;

pub trait SpecializationResolver<E>:
    PersistentIdResolver<PersistentGenericTypeId, Error = E>
    + PersistentIdResolver<PersistentExactTypeId, Error = E>
    + PersistentIdResolver<PersistentFunctionId, Error = E>
    + PersistentIdResolver<PersistentGenericFunctionId, Error = E>
    + PersistentIdResolver<PersistentConstructorId, Error = E>
    + PersistentIdResolver<PersistentEnumVariantId, Error = E>
    + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
    + PersistentIdResolver<PersistentCallableApplicationId, Error = E>
    + PersistentIdResolver<PersistentInitializationUnitId, Error = E>
    + PersistentIdResolver<PersistentExtensionPropertyId, Error = E>
{
}

impl<T, E> SpecializationResolver<E> for T where
    T: PersistentIdResolver<PersistentGenericTypeId, Error = E>
        + PersistentIdResolver<PersistentExactTypeId, Error = E>
        + PersistentIdResolver<PersistentFunctionId, Error = E>
        + PersistentIdResolver<PersistentGenericFunctionId, Error = E>
        + PersistentIdResolver<PersistentConstructorId, Error = E>
        + PersistentIdResolver<PersistentEnumVariantId, Error = E>
        + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
        + PersistentIdResolver<PersistentCallableApplicationId, Error = E>
        + PersistentIdResolver<PersistentInitializationUnitId, Error = E>
        + PersistentIdResolver<PersistentExtensionPropertyId, Error = E>
{
}

pub trait OdrMemberResolver<E>:
    PersistentIdResolver<OdrGroupId, Error = E>
    + PersistentIdResolver<PersistentCallableApplicationId, Error = E>
    + PersistentIdResolver<PersistentGeneratedCallableId, Error = E>
    + PersistentIdResolver<PersistentTypeId, Error = E>
    + PersistentIdResolver<PersistentExactTypeId, Error = E>
    + PersistentIdResolver<PersistentLayoutId, Error = E>
    + PersistentIdResolver<PersistentScanId, Error = E>
    + PersistentIdResolver<PersistentDispatchTableId, Error = E>
    + PersistentIdResolver<PersistentDispatchSlotId, Error = E>
    + PersistentIdResolver<PersistentStaticStorageId, Error = E>
    + PersistentIdResolver<PersistentImmortalObjectId, Error = E>
    + PersistentIdResolver<PersistentInitializationUnitId, Error = E>
    + PersistentIdResolver<PersistentCallableBodyId, Error = E>
    + PersistentIdResolver<PersistentSafepointSiteId, Error = E>
{
}

impl<T, E> OdrMemberResolver<E> for T where
    T: PersistentIdResolver<OdrGroupId, Error = E>
        + PersistentIdResolver<PersistentCallableApplicationId, Error = E>
        + PersistentIdResolver<PersistentGeneratedCallableId, Error = E>
        + PersistentIdResolver<PersistentTypeId, Error = E>
        + PersistentIdResolver<PersistentExactTypeId, Error = E>
        + PersistentIdResolver<PersistentLayoutId, Error = E>
        + PersistentIdResolver<PersistentScanId, Error = E>
        + PersistentIdResolver<PersistentDispatchTableId, Error = E>
        + PersistentIdResolver<PersistentDispatchSlotId, Error = E>
        + PersistentIdResolver<PersistentStaticStorageId, Error = E>
        + PersistentIdResolver<PersistentImmortalObjectId, Error = E>
        + PersistentIdResolver<PersistentInitializationUnitId, Error = E>
        + PersistentIdResolver<PersistentCallableBodyId, Error = E>
        + PersistentIdResolver<PersistentSafepointSiteId, Error = E>
{
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OdrIdentityResolutionError<E> {
    Reference(E),
    Callable(CallableApplicationResolutionError<E>),
    Allocation,
    EmptyArguments,
    Member(OdrMemberIdentityError),
}

impl<E: fmt::Display> fmt::Display for OdrIdentityResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(error) => error.fmt(formatter),
            Self::Callable(error) => error.fmt(formatter),
            Self::Allocation => formatter.write_str("failed to allocate specialization arguments"),
            Self::EmptyArguments => {
                formatter.write_str("specialization arguments must not be empty")
            }
            Self::Member(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for OdrIdentityResolutionError<E> {}

#[cfg(test)]
mod tests;
