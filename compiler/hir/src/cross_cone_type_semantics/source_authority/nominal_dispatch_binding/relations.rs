use super::*;
use scoop_identity::DispatchDeclarationOwner;

pub(super) fn validate(
    bound: &BoundNominalDispatchSourcesV1<'_, '_, '_, '_, '_>,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let members = bound.parameters.members();
    for selection in bound.slots.dispatch.selections.records() {
        meter.charge_nodes(1, &WirePath::root())?;
        meter.charge_work(1, &WirePath::root())?;
        let declaration = match selection.selection() {
            InheritanceSourceSlotSelectionV1::Abstract => continue,
            InheritanceSourceSlotSelectionV1::Concrete(id)
            | InheritanceSourceSlotSelectionV1::InterfaceDefault(id) => id,
        };
        query(bound.slots.dispatch.selections.records().len(), meter)?;
        let key = bound
            .slots
            .dispatch_slot_key(selection.slot())
            .map_err(Error::from_slot)?;
        let root = match key.owner() {
            DispatchDeclarationOwner::Function(id) => CallableTemplateOrigin::Function(id),
            DispatchDeclarationOwner::Accessor(id) => CallableTemplateOrigin::Accessor(id),
        };
        query(members.callables().records().len(), meter)?;
        let root = members
            .callable_source(root)
            .map_err(NominalNestedBindingError::from)?;
        query(members.nominals.table().records().len(), meter)?;
        let owner = members
            .nominals
            .nominal_source(root.payload().owner())
            .map_err(NominalNestedBindingError::from)?;
        // Interface implementation choices are receiver-specific and may
        // select a Direct method without any declaration-side virtual slot.
        if owner.kind() == PublicNominalKindV1::Interface {
            continue;
        }
        if owner.kind() != PublicNominalKindV1::Class {
            return Err(Error::Callable {
                declaration,
                field: "slot root owner",
            });
        }
        query(members.callables().records().len(), meter)?;
        let source = members
            .callable_source(origin(declaration))
            .map_err(NominalNestedBindingError::from)?;
        query(source.payload().slot_relations().slots().len(), meter)?;
        if source
            .payload()
            .slot_relations()
            .slots()
            .binary_search(&selection.slot())
            .is_err()
        {
            return Err(Error::Callable {
                declaration,
                field: "selected slot relation",
            });
        }
    }
    Ok(())
}
