use super::*;
use scoop_identity::{PersistentConstructorId, PersistentDispatchSlotId};

impl InheritanceSlotContractSemanticAuthority<Error> for BoundInheritanceSourcesV1<'_, '_, '_> {
    fn unit_exact_type(&self) -> Result<PersistentExactTypeId, Error> {
        self.slots.unit_exact_type().map_err(Into::into)
    }
}
impl InheritanceSlotSourceSemanticAuthority<Error> for BoundInheritanceSourcesV1<'_, '_, '_> {
    fn inheritance_callable_source(
        &self,
        declaration: InheritanceCallableDeclarationV1,
    ) -> Result<InheritanceSourceCallableFactsV1<'_>, Error> {
        self.slots
            .inheritance_callable_source(declaration)
            .map_err(Into::into)
    }
    fn inheritance_slot_selection(
        &self,
        owner: PersistentExactTypeId,
        slot: PersistentDispatchSlotId,
    ) -> Result<InheritanceSourceSlotSelectionV1, Error> {
        self.slots
            .inheritance_slot_selection(owner, slot)
            .map_err(Into::into)
    }
}
impl NominalInheritanceInterfaceSemanticAuthority<Error> for BoundInheritanceSourcesV1<'_, '_, '_> {
    fn required_inheritance_owners(
        &self,
    ) -> Result<&CanonicalPersistentIdsV1<PersistentExactTypeId>, Error> {
        Ok(self.protected.inventory.owners())
    }
    fn required_inheritance_constructors(
        &self,
        owner: PersistentExactTypeId,
    ) -> Result<&CanonicalPersistentIdsV1<PersistentConstructorId>, Error> {
        self.constructors.required_for(owner).map_err(Into::into)
    }
    fn required_inheritance_protected_members(
        &self,
        owner: PersistentExactTypeId,
    ) -> Result<&CanonicalProtectedDeclarationRefsV1, Error> {
        self.protected
            .inventory
            .get(owner)
            .map(SourceInheritanceInventoryV1::protected_members)
            .ok_or(Error::MissingOwner(owner))
    }
    fn constructor_source(
        &self,
        declaration: PersistentConstructorId,
    ) -> Result<&NominalSupportConstructorInterfaceV1, Error> {
        self.constructors
            .constructor_source(declaration)
            .map_err(Into::into)
    }
}
