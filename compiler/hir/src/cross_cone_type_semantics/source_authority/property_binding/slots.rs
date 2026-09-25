use super::*;
use scoop_identity::DispatchDeclarationOwner;

mod abstract_override;

pub(super) fn validate(
    dispatch: &BoundInheritanceDispatchSourcesV1<'_, '_>,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    source: &SourceInheritanceInventoryV1,
    record: &NominalSupportPropertyInterfaceV1,
) -> Result<(), InheritancePropertyBindingError> {
    let payload = payload(record)?;
    let getter = payload.getter();
    let setter = match payload.mutability() {
        ProtectedPropertyMutabilityV1::ReadOnly => None,
        ProtectedPropertyMutabilityV1::ReadWrite { setter, .. } => Some(*setter),
    };
    let belongs = |declaration| match declaration {
        InheritanceCallableDeclarationV1::Getter(id) => id == getter,
        InheritanceCallableDeclarationV1::Setter(id) => Some(id) == setter,
        InheritanceCallableDeclarationV1::Function(_) => false,
    };
    let mut expected = BTreeSet::new();
    for schema in source.slot_schemas().records() {
        for slot in schema.slots() {
            let key = dispatch.dispatch_slot_key(*slot)?;
            let own = matches!(key.owner(), DispatchDeclarationOwner::Accessor(id)
                if id == getter || Some(id) == setter);
            let selected = if schema.role() == InheritanceSlotSchemaRoleV1::ClassVtable {
                match dispatch.selection(source.owner(), *slot)? {
                    InheritanceSourceSlotSelectionV1::Abstract => {
                        abstract_override::matches(dispatch, graph, source.owner(), record, key)?
                    }
                    InheritanceSourceSlotSelectionV1::Concrete(target)
                    | InheritanceSourceSlotSelectionV1::InterfaceDefault(target) => belongs(target),
                }
            } else {
                false
            };
            if own || selected {
                expected.insert(*slot);
            }
        }
    }

    if !expected
        .iter()
        .copied()
        .eq(payload.slot_relations().slots().iter().copied())
    {
        return Err(InheritancePropertyBindingError::Slots(record.declaration()));
    }
    Ok(())
}
