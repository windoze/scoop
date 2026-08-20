use super::*;
use crate::NominalInterfaceRecordV1;
use scoop_wire::WirePath;

pub(super) fn collect<'a>(
    context: &mut SchemaDeclarations<'a>,
    provider: CheckedSharedTypeFoundationV1<'a>,
    dependencies: &[CheckedSharedTypeFoundationV1<'a>],
    graph: &CheckedNominalInheritanceGraphV1<'_>,
) -> Result<(), Error> {
    let metadata = provider.metadata;
    metadata
        .public
        .nominal_interfaces()
        .validate_dispatch_declarations(metadata.public.callable_interfaces())
        .map_err(Error::DispatchDeclarations)?;
    for record in provider.section.inheritance().records() {
        let owner = record.owner();
        let node = graph.get(owner).ok_or(Error::SlotOrder(owner))?;
        let declaration = metadata
            .public
            .nominal_interfaces()
            .declaration(node.source())
            .ok_or(Error::InheritanceSource(node.source()))?;
        insert(
            context,
            MetadataTypes {
                current: metadata,
                dependencies,
            },
            owner,
            declaration,
            &[],
        )?;
        context
            .schemas
            .insert(owner, Cow::Borrowed(record.slot_schemas()));
        for slot in record.slots().records() {
            keys::collect(context, metadata, slot.slot())?;
        }
    }
    Ok(())
}

pub(super) fn collect_applications<'a>(
    context: &mut SchemaDeclarations<'a>,
    current: CheckedSharedTypeFoundationV1<'a>,
    dependencies: &[CheckedSharedTypeFoundationV1<'a>],
    sources: &Context<'_>,
) -> Result<(), Error> {
    let types = MetadataTypes {
        current: current.metadata,
        dependencies,
    };
    for owner in sources.edges.keys().copied() {
        if context.orders.contains_key(&owner) {
            continue;
        }
        let application = types.applied_nominal(owner)?;
        insert(
            context,
            types,
            owner,
            application.declaration,
            &application.bindings(),
        )?;
    }
    Ok(())
}

fn insert<'a>(
    context: &mut SchemaDeclarations<'a>,
    types: MetadataTypes<'a, '_>,
    owner: PersistentExactTypeId,
    declaration: &'a NominalInterfaceRecordV1,
    bindings: &[Vec<PersistentExactTypeId>],
) -> Result<(), Error> {
    let order = declaration.declaration_details().dispatch_order();
    context.orders.insert(owner, order);
    let mut selections = BTreeMap::new();
    for selection in declaration
        .declaration_details()
        .dispatch_selections()
        .records()
        .iter()
    {
        let role = match selection.role() {
            crate::NominalDispatchSelectionRoleV1::ClassVtable => {
                crate::InheritanceSlotSchemaRoleV1::ClassVtable
            }
            crate::NominalDispatchSelectionRoleV1::Interface { interface } => {
                crate::InheritanceSlotSchemaRoleV1::Interface {
                    interface_exact: types.exact_with_bindings(interface, bindings)?,
                }
            }
        };
        let choice = AppliedDispatchSelection {
            selection: selection.selection(),
            receiver: types.exact_with_bindings(selection.receiver(), bindings)?,
        };
        if let Some(previous) = selections.insert((role, selection.slot()), choice)
            && previous != choice
        {
            return Err(Error::SlotSelectionInventory(owner));
        }
    }
    context.selections.insert(owner, selections);
    for slot in order.declared_slots() {
        keys::collect(context, types.current, slot)?;
    }
    if let NominalDispatchOrderV1::Interface { parents, members } = order {
        let mut exacts = Vec::new();
        scoop_wire::allocation::try_reserve(&mut exacts, parents.len(), &WirePath::root())?;
        for parent in parents {
            exacts.push(types.exact_with_bindings(parent, bindings)?);
        }
        for member in members {
            for overridden in member.overrides().values() {
                keys::collect(context, types.current, *overridden)?;
            }
        }
        context.interface_parents.insert(owner, exacts);
    }
    Ok(())
}
