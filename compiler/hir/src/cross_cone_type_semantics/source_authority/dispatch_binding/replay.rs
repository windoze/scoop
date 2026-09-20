use super::*;

impl InheritanceSlotSchemaSemanticAuthority<InheritanceDispatchBindingError>
    for BoundInheritanceDispatchSourcesV1<'_, '_>
{
    fn interface_dispatch_source(
        &self,
        owner: PersistentExactTypeId,
    ) -> Result<&InterfaceSourceDispatchV1, InheritanceDispatchBindingError> {
        self.interfaces
            .get(owner)
            .ok_or(InheritanceDispatchBindingError::MissingInterface(owner))
    }

    fn schemas(
        &self,
        owner: PersistentExactTypeId,
    ) -> Result<&CanonicalInheritanceSlotSchemasV1, InheritanceDispatchBindingError> {
        self.inventory
            .get(owner)
            .map(SourceInheritanceInventoryV1::slot_schemas)
            .ok_or(InheritanceDispatchBindingError::MissingOwner(owner))
    }

    fn dispatch_slot_key(
        &self,
        slot: PersistentDispatchSlotId,
    ) -> Result<&DispatchSlotKey, InheritanceDispatchBindingError> {
        self.slots
            .get(&slot)
            .copied()
            .ok_or(InheritanceDispatchBindingError::MissingSlot(slot))
    }

    fn function_key(
        &self,
        function: PersistentFunctionId,
    ) -> Result<&SourceDeclarationKey, InheritanceDispatchBindingError> {
        self.functions
            .get(&function)
            .copied()
            .ok_or(InheritanceDispatchBindingError::MissingFunction(function))
    }

    fn accessor_key(
        &self,
        accessor: PersistentPropertyAccessorId,
    ) -> Result<&PropertyAccessorKey, InheritanceDispatchBindingError> {
        self.foundation.accessor_key(accessor).map_err(Into::into)
    }

    fn property_key(
        &self,
        property: PersistentPropertyId,
    ) -> Result<&SourceDeclarationKey, InheritanceDispatchBindingError> {
        self.properties
            .get(&property)
            .copied()
            .ok_or(InheritanceDispatchBindingError::MissingProperty(property))
    }
}
