use super::*;

#[derive(Clone)]
pub(in crate::cross_cone_type_semantics) struct InheritanceInterfaceFixtureData {
    pub owners: CanonicalPersistentIdsV1<PersistentExactTypeId>,
    pub constructors:
        BTreeMap<PersistentExactTypeId, CanonicalPersistentIdsV1<PersistentConstructorId>>,
    pub members: BTreeMap<PersistentExactTypeId, CanonicalProtectedDeclarationRefsV1>,
    pub constructor_sources:
        BTreeMap<PersistentConstructorId, NominalSupportConstructorInterfaceV1>,
    pub interface_sources: BTreeMap<
        PersistentExactTypeId,
        (
            Vec<PersistentExactTypeId>,
            Vec<crate::InterfaceSourceMemberV1>,
        ),
    >,
    pub schemas: BTreeMap<PersistentExactTypeId, CanonicalInheritanceSlotSchemasV1>,
    pub slot_keys: BTreeMap<PersistentDispatchSlotId, DispatchSlotKey>,
    pub callables: BTreeMap<
        InheritanceCallableDeclarationV1,
        (
            InheritanceCallableSignatureV1,
            CallableModalityV1,
            DeclarationAccessSourceV1,
        ),
    >,
    pub selections: BTreeMap<
        (PersistentExactTypeId, PersistentDispatchSlotId),
        InheritanceSourceSlotSelectionV1,
    >,
}
impl Default for InheritanceInterfaceFixtureData {
    fn default() -> Self {
        Self {
            owners: CanonicalPersistentIdsV1::empty(),
            constructors: BTreeMap::new(),
            members: BTreeMap::new(),
            constructor_sources: BTreeMap::new(),
            schemas: BTreeMap::new(),
            interface_sources: BTreeMap::new(),
            slot_keys: BTreeMap::new(),
            callables: BTreeMap::new(),
            selections: BTreeMap::new(),
        }
    }
}
impl InheritanceSlotSchemaSemanticAuthority<&'static str> for Fixture {
    fn interface_parent_order(
        &self,
        owner: PersistentExactTypeId,
    ) -> Result<&[PersistentExactTypeId], &'static str> {
        self.inheritance_interfaces
            .interface_sources
            .get(&owner)
            .map(|source| source.0.as_slice())
            .ok_or("unknown interface parents")
    }
    fn interface_members(
        &self,
        owner: PersistentExactTypeId,
    ) -> Result<&[crate::InterfaceSourceMemberV1], &'static str> {
        self.inheritance_interfaces
            .interface_sources
            .get(&owner)
            .map(|source| source.1.as_slice())
            .ok_or("unknown interface members")
    }
    fn schemas(
        &self,
        owner: PersistentExactTypeId,
    ) -> Result<&CanonicalInheritanceSlotSchemasV1, &'static str> {
        self.inheritance_interfaces
            .schemas
            .get(&owner)
            .ok_or("unknown schema")
    }
    fn dispatch_slot_key(
        &self,
        slot: PersistentDispatchSlotId,
    ) -> Result<&DispatchSlotKey, &'static str> {
        self.inheritance_interfaces
            .slot_keys
            .get(&slot)
            .ok_or("unknown slot")
    }
    fn function_key(
        &self,
        id: PersistentFunctionId,
    ) -> Result<&SourceDeclarationKey, &'static str> {
        self.declarations
            .get(&CallableTemplateOrigin::Function(id))
            .ok_or("unknown function")
    }
    fn accessor_key(
        &self,
        id: PersistentPropertyAccessorId,
    ) -> Result<&PropertyAccessorKey, &'static str> {
        self.accessors.get(&id).ok_or("unknown accessor")
    }
    fn property_key(
        &self,
        id: PersistentPropertyId,
    ) -> Result<&SourceDeclarationKey, &'static str> {
        self.property_source_key(id)
    }
}
impl InheritanceSlotContractSemanticAuthority<&'static str> for Fixture {
    fn unit_exact_type(&self) -> Result<PersistentExactTypeId, &'static str> {
        Ok(self.unit.exact)
    }
}
impl NominalInheritanceInterfaceSemanticAuthority<&'static str> for Fixture {
    fn required_inheritance_owners(
        &self,
    ) -> Result<&CanonicalPersistentIdsV1<PersistentExactTypeId>, &'static str> {
        Ok(&self.inheritance_interfaces.owners)
    }
    fn required_inheritance_constructors(
        &self,
        owner: PersistentExactTypeId,
    ) -> Result<&CanonicalPersistentIdsV1<PersistentConstructorId>, &'static str> {
        self.inheritance_interfaces
            .constructors
            .get(&owner)
            .ok_or("unknown constructor inventory")
    }
    fn required_inheritance_protected_members(
        &self,
        owner: PersistentExactTypeId,
    ) -> Result<&CanonicalProtectedDeclarationRefsV1, &'static str> {
        self.inheritance_interfaces
            .members
            .get(&owner)
            .ok_or("unknown member inventory")
    }
    fn constructor_source(
        &self,
        declaration: PersistentConstructorId,
    ) -> Result<&NominalSupportConstructorInterfaceV1, &'static str> {
        self.inheritance_interfaces
            .constructor_sources
            .get(&declaration)
            .ok_or("unknown constructor source")
    }
}
impl InheritanceSlotSourceSemanticAuthority<&'static str> for Fixture {
    fn inheritance_callable_source(
        &self,
        declaration: InheritanceCallableDeclarationV1,
    ) -> Result<InheritanceSourceCallableFactsV1<'_>, &'static str> {
        let (signature, modality, access) = self
            .inheritance_interfaces
            .callables
            .get(&declaration)
            .ok_or("unknown callable source")?;
        Ok(InheritanceSourceCallableFactsV1 {
            signature,
            modality: *modality,
            declaration_access: access,
        })
    }
    fn inheritance_slot_selection(
        &self,
        owner: PersistentExactTypeId,
        slot: PersistentDispatchSlotId,
    ) -> Result<InheritanceSourceSlotSelectionV1, &'static str> {
        self.inheritance_interfaces
            .selections
            .get(&(owner, slot))
            .copied()
            .ok_or("unknown slot selection")
    }
}
