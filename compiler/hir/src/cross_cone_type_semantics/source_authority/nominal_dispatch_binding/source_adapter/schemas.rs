use super::*;
use scoop_identity::{
    DispatchSlotKey, PersistentDispatchSlotId, PersistentFunctionId, PersistentPropertyAccessorId,
    PersistentPropertyId, PropertyAccessorKey,
};

impl InheritanceSlotSchemaSemanticAuthority<Error>
    for BoundNominalDispatchSourcesV1<'_, '_, '_, '_, '_>
{
    fn interface_dispatch_source(
        &self,
        owner: PersistentExactTypeId,
    ) -> Result<&InterfaceSourceDispatchV1, Error> {
        self.slots
            .interface_dispatch_source(owner)
            .map_err(Into::into)
    }
    fn schemas(
        &self,
        owner: PersistentExactTypeId,
    ) -> Result<&CanonicalInheritanceSlotSchemasV1, Error> {
        self.slots.schemas(owner).map_err(Into::into)
    }
    fn dispatch_slot_key(&self, slot: PersistentDispatchSlotId) -> Result<&DispatchSlotKey, Error> {
        self.slots.dispatch_slot_key(slot).map_err(Into::into)
    }
    fn function_key(&self, function: PersistentFunctionId) -> Result<&SourceDeclarationKey, Error> {
        self.slots.function_key(function).map_err(Into::into)
    }
    fn accessor_key(
        &self,
        accessor: PersistentPropertyAccessorId,
    ) -> Result<&PropertyAccessorKey, Error> {
        self.slots.accessor_key(accessor).map_err(Into::into)
    }
    fn property_key(&self, property: PersistentPropertyId) -> Result<&SourceDeclarationKey, Error> {
        self.slots.property_key(property).map_err(Into::into)
    }
}
