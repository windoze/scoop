use super::*;
use scoop_identity::{DispatchDeclarationOwner, DispatchRole};
use scoop_wire::{BudgetMeter, WirePath};
use std::collections::BTreeSet;

mod contract;

type Declaration = InheritanceCallableDeclarationV1;

pub(in crate::production::type_semantics) fn project(
    export: &ExportHir,
    inventory: &CanonicalSourceInheritanceInventoriesV1,
    selections: &CanonicalInheritanceSourceSlotSelectionsV1,
    meter: &mut BudgetMeter,
) -> Result<CanonicalInheritanceSourceCallablesV1, Error> {
    let mut required = required(export, inventory, selections, meter)?;
    let mut records = Vec::new();
    for (id, _) in export.functions.iter() {
        work(meter, required.len())?;
        let Some(declaration) = identity(export, id) else {
            continue;
        };
        if required.remove(&declaration) {
            meter
                .check_table_entries(records.len() as u64 + 1, &WirePath::root())
                .map_err(resource)?;
            meter
                .try_reserve_collection_slots(&mut records, 1, &WirePath::root())
                .map_err(resource)?;
            records.push(contract::project(export, id, declaration, meter)?);
        }
    }
    if !required.is_empty() {
        return Err(invalid(
            "a required dispatch callable has no sealed source declaration",
        ));
    }
    CanonicalInheritanceSourceCallablesV1::try_new(records, meter).map_err(Error::SourceInventory)
}

pub(super) fn required(
    export: &ExportHir,
    inventory: &CanonicalSourceInheritanceInventoriesV1,
    selections: &CanonicalInheritanceSourceSlotSelectionsV1,
    meter: &mut BudgetMeter,
) -> Result<BTreeSet<Declaration>, Error> {
    let mut slots = BTreeSet::new();
    let mut declarations = BTreeSet::new();
    for owner in inventory.records() {
        work(meter, 1)?;
        for schema in owner.slot_schemas().records() {
            work(meter, 1)?;
            for slot in schema.slots() {
                insert(&mut slots, *slot, meter)?;
            }
        }
    }
    for record in export.dispatch_slot_identities.records() {
        work(meter, slots.len())?;
        if slots.remove(&record.id()) {
            let declaration = match (record.key().owner(), record.key().role()) {
                (
                    DispatchDeclarationOwner::Function(id),
                    DispatchRole::VirtualMethod | DispatchRole::InterfaceMethod,
                ) => Declaration::Function(id),
                (DispatchDeclarationOwner::Accessor(id), DispatchRole::PropertyGetter) => {
                    Declaration::Getter(id)
                }
                (DispatchDeclarationOwner::Accessor(id), DispatchRole::PropertySetter) => {
                    Declaration::Setter(id)
                }
                _ => {
                    return Err(invalid(
                        "dispatch role disagrees with its sealed declaration identity",
                    ));
                }
            };
            insert(&mut declarations, declaration, meter)?;
        }
    }
    if !slots.is_empty() {
        return Err(invalid("source schema refers to an unsealed dispatch slot"));
    }
    for record in selections.records() {
        work(meter, 1)?;
        let declaration = match record.selection() {
            InheritanceSourceSlotSelectionV1::Abstract => continue,
            InheritanceSourceSlotSelectionV1::Concrete(declaration)
            | InheritanceSourceSlotSelectionV1::InterfaceDefault(declaration) => declaration,
        };
        insert(&mut declarations, declaration, meter)?;
    }
    Ok(declarations)
}

fn identity(export: &ExportHir, function: FunctionId) -> Option<Declaration> {
    match &export.function_identities[function] {
        HirFunctionIdentity::Source(HirSourceFunctionIdentity::Plain(record)) => {
            Some(Declaration::Function(record.id()))
        }
        HirFunctionIdentity::PropertyAccessor(HirPropertyAccessorFunction::Getter(id)) => Some(
            Declaration::Getter(export.property_accessor_identities[*id].id()),
        ),
        HirFunctionIdentity::PropertyAccessor(HirPropertyAccessorFunction::Setter(id)) => Some(
            Declaration::Setter(export.property_accessor_identities[*id].id()),
        ),
        HirFunctionIdentity::Source(HirSourceFunctionIdentity::Generic(_))
        | HirFunctionIdentity::LexicalGenerated(_)
        | HirFunctionIdentity::Initialization { .. }
        | HirFunctionIdentity::DerivedEquality(_) => None,
    }
}

fn insert<T: Ord>(set: &mut BTreeSet<T>, value: T, meter: &mut BudgetMeter) -> Result<(), Error> {
    work(meter, set.len())?;
    meter
        .charge_collection_slots(1, &WirePath::root())
        .map_err(resource)?;
    if !set.contains(&value) {
        meter
            .check_table_entries(set.len() as u64 + 1, &WirePath::root())
            .map_err(resource)?;
        work(meter, set.len())?;
        set.insert(value);
    }
    Ok(())
}
fn work(meter: &mut BudgetMeter, length: usize) -> Result<(), Error> {
    meter
        .charge_work(u64::from(length.max(1).ilog2()) + 1, &WirePath::root())
        .map_err(resource)
}
fn resource(error: scoop_wire::WireError) -> Error {
    Error::SourceInventory(SourceInventoryError::Resource(error))
}
fn invalid(reason: impl Into<String>) -> Error {
    Error::InvalidSourceDeclaration(reason.into())
}
