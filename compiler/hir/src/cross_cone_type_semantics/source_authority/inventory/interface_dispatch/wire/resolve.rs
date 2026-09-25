use super::*;
use scoop_identity::PersistentIdResolver;

impl DecodedInterfaceSourceMemberV1 {
    pub(crate) fn resolve_member<
        R: PersistentIdResolver<PersistentDispatchSlotId, Error = E>,
        E,
    >(
        self,
        resolver: &mut R,
    ) -> Result<InterfaceSourceMemberV1, InterfaceSourceMemberResolutionError<E>> {
        Ok(InterfaceSourceMemberV1::new(
            resolver
                .resolve(self.slot)
                .map_err(InterfaceSourceMemberResolutionError::Reference)?,
            self.overrides
                .resolve(resolver)
                .map_err(InterfaceSourceMemberResolutionError::Overrides)?,
        ))
    }
}

#[derive(Debug)]
pub enum InterfaceSourceMemberResolutionError<E> {
    Reference(E),
    Overrides(crate::CanonicalPersistentIdSetValidationError<PersistentDispatchSlotId, E>),
}
impl<E: fmt::Display> fmt::Display for InterfaceSourceMemberResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(e) => e.fmt(f),
            Self::Overrides(e) => e.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for InterfaceSourceMemberResolutionError<E> {}

impl DecodedInterfaceSourceDispatchV1 {
    pub fn resolve<R, E: fmt::Display>(
        self,
        resolver: &mut R,
    ) -> Result<InterfaceSourceDispatchV1, SourceInventoryError>
    where
        R: PersistentIdResolver<PersistentExactTypeId, Error = E>
            + PersistentIdResolver<PersistentDispatchSlotId, Error = E>,
    {
        let mut parents = reserve(self.parents.len())?;
        let mut members = reserve(self.members.len())?;

        let owner = resolver.resolve(self.owner).map_err(reference)?;
        for parent in self.parents {
            parents.push(resolver.resolve(parent).map_err(reference)?);
        }
        for member in self.members {
            let slot = resolver.resolve(member.slot).map_err(reference)?;
            let overrides = member.overrides.resolve(resolver).map_err(reference)?;
            members.push(InterfaceSourceMemberV1::new(slot, overrides));
        }
        InterfaceSourceDispatchV1::try_new(owner, parents, members)
    }
}
impl DecodedCanonicalInterfaceSourceDispatchesV1 {
    pub fn resolve<R, E: fmt::Display>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalInterfaceSourceDispatchesV1, SourceInventoryError>
    where
        R: PersistentIdResolver<PersistentExactTypeId, Error = E>
            + PersistentIdResolver<PersistentDispatchSlotId, Error = E>,
    {
        let mut records = reserve(self.records.len())?;
        for record in self.records {
            records.push(record.resolve(resolver)?);
        }
        CanonicalInterfaceSourceDispatchesV1::from_ordered(records)
    }
}
