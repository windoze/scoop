use super::*;
use scoop_identity::{DispatchDeclarationOwner, DispatchRole};
use scoop_wire::WirePath;
use std::collections::BTreeSet;

mod contract;

type Declaration = InheritanceCallableDeclarationV1;

pub(in crate::production::type_semantics) fn project(
    export: &ExportHir,
    inventory: &CanonicalSourceInheritanceInventoriesV1,
    selections: &CanonicalInheritanceSourceSlotSelectionsV1,
) -> Result<CanonicalInheritanceSourceCallablesV1, Error> {
    let mut required = required(export, inventory, selections)?;
    let mut records = Vec::new();
    for (id, _) in export.functions.iter() {
        let Some(declaration) = identity(export, id) else {
            continue;
        };
        if required.remove(&declaration) {
            scoop_wire::allocation::try_reserve(&mut records, 1, &WirePath::root())
                .map_err(resource)?;
            records.push(contract::project(export, id, declaration)?);
        }
    }
    if !required.is_empty() {
        return Err(invalid(
            "a required dispatch callable has no sealed source declaration",
        ));
    }
    CanonicalInheritanceSourceCallablesV1::try_new(records).map_err(Error::SourceInventory)
}

pub(super) fn required(
    export: &ExportHir,
    inventory: &CanonicalSourceInheritanceInventoriesV1,
    selections: &CanonicalInheritanceSourceSlotSelectionsV1,
) -> Result<BTreeSet<Declaration>, Error> {
    let mut slots = BTreeSet::new();
    let mut declarations = BTreeSet::new();
    for owner in inventory.records() {
        for schema in owner.slot_schemas().records() {
            for slot in schema.slots() {
                insert(&mut slots, *slot)?;
            }
        }
    }
    for record in export.dispatch_slot_identities.records() {
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
            insert(&mut declarations, declaration)?;
        }
    }
    if !slots.is_empty() {
        return Err(invalid("source schema refers to an unsealed dispatch slot"));
    }
    for record in selections.records() {
        let declaration = match record.selection() {
            InheritanceSourceSlotSelectionV1::Abstract => continue,
            InheritanceSourceSlotSelectionV1::Concrete(declaration)
            | InheritanceSourceSlotSelectionV1::InterfaceDefault(declaration) => declaration,
        };
        insert(&mut declarations, declaration)?;
    }
    Ok(declarations)
}

pub(super) fn identity(export: &ExportHir, function: FunctionId) -> Option<Declaration> {
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

fn insert<T: Ord>(set: &mut BTreeSet<T>, value: T) -> Result<(), Error> {
    if !set.contains(&value) {
        set.insert(value);
    }
    Ok(())
}

fn resource(error: scoop_wire::WireError) -> Error {
    Error::SourceInventory(SourceInventoryError::Resource(error))
}
fn invalid(reason: impl Into<String>) -> Error {
    Error::InvalidSourceDeclaration(reason.into())
}
