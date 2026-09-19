use std::collections::BTreeMap;

use scoop_identity::{
    AccessorRole, CanonicalIdentifier, DecodedPersistentId, DispatchSlotKey,
    PersistentDispatchSlotId, PersistentExactTypeId, PersistentFunctionId, PersistentIdResolver,
    PersistentPropertyAccessorId, PersistentPropertyId, PropertyAccessorKey, PropertyOwner,
    SourceDeclarationKey, SourceNominalKind,
};

use super::super::*;
use crate::cross_cone_type_semantics::inheritance::tests::support::{
    Fixture as InheritanceFixture, Node, site,
};

#[derive(Default)]
pub(in crate::cross_cone_type_semantics) struct Fixture {
    pub inheritance: InheritanceFixture,
    pub schemas: BTreeMap<PersistentExactTypeId, CanonicalInheritanceSlotSchemasV1>,
    pub slots: BTreeMap<PersistentDispatchSlotId, DispatchSlotKey>,
    pub functions: BTreeMap<PersistentFunctionId, SourceDeclarationKey>,
    pub accessors: BTreeMap<PersistentPropertyAccessorId, PropertyAccessorKey>,
    pub properties: BTreeMap<PersistentPropertyId, SourceDeclarationKey>,
}
impl Fixture {
    pub fn add(&mut self, name: &str, kind: SourceNominalKind) -> Node {
        let node = self.inheritance.add(name, kind, &[]);
        let role = if kind == SourceNominalKind::Interface {
            Some(InheritanceSlotSchemaRoleV1::Interface {
                interface_exact: node.exact,
            })
        } else if matches!(kind, SourceNominalKind::Class | SourceNominalKind::Object) {
            Some(InheritanceSlotSchemaRoleV1::ClassVtable)
        } else {
            None
        };
        self.set(
            node,
            role.into_iter()
                .map(|role| InheritanceSlotSchemaV1::try_new(role, vec![]).unwrap())
                .collect(),
        );
        node
    }
    pub fn set(&mut self, node: Node, schemas: Vec<InheritanceSlotSchemaV1>) {
        self.schemas.insert(
            node.exact,
            CanonicalInheritanceSlotSchemasV1::try_new(schemas).unwrap(),
        );
    }
    pub fn function(&mut self, owner: Node, name: &str) -> PersistentDispatchSlotId {
        let key = SourceDeclarationKey::function(
            site(&[owner.source]),
            CanonicalIdentifier::new(name).unwrap(),
            0,
            None,
            vec![],
        );
        let id = PersistentFunctionId::from_source_declaration(&key).unwrap();
        let slot = if self.inheritance.records[&owner.exact].modality()
            == crate::NominalInheritanceModalityV1::Interface
        {
            DispatchSlotKey::interface_method(id)
        } else {
            DispatchSlotKey::virtual_method(id)
        };
        self.functions.insert(id, key);
        self.slot(slot)
    }
    pub fn accessor(
        &mut self,
        owner: Node,
        name: &str,
        role: AccessorRole,
    ) -> PersistentDispatchSlotId {
        let property_key = SourceDeclarationKey::property(
            site(&[owner.source]),
            CanonicalIdentifier::new(name).unwrap(),
        );
        let property = PersistentPropertyId::from_source_declaration(&property_key).unwrap();
        self.properties.insert(property, property_key);
        let key = PropertyAccessorKey::new(PropertyOwner::Property(property), role);
        let id = PersistentPropertyAccessorId::from_key(&key).unwrap();
        self.accessors.insert(id, key);
        self.slot(match role {
            AccessorRole::Getter => DispatchSlotKey::property_getter(id),
            AccessorRole::Setter => DispatchSlotKey::property_setter(id),
        })
    }
    pub fn slot(&mut self, key: DispatchSlotKey) -> PersistentDispatchSlotId {
        let slot = PersistentDispatchSlotId::from_key(&key).unwrap();
        self.slots.insert(slot, key);
        slot
    }
}
impl InheritanceSlotSchemaSemanticAuthority<&'static str> for Fixture {
    fn schemas(
        &self,
        owner: PersistentExactTypeId,
    ) -> Result<&CanonicalInheritanceSlotSchemasV1, &'static str> {
        self.schemas.get(&owner).ok_or("missing owner schemas")
    }
    fn dispatch_slot_key(
        &self,
        slot: PersistentDispatchSlotId,
    ) -> Result<&DispatchSlotKey, &'static str> {
        self.slots.get(&slot).ok_or("unknown slot")
    }
    fn function_key(
        &self,
        function: PersistentFunctionId,
    ) -> Result<&SourceDeclarationKey, &'static str> {
        self.functions.get(&function).ok_or("unknown function")
    }
    fn accessor_key(
        &self,
        accessor: PersistentPropertyAccessorId,
    ) -> Result<&PropertyAccessorKey, &'static str> {
        self.accessors.get(&accessor).ok_or("unknown accessor")
    }
    fn property_key(
        &self,
        property: PersistentPropertyId,
    ) -> Result<&SourceDeclarationKey, &'static str> {
        self.properties.get(&property).ok_or("unknown property")
    }
}
impl PersistentIdResolver<PersistentExactTypeId> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentExactTypeId>,
    ) -> Result<PersistentExactTypeId, Self::Error> {
        self.inheritance.resolve(id)
    }
}
impl PersistentIdResolver<PersistentDispatchSlotId> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentDispatchSlotId>,
    ) -> Result<PersistentDispatchSlotId, Self::Error> {
        self.slots
            .keys()
            .copied()
            .find(|known| id.as_array() == known.as_array())
            .ok_or("unknown slot")
    }
}
