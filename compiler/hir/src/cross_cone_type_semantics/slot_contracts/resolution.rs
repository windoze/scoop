use std::fmt;

use scoop_identity::{
    ConeIdentity, PersistentDispatchSlotId, PersistentExactTypeId, PersistentFunctionId,
    PersistentIdResolver, PersistentKeyResolver, PersistentPropertyAccessorId,
    PersistentSourceContextId, SourceContextKey,
};
use scoop_wire::WireError;

use super::*;
use crate::{DeclarationAccessSourceResolutionError, SourceNominalIdResolver};

pub trait InheritanceSlotResolver<E>:
    SourceNominalIdResolver<E>
    + PersistentIdResolver<ConeIdentity, Error = E>
    + PersistentIdResolver<PersistentExactTypeId, Error = E>
    + PersistentIdResolver<PersistentDispatchSlotId, Error = E>
    + PersistentIdResolver<PersistentFunctionId, Error = E>
    + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
    + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>
{
}
impl<R, E> InheritanceSlotResolver<E> for R where
    R: SourceNominalIdResolver<E>
        + PersistentIdResolver<ConeIdentity, Error = E>
        + PersistentIdResolver<PersistentExactTypeId, Error = E>
        + PersistentIdResolver<PersistentDispatchSlotId, Error = E>
        + PersistentIdResolver<PersistentFunctionId, Error = E>
        + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
        + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>
{
}

impl DecodedInheritanceSlotTargetV1 {
    pub fn resolve<R: InheritanceSlotResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<InheritanceSlotTargetV1, InheritanceSlotResolutionError<E>> {
        let declaration = self
            .declaration
            .resolve(resolver)
            .map_err(InheritanceSlotResolutionError::Identity)?;
        let owner = resolver
            .resolve(self.owner)
            .map_err(InheritanceSlotResolutionError::Identity)?;
        let signature = self
            .signature
            .resolve(resolver)
            .map_err(InheritanceSlotResolutionError::Signature)?;
        let access = self
            .declaration_access
            .resolve(resolver)
            .map_err(InheritanceSlotResolutionError::Source)?;
        Ok(InheritanceSlotTargetV1::new(
            declaration,
            owner,
            signature,
            self.modality,
            access,
        ))
    }
}
impl DecodedInheritanceSlotImplementationV1 {
    pub fn resolve<R: InheritanceSlotResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<InheritanceSlotImplementationV1, InheritanceSlotResolutionError<E>> {
        match self {
            Self::Abstract(target) => target
                .resolve(resolver)
                .map(InheritanceSlotImplementationV1::Abstract),
            Self::Concrete(target) => target
                .resolve(resolver)
                .map(InheritanceSlotImplementationV1::Concrete),
            Self::InterfaceDefault(target) => target
                .resolve(resolver)
                .map(InheritanceSlotImplementationV1::InterfaceDefault),
        }
    }
}
impl DecodedInheritanceSlotContractV1 {
    pub fn resolve<R: InheritanceSlotResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<InheritanceSlotContractV1, InheritanceSlotResolutionError<E>> {
        let slot = resolver
            .resolve(self.slot)
            .map_err(InheritanceSlotResolutionError::Identity)?;
        let owner = resolver
            .resolve(self.declaration_owner)
            .map_err(InheritanceSlotResolutionError::Identity)?;
        let declaration = self
            .declaration
            .resolve(resolver)
            .map_err(InheritanceSlotResolutionError::Identity)?;
        let signature = self
            .signature
            .resolve(resolver)
            .map_err(InheritanceSlotResolutionError::Signature)?;
        let implementation = self.implementation.resolve(resolver)?;
        let access = self
            .declaration_access
            .resolve(resolver)
            .map_err(InheritanceSlotResolutionError::Source)?;
        InheritanceSlotContractV1::try_new(
            slot,
            owner,
            declaration,
            signature,
            implementation,
            access,
        )
        .map_err(InheritanceSlotResolutionError::Contract)
    }
}

#[derive(Debug)]
pub enum InheritanceSlotResolutionError<E> {
    Resource(WireError),
    Identity(E),
    Signature(InheritanceCallableSignatureResolutionError<E>),
    Source(DeclarationAccessSourceResolutionError<E>),
    Contract(InheritanceSlotContractBuildError),
}
impl<E: fmt::Display> fmt::Display for InheritanceSlotResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Identity(error) => error.fmt(f),
            Self::Signature(error) => error.fmt(f),
            Self::Source(error) => error.fmt(f),
            Self::Contract(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for InheritanceSlotResolutionError<E> {}
