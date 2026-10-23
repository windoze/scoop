//! Emit one static array per maximal interface and alias complete parent views.

use super::*;

pub(super) fn tables(
    module: &mir::Module,
    class: mir::ClassId,
    owner: &lir::TypeDescriptorIdentity,
    refs: &TypeDescriptorRefs,
    functions: &HashMap<mir::FunctionId, lir::LocalFunctionRef>,
    external: &HashMap<mir::ExternalCallableUseId, lir::ExternalCallableId>,
) -> StorageResult<Vec<lir::ItableRecord>> {
    let layouts = &refs.interface_layouts;
    let logical = module.classes[class]
        .itables
        .iter()
        .filter(|table| refs.interfaces.contains_key(&table.interface))
        .map(|table| {
            (
                table.interface,
                table
                    .slots
                    .iter()
                    .map(|slot| dispatch_entry(slot, functions, external))
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<HashMap<_, _>>();
    let identities = logical
        .keys()
        .map(|&interface| {
            let exact = exact_type_record(module, &mir::Type::Interface(interface)).id();
            let table = scoop_identity::PersistentDispatchTableId::from_key(
                &scoop_identity::DispatchTableKey::itable(owner.exact_type(), exact),
            )
            .expect("exact interface tables have stable identities");
            (interface, (exact, table))
        })
        .collect::<HashMap<_, _>>();
    let mut roots = logical
        .keys()
        .copied()
        .filter(|interface| {
            !layouts.get(*interface).slots.is_empty()
                && !logical
                    .keys()
                    .any(|child| layouts.get(*child).parents.contains_key(interface))
        })
        .collect::<Vec<_>>();
    roots.sort_unstable_by_key(|interface| identities[interface].0);
    let mut tables = Vec::with_capacity(logical.len());
    for (&interface, methods) in &logical {
        let (exact, _) = identities[&interface];
        let table =
            lir::ItableRecord::new(owner, exact, refs.interfaces[&interface], methods.clone())
                .expect("validated exact types derive an interface table identity");
        let layout = layouts.get(interface);
        if layout.slots.is_empty() {
            tables.push(table);
            continue;
        }
        let (root, offset) = roots
            .iter()
            .find_map(|&root| {
                layouts
                    .projection(root, interface)
                    .map(|offset| (root, offset))
            })
            .ok_or(StorageLoweringError::InvalidRepresentation(
                "missing interface table storage",
            ))?;
        let mut physical = Vec::with_capacity(layout.slots.len());
        let mut sources = Vec::with_capacity(layout.slots.len());
        for source in &layout.slots {
            let entry = logical
                .get(&source.interface)
                .and_then(|table| table.get(source.method as usize))
                .ok_or(StorageLoweringError::InvalidRepresentation(
                    "missing inherited interface slot",
                ))?;
            physical.push(*entry);
            sources.push(lir::DispatchSlotSource {
                table: identities[&source.interface].1,
                method: source.method,
            });
        }
        tables.push(table.with_layout(
            lir::DispatchTableLayout::new(identities[&root].1, offset, sources),
            physical,
        ));
    }
    tables.sort_unstable_by_key(|table| table.identity_record().key().interface());
    Ok(tables)
}
