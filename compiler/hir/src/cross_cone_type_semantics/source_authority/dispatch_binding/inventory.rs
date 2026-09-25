use super::*;
use scoop_identity::{ExactTypeKey, SourceDeclarationKind};

pub(super) fn validate(
    bound: &BoundInheritanceDispatchSourcesV1<'_, '_>,
) -> Result<(), InheritanceDispatchBindingError> {
    use InheritanceDispatchBindingError as Error;
    let edges = bound
        .foundation
        .source()
        .entries()
        .local_inheritance_edges
        .records();

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
    }

    if !interfaces.iter().copied().eq(bound
        .interfaces
        .records()
        .iter()
        .map(InterfaceSourceDispatchV1::owner))
    {
        return Err(Error::Inventory("interface owners"));
    }
    for interface in bound.interfaces.records() {
        for parent in interface.parents() {
            bound.interface_dispatch_source(*parent)?;
        }

        for member in interface.members() {
            bound.dispatch_slot_key(member.slot())?;

            for slot in member.overrides().values() {
                bound.dispatch_slot_key(*slot)?;
            }
        }
    }

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

    for selection in bound.selections.records() {
        let target = match selection.selection() {
            InheritanceSourceSlotSelectionV1::Abstract => continue,
            InheritanceSourceSlotSelectionV1::Concrete(target)
            | InheritanceSourceSlotSelectionV1::InterfaceDefault(target) => target,
        };
        required_callables.insert(target);
    }

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
