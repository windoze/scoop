//! Reuses schema graph validation with declarations from the decoded metadata.

use super::*;
use crate::{
    CanonicalInheritanceSlotSchemasV1, InheritanceSlotSchemaSemanticAuthority,
    InterfaceSourceDispatchV1, NominalDispatchOrderV1,
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
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let mut context = SchemaDeclarations::default();
    for provider in std::iter::once(current).chain(dependencies.iter().copied()) {
        declarations::collect(&mut context, provider, dependencies, graph, meter)?;
    }
    for (owner, order) in &context.orders {
        classes::validate(*owner, order, &context, graph, meter)?;
        graph
            .validate_slot_schemas(*owner, &context, meter)
            .map_err(|error| Error::SlotSchemas(Box::new(error)))?;
    }
    slots::validate(current, dependencies, sources, &mut context, graph, meter)
}

#[derive(Default)]
struct SchemaDeclarations<'a> {
    selections: BTreeMap<PersistentExactTypeId, &'a crate::CanonicalNominalDispatchSelectionsV1>,
    schemas: BTreeMap<PersistentExactTypeId, &'a CanonicalInheritanceSlotSchemasV1>,
    orders: BTreeMap<PersistentExactTypeId, &'a NominalDispatchOrderV1>,
    interfaces: BTreeMap<PersistentExactTypeId, InterfaceSourceDispatchV1>,
    slots: BTreeMap<PersistentDispatchSlotId, Arc<DispatchSlotKey>>,
    functions: BTreeMap<PersistentFunctionId, Arc<SourceDeclarationKey>>,
    accessors: BTreeMap<PersistentPropertyAccessorId, Arc<PropertyAccessorKey>>,
    properties: BTreeMap<PersistentPropertyId, Arc<SourceDeclarationKey>>,
}

impl InheritanceSlotSchemaSemanticAuthority<Error> for SchemaDeclarations<'_> {
    fn interface_dispatch_source(
        &self,
        owner: PersistentExactTypeId,
    ) -> Result<&InterfaceSourceDispatchV1, Error> {
        self.interfaces.get(&owner).ok_or(Error::SlotOrder(owner))
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
