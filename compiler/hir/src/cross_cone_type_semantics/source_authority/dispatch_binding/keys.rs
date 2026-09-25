use super::*;
use scoop_identity::{
    CborIdentityKey, CborIdentityRecord, DispatchDeclarationOwner, DispatchRole, PersistentId,
};

pub(super) fn index<'f, I, K>(
    records: &'f [CborIdentityRecord<I, K>],
    foundation: &BoundTypeFoundationSourcesV1<'f>,
) -> Result<BTreeMap<I, &'f K>, InheritanceDispatchBindingError>
where
    I: PersistentId + 'static,
    K: CborIdentityKey<I> + Eq + Clone + Send + Sync + 'static,
{
    let keys = binding_keys::index(records)?;
    for (id, key) in &keys {
        binding_keys::verify(*id, *key, foundation.identities)?;
    }
    Ok(keys)
}

pub(super) fn declaration(
    slot: PersistentDispatchSlotId,
    bound: &BoundInheritanceDispatchSourcesV1<'_, '_>,
) -> Result<InheritanceCallableDeclarationV1, InheritanceDispatchBindingError> {
    use InheritanceCallableDeclarationV1 as Declaration;
    let key = bound.dispatch_slot_key(slot)?;
    match (key.owner(), key.role()) {
        (
            DispatchDeclarationOwner::Function(id),
            DispatchRole::VirtualMethod | DispatchRole::InterfaceMethod,
        ) => Ok(Declaration::Function(id)),
        (DispatchDeclarationOwner::Accessor(id), DispatchRole::PropertyGetter) => {
            Ok(Declaration::Getter(id))
        }
        (DispatchDeclarationOwner::Accessor(id), DispatchRole::PropertySetter) => {
            Ok(Declaration::Setter(id))
        }
        _ => Err(InheritanceDispatchBindingError::SlotRole(slot)),
    }
}
