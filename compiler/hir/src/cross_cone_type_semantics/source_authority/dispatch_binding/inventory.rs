use super::*;
use scoop_identity::{ExactTypeKey, SourceDeclarationKind};

pub(super) fn validate(
    bound: &BoundInheritanceDispatchSourcesV1<'_, '_>,
    meter: &mut BudgetMeter,
) -> Result<(), InheritanceDispatchBindingError> {
    use InheritanceDispatchBindingError as Error;
    let edges = bound
        .foundation
        .source()
        .entries()
        .local_inheritance_edges
        .records();
    charge(edges.len(), meter)?;
    charge(bound.inventory.records().len(), meter)?;
    if !bound
        .inventory
        .owners()
        .values()
        .iter()
        .copied()
        .eq(edges.iter().map(NominalInheritanceEdgesV1::owner))
    {
        return Err(Error::Inventory("inheritance owners"));
    }
    let mut interfaces = BTreeSet::new();
    let mut slot_count = 0usize;
    let entries = bound.foundation.source().entries();
    charge_queries(
        bound.inventory.records().len(),
        entries.exact_keys.values().len(),
        meter,
    )?;
    charge_queries(
        bound.inventory.records().len(),
        entries.sources.records().len(),
        meter,
    )?;
    for owner in bound.inventory.records() {
        let ExactTypeKey::Nominal(id) = bound.foundation.exact_type_key(owner.owner())? else {
            return Err(Error::MissingOwner(owner.owner()));
        };
        if bound
            .foundation
            .nominal_key(SourceNominalId::Concrete(*id))?
            .declaration_kind()
            == SourceDeclarationKind::Interface
        {
            interfaces.insert(owner.owner());
        }
        charge(owner.slot_schemas().records().len(), meter)?;
        for schema in owner.slot_schemas().records() {
            slot_count = slot_count.saturating_add(schema.slots().len());
        }
    }
    charge(bound.interfaces.records().len(), meter)?;
    if !interfaces.iter().copied().eq(bound
        .interfaces
        .records()
        .iter()
        .map(InterfaceSourceDispatchV1::owner))
    {
        return Err(Error::Inventory("interface owners"));
    }
    for interface in bound.interfaces.records() {
        charge(interface.parents().len(), meter)?;
        charge_queries(
            interface.parents().len(),
            bound.interfaces.records().len(),
            meter,
        )?;
        for parent in interface.parents() {
            bound.interface_dispatch_source(*parent)?;
        }
        charge(interface.members().len(), meter)?;
        charge_queries(interface.members().len(), bound.slots.len(), meter)?;
        for member in interface.members() {
            bound.dispatch_slot_key(member.slot())?;
            charge(member.overrides().values().len(), meter)?;
            charge_queries(member.overrides().values().len(), bound.slots.len(), meter)?;
            for slot in member.overrides().values() {
                bound.dispatch_slot_key(*slot)?;
            }
        }
    }
    charge(slot_count, meter)?;
    charge_queries(slot_count, bound.slots.len(), meter)?;
    let mut selections = BTreeSet::new();
    let mut required_callables = BTreeSet::new();
    for owner in bound.inventory.records() {
        for schema in owner.slot_schemas().records() {
            for slot in schema.slots() {
                selections.insert((owner.owner(), *slot));
                required_callables.insert(keys::declaration(*slot, bound)?);
            }
        }
    }
    charge(bound.selections.records().len(), meter)?;
    if !selections.iter().copied().eq(bound
        .selections
        .records()
        .iter()
        .map(|record| (record.owner(), record.slot())))
    {
        return Err(Error::Inventory("slot selections"));
    }
    // Each target joins the same required set, including slots shared by
    // multiple interface roles. The bound includes no unused callable records.
    charge(
        slot_count.saturating_add(bound.selections.records().len()),
        meter,
    )?;
    for selection in bound.selections.records() {
        let target = match selection.selection() {
            InheritanceSourceSlotSelectionV1::Abstract => continue,
            InheritanceSourceSlotSelectionV1::Concrete(target)
            | InheritanceSourceSlotSelectionV1::InterfaceDefault(target) => target,
        };
        required_callables.insert(target);
    }
    charge(bound.callables.records().len(), meter)?;
    if !required_callables.iter().copied().eq(bound
        .callables
        .records()
        .iter()
        .map(InheritanceSourceCallableV1::declaration))
    {
        return Err(Error::Inventory("dispatch callables"));
    }
    Ok(())
}
