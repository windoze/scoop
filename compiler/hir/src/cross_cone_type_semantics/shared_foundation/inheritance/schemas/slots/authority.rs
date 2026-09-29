use super::*;
use crate::{InheritanceSlotContractSemanticAuthority, InheritanceSourceCallableFactsV1};

pub(super) struct Replay<'s, 'a, 'n> {
    pub data: &'s Data<'a>,
    pub schemas: &'s SchemaDeclarations<'a>,
    pub sources: &'s Context<'n>,
    pub unit: PersistentExactTypeId,
}

impl InheritanceSlotSourceSemanticAuthority<Error> for Replay<'_, '_, '_> {
    fn inheritance_callable_source(
        &self,
        declaration: Declaration,
        receiver: PersistentExactTypeId,
    ) -> Result<InheritanceSourceCallableFactsV1<'_>, Error> {
        let member = self
            .data
            .members
            .get(&declaration)
            .ok_or(Error::SlotCallable(declaration))?;
        let signature = self
            .data
            .signatures
            .get(&(declaration, receiver))
            .ok_or(Error::SlotCallable(declaration))?;
        Ok(InheritanceSourceCallableFactsV1 {
            signature,
            modality: member.source.modality(),
            declaration_access: &member.access,
        })
    }
    fn inheritance_slot_selection(
        &self,
        owner: PersistentExactTypeId,
        slot: PersistentDispatchSlotId,
    ) -> Result<Selection, Error> {
        let selections = self
            .schemas
            .selections
            .get(&owner)
            .ok_or(Error::SlotSelectionInventory(owner))?
            .records();
        let index = selections
            .binary_search_by_key(&slot, |record| record.slot())
            .map_err(|_| selection_error(owner, slot))?;
        Ok(selections[index].selection())
    }
}

impl InheritanceSlotContractSemanticAuthority<Error> for Replay<'_, '_, '_> {
    fn unit_exact_type(&self) -> Result<PersistentExactTypeId, Error> {
        Ok(self.unit)
    }
}

impl InheritanceSlotSchemaSemanticAuthority<Error> for Replay<'_, '_, '_> {
    fn interface_parent_order(
        &self,
        owner: PersistentExactTypeId,
    ) -> Result<&[PersistentExactTypeId], Error> {
        self.schemas.interface_parent_order(owner)
    }
    fn interface_members(
        &self,
        owner: PersistentExactTypeId,
    ) -> Result<&[crate::InterfaceSourceMemberV1], Error> {
        self.schemas.interface_members(owner)
    }
    fn schemas(
        &self,
        owner: PersistentExactTypeId,
    ) -> Result<&CanonicalInheritanceSlotSchemasV1, Error> {
        self.schemas.schemas(owner)
    }
    fn dispatch_slot_key(&self, slot: PersistentDispatchSlotId) -> Result<&DispatchSlotKey, Error> {
        self.schemas.dispatch_slot_key(slot)
    }
    fn function_key(&self, id: PersistentFunctionId) -> Result<&SourceDeclarationKey, Error> {
        self.schemas.function_key(id)
    }
    fn accessor_key(
        &self,
        id: PersistentPropertyAccessorId,
    ) -> Result<&PropertyAccessorKey, Error> {
        self.schemas.accessor_key(id)
    }
    fn property_key(&self, id: PersistentPropertyId) -> Result<&SourceDeclarationKey, Error> {
        self.schemas.property_key(id)
    }
}

impl NominalInheritanceSemanticAuthority<Error> for Replay<'_, '_, '_> {
    fn exact_type_key(&self, exact: PersistentExactTypeId) -> Result<&ExactTypeKey, Error> {
        self.data
            .exacts
            .get(&exact)
            .map(AsRef::as_ref)
            .map_or_else(|| self.sources.exact_type_key(exact), Ok)
    }
    fn nominal_declaration_key(
        &self,
        owner: SourceNominalId,
    ) -> Result<&SourceDeclarationKey, Error> {
        self.sources.nominal_declaration_key(owner)
    }
    fn nominal_access_source(
        &self,
        owner: SourceNominalId,
    ) -> Result<&DeclarationAccessSourceV1, Error> {
        self.sources.nominal_access_source(owner)
    }
    fn nominal_definition_source(
        &self,
        owner: SourceNominalId,
    ) -> Result<&ExportDefinitionSourceV1, Error> {
        self.sources.nominal_definition_source(owner)
    }
    fn validate_definition_source(&self, source: &ExportDefinitionSourceV1) -> Result<(), Error> {
        if self.data.origins.contains(source) {
            Ok(())
        } else {
            self.sources.validate_definition_source(source)
        }
    }
    fn object_representation(
        &self,
        owner: PersistentTypeId,
    ) -> Result<&NominalRepresentationSupportV1, Error> {
        self.sources.object_representation(owner)
    }
    fn generated_nominal_key(
        &self,
        owner: PersistentTypeId,
    ) -> Result<&GeneratedNominalKey, Error> {
        self.sources.generated_nominal_key(owner)
    }
}
