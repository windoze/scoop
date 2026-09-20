use super::*;
use scoop_identity::DispatchDeclarationOwner;

mod abstract_override;

pub(super) fn validate(
    dispatch: &BoundInheritanceDispatchSourcesV1<'_, '_>,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    source: &SourceInheritanceInventoryV1,
    record: &NominalSupportPropertyInterfaceV1,
    meter: &mut BudgetMeter,
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
        meter.charge_work(1, &WirePath::root())?;
        for slot in schema.slots() {
            query(
                dispatch
                    .foundation
                    .foundation
                    .as_canonical()
                    .counts()
                    .dispatch_slots,
                meter,
            )?;
            let key = dispatch.dispatch_slot_key(*slot)?;
            let own = matches!(key.owner(), DispatchDeclarationOwner::Accessor(id)
                if id == getter || Some(id) == setter);
            let selected = if schema.role() == InheritanceSlotSchemaRoleV1::ClassVtable {
                query(dispatch.selections.records().len(), meter)?;
                match dispatch.selection(source.owner(), *slot)? {
                    InheritanceSourceSlotSelectionV1::Abstract => abstract_override::matches(
                        dispatch,
                        graph,
                        source.owner(),
                        record,
                        key,
                        meter,
                    )?,
                    InheritanceSourceSlotSelectionV1::Concrete(target)
                    | InheritanceSourceSlotSelectionV1::InterfaceDefault(target) => belongs(target),
                }
            } else {
                false
            };
            if own || selected {
                query(expected.len(), meter)?;
                if !expected.contains(slot) {
                    meter.check_table_entries(expected.len() as u64 + 1, &WirePath::root())?;
                    meter.charge_collection_slots(1, &WirePath::root())?;
                    query(expected.len(), meter)?;
                    expected.insert(*slot);
                }
            }
        }
    }
    meter.charge_work(
        payload.slot_relations().slots().len() as u64,
        &WirePath::root(),
    )?;
    if !expected
        .iter()
        .copied()
        .eq(payload.slot_relations().slots().iter().copied())
    {
        return Err(InheritancePropertyBindingError::Slots(record.declaration()));
    }
    Ok(())
}
