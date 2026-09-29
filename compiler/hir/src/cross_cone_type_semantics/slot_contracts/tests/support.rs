use std::ops::{Deref, DerefMut};

use crate::cross_cone_type_semantics::inheritance::tests::support::{Node, site};
use crate::cross_cone_type_semantics::slot_schemas::tests::support::Fixture as SchemaFixture;
use crate::*;
use scoop_identity::GcEffect;
use scoop_identity::*;

mod authority;

pub(in crate::cross_cone_type_semantics) struct Fixture {
    pub schema: SchemaFixture,
    pub unit: Node,
}
impl Default for Fixture {
    fn default() -> Self {
        let mut schema = SchemaFixture::default();
        let unit = schema.add("Unit", SourceNominalKind::Struct);
        Self { schema, unit }
    }
}
impl Deref for Fixture {
    type Target = SchemaFixture;
    fn deref(&self) -> &Self::Target {
        &self.schema
    }
}
impl DerefMut for Fixture {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.schema
    }
}

pub(super) fn nominal(owner: Node) -> PersistentTypeId {
    let SourceNominalId::Concrete(id) = owner.source else {
        unreachable!()
    };
    id
}
pub(super) fn effects(
    gc: GcEffect,
    implementation: CallableImplementationV1,
) -> CallableSourceEffectsV1 {
    CallableSourceEffectsV1::try_new(
        Effect::Ordinary,
        CallableSafetyV1::Safe,
        gc,
        implementation,
        CallableOperatorRoleV1::None,
        CallableInfixV1::Ordinary,
    )
    .unwrap()
}
impl Fixture {
    pub fn signature(
        &self,
        owner: Node,
        parameters: Vec<PersistentExactTypeId>,
    ) -> InheritanceCallableSignatureV1 {
        InheritanceCallableSignatureV1::try_new(
            ExactCallableSignature::new(
                Effect::Ordinary,
                Some(owner.exact),
                parameters,
                self.unit.exact,
            ),
            effects(GcEffect::Managed, CallableImplementationV1::Scoop),
        )
        .unwrap()
    }
    pub fn access(
        &self,
        owner: Node,
        visibility: DeclaredVisibilityV1,
    ) -> DeclarationAccessSourceV1 {
        let mut chain = self.inheritance.access[&owner.source]
            .lexical_owners()
            .to_vec();
        chain.push(owner.source);
        DeclarationAccessSourceV1::try_new(
            visibility,
            chain,
            self.inheritance.origins[&owner.source].clone(),
        )
        .unwrap()
    }
    pub fn declaration(&self, slot: PersistentDispatchSlotId) -> InheritanceCallableDeclarationV1 {
        let key = &self.slots[&slot];
        match key.owner() {
            DispatchDeclarationOwner::Function(id) => {
                InheritanceCallableDeclarationV1::Function(id)
            }
            DispatchDeclarationOwner::Accessor(id)
                if key.role() == DispatchRole::PropertyGetter =>
            {
                InheritanceCallableDeclarationV1::Getter(id)
            }
            DispatchDeclarationOwner::Accessor(id) => InheritanceCallableDeclarationV1::Setter(id),
        }
    }
    pub fn method(
        &mut self,
        owner: Node,
        name: &str,
        parameters: Vec<SignatureTypeKey>,
    ) -> PersistentDispatchSlotId {
        let key = SourceDeclarationKey::function(
            site(&[owner.source]),
            CanonicalIdentifier::new(name).unwrap(),
            0,
            None,
            parameters,
        );
        let id = PersistentFunctionId::from_source_declaration(&key).unwrap();
        self.functions.insert(id, key);
        let slot_key = if self.inheritance.records[&owner.exact].modality()
            == NominalInheritanceModalityV1::Interface
        {
            DispatchSlotKey::interface_method(id)
        } else {
            DispatchSlotKey::virtual_method(id)
        };
        let slot = self.slot(slot_key);
        self.declare(owner, slot);
        slot
    }
    pub fn schema(&mut self, owner: Node, slots: &[PersistentDispatchSlotId]) {
        let role = if self.inheritance.records[&owner.exact].modality()
            == NominalInheritanceModalityV1::Interface
        {
            InheritanceSlotSchemaRoleV1::Interface {
                interface_exact: owner.exact,
            }
        } else {
            InheritanceSlotSchemaRoleV1::ClassVtable
        };
        self.set(
            owner,
            vec![InheritanceSlotSchemaV1::try_new(role, slots.to_vec()).unwrap()],
        );
    }
    pub fn concrete(&self, owner: Node, slot: PersistentDispatchSlotId) -> InheritanceSlotTargetV1 {
        InheritanceSlotTargetV1::new(
            self.declaration(slot),
            self.signature(owner, vec![]),
            CallableModalityV1::Open,
            self.access(owner, DeclaredVisibilityV1::Public),
        )
    }
    pub fn abstract_target(
        &self,
        owner: Node,
        slot: PersistentDispatchSlotId,
    ) -> InheritanceSlotTargetV1 {
        let mut target = self.concrete(owner, slot);
        target.modality = CallableModalityV1::Abstract;
        target
    }
    pub fn contract(
        &self,
        owner: Node,
        slot: PersistentDispatchSlotId,
        implementation: InheritanceSlotImplementationV1,
    ) -> InheritanceSlotContractV1 {
        InheritanceSlotContractV1::try_new(
            slot,
            self.declaration(slot),
            self.signature(owner, vec![]),
            implementation,
            self.access(owner, DeclaredVisibilityV1::Public),
        )
        .unwrap()
    }
}
