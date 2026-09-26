use super::Fixture;
use crate::*;
use scoop_identity::*;
use std::sync::Arc;

impl InheritanceSlotSchemaSemanticAuthority<&'static str> for Fixture {
    fn interface_parent_order(
        &self,
        owner: PersistentExactTypeId,
    ) -> Result<&[PersistentExactTypeId], &'static str> {
        self.schema.interface_parent_order(owner)
    }
    fn interface_members(
        &self,
        owner: PersistentExactTypeId,
    ) -> Result<&[crate::InterfaceSourceMemberV1], &'static str> {
        self.schema.interface_members(owner)
    }
    fn schemas(
        &self,
        owner: PersistentExactTypeId,
    ) -> Result<&CanonicalInheritanceSlotSchemasV1, &'static str> {
        self.schema.schemas(owner)
    }
    fn dispatch_slot_key(
        &self,
        slot: PersistentDispatchSlotId,
    ) -> Result<&DispatchSlotKey, &'static str> {
        self.schema.dispatch_slot_key(slot)
    }
    fn function_key(
        &self,
        id: PersistentFunctionId,
    ) -> Result<&SourceDeclarationKey, &'static str> {
        self.schema.function_key(id)
    }
    fn accessor_key(
        &self,
        id: PersistentPropertyAccessorId,
    ) -> Result<&PropertyAccessorKey, &'static str> {
        self.schema.accessor_key(id)
    }
    fn property_key(
        &self,
        id: PersistentPropertyId,
    ) -> Result<&SourceDeclarationKey, &'static str> {
        self.schema.property_key(id)
    }
}
impl NominalInheritanceSemanticAuthority<&'static str> for Fixture {
    fn exact_type_key(&self, id: PersistentExactTypeId) -> Result<&ExactTypeKey, &'static str> {
        self.inheritance.exact_type_key(id)
    }
    fn nominal_declaration_key(
        &self,
        owner: SourceNominalId,
    ) -> Result<&SourceDeclarationKey, &'static str> {
        self.inheritance.keys.get(&owner).ok_or("unknown nominal")
    }
    fn nominal_access_source(
        &self,
        owner: SourceNominalId,
    ) -> Result<&DeclarationAccessSourceV1, &'static str> {
        self.inheritance.nominal_access_source(owner)
    }
    fn nominal_definition_source(
        &self,
        owner: SourceNominalId,
    ) -> Result<&ExportDefinitionSourceV1, &'static str> {
        self.inheritance.origins.get(&owner).ok_or("unknown origin")
    }
    fn validate_definition_source(
        &self,
        source: &ExportDefinitionSourceV1,
    ) -> Result<(), &'static str> {
        self.inheritance.validate_definition_source(source)
    }
    fn object_representation(
        &self,
        owner: PersistentTypeId,
    ) -> Result<&NominalRepresentationSupportV1, &'static str> {
        self.inheritance.object_representation(owner)
    }
    fn generated_nominal_key(
        &self,
        owner: PersistentTypeId,
    ) -> Result<&GeneratedNominalKey, &'static str> {
        self.inheritance.generated_nominal_key(owner)
    }
}
impl InheritanceSlotContractSemanticAuthority<&'static str> for Fixture {
    fn unit_exact_type(&self) -> Result<PersistentExactTypeId, &'static str> {
        Ok(self.unit.exact)
    }
}
impl PersistentIdResolver<PersistentExactTypeId> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentExactTypeId>,
    ) -> Result<PersistentExactTypeId, Self::Error> {
        self.schema.resolve(id)
    }
}
impl PersistentIdResolver<PersistentDispatchSlotId> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentDispatchSlotId>,
    ) -> Result<PersistentDispatchSlotId, Self::Error> {
        self.schema.resolve(id)
    }
}
impl PersistentIdResolver<PersistentTypeId> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentTypeId>,
    ) -> Result<PersistentTypeId, Self::Error> {
        self.inheritance
            .keys
            .keys()
            .find_map(|key| match key {
                SourceNominalId::Concrete(known) if known.as_array() == id.as_array() => {
                    Some(*known)
                }
                _ => None,
            })
            .ok_or("unknown nominal")
    }
}
impl PersistentIdResolver<PersistentGenericTypeId> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        _: DecodedPersistentId<PersistentGenericTypeId>,
    ) -> Result<PersistentGenericTypeId, Self::Error> {
        Err("generic owner is outside this fixture")
    }
}
impl PersistentIdResolver<PersistentFunctionId> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentFunctionId>,
    ) -> Result<PersistentFunctionId, Self::Error> {
        self.functions
            .keys()
            .copied()
            .find(|known| known.as_array() == id.as_array())
            .ok_or("unknown function")
    }
}
impl PersistentIdResolver<PersistentPropertyAccessorId> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentPropertyAccessorId>,
    ) -> Result<PersistentPropertyAccessorId, Self::Error> {
        self.accessors
            .keys()
            .copied()
            .find(|known| known.as_array() == id.as_array())
            .ok_or("unknown accessor")
    }
}
impl PersistentIdResolver<ConeIdentity> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<ConeIdentity>,
    ) -> Result<ConeIdentity, Self::Error> {
        id.verify(ConeIdentity::CORE).map_err(|_| "unknown cone")
    }
}
impl PersistentKeyResolver<PersistentSourceContextId, SourceContextKey> for Fixture {
    type Error = &'static str;
    fn resolve_key(
        &mut self,
        id: DecodedPersistentId<PersistentSourceContextId>,
    ) -> Result<Arc<SourceContextKey>, Self::Error> {
        let context = SourceContextKey::File {
            source: self.inheritance.origins[&self.unit.source]
                .origin()
                .source()
                .clone(),
        };
        id.verify(PersistentSourceContextId::from_key(&context).unwrap())
            .map_err(|_| "unknown context")?;
        Ok(Arc::new(context))
    }
}
