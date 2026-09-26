//! Reuses schema graph validation with declarations from the decoded metadata.

use super::*;
use crate::{
    CanonicalInheritanceSlotSchemasV1, InheritanceSlotSchemaSemanticAuthority,
    NominalDispatchOrderV1,
};
use scoop_identity::{
    DispatchSlotKey, PersistentDispatchSlotId, PersistentFunctionId, PersistentPropertyAccessorId,
    PersistentPropertyId, PropertyAccessorKey,
};

mod classes;
mod declarations;
mod keys;
mod slots;

pub(super) fn validate(
    current: CheckedSharedTypeFoundationV1<'_>,
    dependencies: &[CheckedSharedTypeFoundationV1<'_>],
    sources: &Context<'_>,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
) -> Result<(), Error> {
    let mut context = SchemaDeclarations::default();
    for provider in std::iter::once(current).chain(dependencies.iter().copied()) {
        declarations::collect(&mut context, provider, dependencies, graph)?;
    }
    for (owner, order) in &context.orders {
        classes::validate(*owner, order, &context, graph)?;
        graph
            .validate_slot_schemas(*owner, &context)
            .map_err(|error| Error::SlotSchemas(Box::new(error)))?;
    }
    slots::validate(current, dependencies, sources, &mut context, graph)
}

#[derive(Default)]
struct SchemaDeclarations<'a> {
    selections: BTreeMap<PersistentExactTypeId, &'a crate::CanonicalNominalDispatchSelectionsV1>,
    schemas: BTreeMap<PersistentExactTypeId, &'a CanonicalInheritanceSlotSchemasV1>,
    orders: BTreeMap<PersistentExactTypeId, &'a NominalDispatchOrderV1>,
    interface_parents: BTreeMap<PersistentExactTypeId, Vec<PersistentExactTypeId>>,
    slots: BTreeMap<PersistentDispatchSlotId, Arc<DispatchSlotKey>>,
    functions: BTreeMap<PersistentFunctionId, Arc<SourceDeclarationKey>>,
    accessors: BTreeMap<PersistentPropertyAccessorId, Arc<PropertyAccessorKey>>,
    properties: BTreeMap<PersistentPropertyId, Arc<SourceDeclarationKey>>,
}

impl InheritanceSlotSchemaSemanticAuthority<Error> for SchemaDeclarations<'_> {
    fn interface_parent_order(
        &self,
        owner: PersistentExactTypeId,
    ) -> Result<&[PersistentExactTypeId], Error> {
        self.interface_parents
            .get(&owner)
            .map(Vec::as_slice)
            .ok_or(Error::SlotOrder(owner))
    }
    fn interface_members(
        &self,
        owner: PersistentExactTypeId,
    ) -> Result<&[crate::InterfaceSourceMemberV1], Error> {
        match self.orders.get(&owner) {
            Some(NominalDispatchOrderV1::Interface { members, .. }) => Ok(members),
            _ => Err(Error::SlotOrder(owner)),
        }
    }
    fn schemas(
        &self,
        owner: PersistentExactTypeId,
    ) -> Result<&CanonicalInheritanceSlotSchemasV1, Error> {
        self.schemas
            .get(&owner)
            .copied()
            .ok_or(Error::SlotOrder(owner))
    }
    fn dispatch_slot_key(&self, slot: PersistentDispatchSlotId) -> Result<&DispatchSlotKey, Error> {
        self.slots
            .get(&slot)
            .map(AsRef::as_ref)
            .ok_or(Error::SlotSource(slot))
    }
    fn function_key(&self, id: PersistentFunctionId) -> Result<&SourceDeclarationKey, Error> {
        self.functions
            .get(&id)
            .map(AsRef::as_ref)
            .ok_or(Error::DeclarationMetadata(
                scoop_identity::DefinitionOriginSubject::Function(id),
            ))
    }
    fn accessor_key(
        &self,
        id: PersistentPropertyAccessorId,
    ) -> Result<&PropertyAccessorKey, Error> {
        self.accessors
            .get(&id)
            .map(AsRef::as_ref)
            .ok_or(Error::DeclarationMetadata(
                scoop_identity::DefinitionOriginSubject::PropertyAccessor(id),
            ))
    }
    fn property_key(&self, id: PersistentPropertyId) -> Result<&SourceDeclarationKey, Error> {
        self.properties
            .get(&id)
            .map(AsRef::as_ref)
            .ok_or(Error::DeclarationMetadata(
                scoop_identity::DefinitionOriginSubject::Property(id),
            ))
    }
}
