use super::*;
use crate::cross_cone_type_semantics::slot_schemas::tests::support::Fixture as SchemaFixture;
use scoop_identity::{
    DecodedPersistentId, PersistentFunctionId, PersistentIdResolver, PersistentPropertyAccessorId,
    SourceNominalKind,
};

pub(super) struct Fixture {
    pub inner: SchemaFixture,
    pub owners: [PersistentExactTypeId; 2],
    pub slots: [PersistentDispatchSlotId; 3],
    pub queries: usize,
}

impl Fixture {
    pub fn new() -> Self {
        let mut inner = SchemaFixture::default();
        let contract = inner.add("Contract", SourceNominalKind::Interface);
        let class = inner.add("Class", SourceNominalKind::Class);
        let slots = [
            inner.function(contract, "call"),
            inner.accessor(contract, "value", AccessorRole::Getter),
            inner.accessor(contract, "value", AccessorRole::Setter),
        ];
        Self {
            inner,
            owners: [contract.exact, class.exact],
            slots,
            queries: 0,
        }
    }

    pub fn selections(&self) -> Vec<Selection> {
        let function = *self.inner.functions.keys().next().unwrap();
        let accessor = |role| {
            *self
                .inner
                .accessors
                .iter()
                .find(|(_, key)| key.role() == role)
                .unwrap()
                .0
        };
        let mut result = vec![Selection::Abstract];
        for declaration in [
            InheritanceCallableDeclarationV1::Function(function),
            InheritanceCallableDeclarationV1::Getter(accessor(AccessorRole::Getter)),
            InheritanceCallableDeclarationV1::Setter(accessor(AccessorRole::Setter)),
        ] {
            result.extend([
                Selection::Concrete(declaration),
                Selection::InterfaceDefault(declaration),
            ]);
        }
        result
    }
}

fn resolve<I: PersistentId>(
    id: DecodedPersistentId<I>,
    known: impl Iterator<Item = I>,
) -> Result<I, &'static str> {
    known
        .into_iter()
        .find(|value| id.verify(*value).is_ok())
        .ok_or("unknown typed source reference")
}

impl PersistentIdResolver<PersistentExactTypeId> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentExactTypeId>,
    ) -> Result<PersistentExactTypeId, Self::Error> {
        self.queries += 1;
        resolve(id, self.inner.inheritance.exacts.keys().copied())
    }
}
impl PersistentIdResolver<PersistentDispatchSlotId> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentDispatchSlotId>,
    ) -> Result<PersistentDispatchSlotId, Self::Error> {
        self.queries += 1;
        resolve(id, self.inner.slots.keys().copied())
    }
}
impl PersistentIdResolver<PersistentFunctionId> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentFunctionId>,
    ) -> Result<PersistentFunctionId, Self::Error> {
        self.queries += 1;
        resolve(id, self.inner.functions.keys().copied())
    }
}
impl PersistentIdResolver<PersistentPropertyAccessorId> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentPropertyAccessorId>,
    ) -> Result<PersistentPropertyAccessorId, Self::Error> {
        self.queries += 1;
        resolve(id, self.inner.accessors.keys().copied())
    }
}
