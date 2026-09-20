use super::*;

impl InheritanceSlotSchemaSemanticAuthority<&'static str> for Replay {
    fn interface_dispatch_source(
        &self,
        owner: PersistentExactTypeId,
    ) -> Result<&InterfaceSourceDispatchV1, &'static str> {
        self.sources.get(owner).ok_or("missing interface source")
    }
    fn schemas(
        &self,
        owner: PersistentExactTypeId,
    ) -> Result<&CanonicalInheritanceSlotSchemasV1, &'static str> {
        self.schemas.get(&owner).ok_or("missing schemas")
    }
    fn dispatch_slot_key(
        &self,
        slot: PersistentDispatchSlotId,
    ) -> Result<&DispatchSlotKey, &'static str> {
        self.slots
            .get(&slot)
            .map(AsRef::as_ref)
            .ok_or("missing slot")
    }
    fn function_key(
        &self,
        function: PersistentFunctionId,
    ) -> Result<&SourceDeclarationKey, &'static str> {
        self.functions
            .get(&function)
            .map(AsRef::as_ref)
            .ok_or("missing function")
    }
    fn accessor_key(
        &self,
        accessor: PersistentPropertyAccessorId,
    ) -> Result<&PropertyAccessorKey, &'static str> {
        self.accessors
            .get(&accessor)
            .map(AsRef::as_ref)
            .ok_or("missing accessor")
    }
    fn property_key(
        &self,
        property: PersistentPropertyId,
    ) -> Result<&SourceDeclarationKey, &'static str> {
        self.properties
            .get(&property)
            .map(AsRef::as_ref)
            .ok_or("missing property")
    }
}
impl NominalInheritanceSemanticAuthority<&'static str> for Replay {
    fn exact_type_key(&self, exact: PersistentExactTypeId) -> Result<&ExactTypeKey, &'static str> {
        self.exacts
            .get(&exact)
            .map(AsRef::as_ref)
            .ok_or("missing exact")
    }
    fn nominal_declaration_key(
        &self,
        owner: SourceNominalId,
    ) -> Result<&SourceDeclarationKey, &'static str> {
        self.nominals
            .get(&owner)
            .map(AsRef::as_ref)
            .ok_or("missing nominal")
    }
    fn nominal_access_source(
        &self,
        owner: SourceNominalId,
    ) -> Result<&DeclarationAccessSourceV1, &'static str> {
        self.access.get(&owner).ok_or("missing access")
    }
    fn nominal_definition_source(
        &self,
        owner: SourceNominalId,
    ) -> Result<&ExportDefinitionSourceV1, &'static str> {
        self.origins.get(&owner).ok_or("missing origin")
    }
    fn validate_definition_source(
        &self,
        source: &ExportDefinitionSourceV1,
    ) -> Result<(), &'static str> {
        if self.origins.values().any(|origin| origin == source) {
            Ok(())
        } else {
            Err("unknown origin")
        }
    }
    fn object_representation(
        &self,
        _: PersistentTypeId,
    ) -> Result<&NominalRepresentationSupportV1, &'static str> {
        Err("this graph contains only interfaces")
    }
    fn generated_nominal_key(
        &self,
        _: PersistentTypeId,
    ) -> Result<&GeneratedNominalKey, &'static str> {
        Err("this graph contains only source interfaces")
    }
}
