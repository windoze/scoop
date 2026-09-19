use std::fmt;

use scoop_identity::{
    PersistentDispatchSlotId, PersistentFunctionId, PersistentIdResolver, PersistentKeyResolver,
    PersistentPropertyAccessorId, PersistentSourceContextId, SourceContextKey,
};
use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::*;
use crate::{
    DeclarationAccessSourceResolutionError, PersistentAccessResolutionError,
    PersistentAccessResolver,
};

pub trait InheritanceSlotResolver<E>:
    PersistentAccessResolver<E>
    + PersistentIdResolver<PersistentDispatchSlotId, Error = E>
    + PersistentIdResolver<PersistentFunctionId, Error = E>
    + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
    + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>
{
}
impl<R, E> InheritanceSlotResolver<E> for R where
    R: PersistentAccessResolver<E>
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
        meter: &mut BudgetMeter,
    ) -> Result<InheritanceSlotTargetV1, InheritanceSlotResolutionError<E>> {
        meter
            .charge_nodes(1, &WirePath::root())
            .map_err(InheritanceSlotResolutionError::Resource)?;
        let declaration = self
            .declaration
            .resolve(resolver)
            .map_err(InheritanceSlotResolutionError::Identity)?;
        let owner = resolver
            .resolve(self.owner)
            .map_err(InheritanceSlotResolutionError::Identity)?;
        let signature = self
            .signature
            .resolve(resolver, meter)
            .map_err(InheritanceSlotResolutionError::Signature)?;
        let access = self
            .declaration_access
            .resolve_metered(resolver, meter)
            .map_err(InheritanceSlotResolutionError::Source)?;
        InheritanceSlotTargetV1::try_new(declaration, owner, signature, self.modality, access)
            .map_err(InheritanceSlotResolutionError::Contract)
    }
}
impl DecodedInheritanceSlotImplementationV1 {
    pub fn resolve<R: InheritanceSlotResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<InheritanceSlotImplementationV1, InheritanceSlotResolutionError<E>> {
        match self {
            Self::Abstract => Ok(InheritanceSlotImplementationV1::Abstract),
            Self::Concrete(target) => target
                .resolve(resolver, meter)
                .map(InheritanceSlotImplementationV1::Concrete),
            Self::InterfaceDefault(target) => target
                .resolve(resolver, meter)
                .map(InheritanceSlotImplementationV1::InterfaceDefault),
        }
    }
}
impl DecodedInheritanceSlotContractV1 {
    pub fn resolve<R: InheritanceSlotResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<InheritanceSlotContractV1, InheritanceSlotResolutionError<E>> {
        meter
            .charge_nodes(1, &WirePath::root())
            .map_err(InheritanceSlotResolutionError::Resource)?;
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
            .resolve(resolver, meter)
            .map_err(InheritanceSlotResolutionError::Signature)?;
        let domain = self
            .domain
            .resolve_metered(resolver, meter)
            .map_err(InheritanceSlotResolutionError::Domain)?;
        let implementation = self.implementation.resolve(resolver, meter)?;
        let access = self
            .declaration_access
            .resolve_metered(resolver, meter)
            .map_err(InheritanceSlotResolutionError::Source)?;
        InheritanceSlotContractV1::try_new(
            slot,
            owner,
            declaration,
            signature,
            PersistentSlotContractDomainV1::new(domain),
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
    Domain(PersistentAccessResolutionError<E>),
    Source(DeclarationAccessSourceResolutionError<E>),
    Contract(InheritanceSlotContractBuildError),
}
impl<E: fmt::Display> fmt::Display for InheritanceSlotResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Identity(error) => error.fmt(f),
            Self::Signature(error) => error.fmt(f),
            Self::Domain(error) => error.fmt(f),
            Self::Source(error) => error.fmt(f),
            Self::Contract(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for InheritanceSlotResolutionError<E> {}
