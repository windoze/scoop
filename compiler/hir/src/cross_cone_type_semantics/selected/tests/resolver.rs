use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Family {
    Cone,
    Exact,
    Constructor,
    Variant,
    Function,
    Accessor,
    Slot,
    Object,
    Type,
    GenericType,
}
pub(super) struct Resolver<'a> {
    pub fixture: &'a Fixture,
    pub calls: Vec<Family>,
}
macro_rules! resolve_family {
    ($id:ty, $family:ident, $fixture:ident, $known:expr) => {
        impl PersistentIdResolver<$id> for Resolver<'_> {
            type Error = Family;
            fn resolve(&mut self, decoded: DecodedPersistentId<$id>) -> Result<$id, Self::Error> {
                self.calls.push(Family::$family);
                let $fixture = self.fixture;
                $known
                    .into_iter()
                    .find_map(|known| decoded.verify(known).ok())
                    .ok_or(Family::$family)
            }
        }
    };
}
resolve_family!(ConeIdentity, Cone, f, [f.provider, f.alternate]);
resolve_family!(
    PersistentExactTypeId,
    Exact,
    f,
    [
        f.owner,
        f.derived,
        f.enumeration,
        f.interface,
        f.object_exact
    ]
);
resolve_family!(PersistentConstructorId, Constructor, f, [f.constructor]);
resolve_family!(PersistentEnumVariantId, Variant, f, [f.variant]);
resolve_family!(PersistentFunctionId, Function, f, [f.function]);
resolve_family!(
    PersistentPropertyAccessorId,
    Accessor,
    f,
    [f.getter, f.setter]
);
resolve_family!(PersistentDispatchSlotId, Slot, f, [f.slot]);
resolve_family!(PersistentObjectValueId, Object, f, [f.object]);
resolve_family!(PersistentTypeId, Type, f, [f.enumeration_id]);
resolve_family!(PersistentGenericTypeId, GenericType, f, [f.generic]);
