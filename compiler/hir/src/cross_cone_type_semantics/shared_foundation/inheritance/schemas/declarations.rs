use super::*;
use scoop_wire::WirePath;

pub(super) fn collect<'a>(
    context: &mut SchemaDeclarations<'a>,
    provider: CheckedSharedTypeFoundationV1<'a>,
    dependencies: &[CheckedSharedTypeFoundationV1<'a>],
    graph: &CheckedNominalInheritanceGraphV1<'_>,
) -> Result<(), Error> {
    let metadata = provider.metadata;
    let path = WirePath::root();
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
        let order = declaration.declaration_details().dispatch_order();

        context.orders.insert(owner, order);
        context.selections.insert(
            owner,
            declaration.declaration_details().dispatch_selections(),
        );
        context.schemas.insert(owner, record.slot_schemas());
        for slot in order
            .declared_slots()
            .chain(record.slots().records().iter().map(|slot| slot.slot()))
        {
            keys::collect(context, metadata, slot)?;
        }
        if let NominalDispatchOrderV1::Interface { parents, members } = order {
            let types = MetadataTypes {
                current: metadata,
                dependencies,
            };
            let mut exacts = Vec::new();
            scoop_wire::allocation::try_reserve(&mut exacts, parents.len(), &path)?;
            for parent in parents {
                exacts.push(types.exact(parent)?);
            }
            for member in members {
                for overridden in member.overrides().values() {
                    keys::collect(context, metadata, *overridden)?;
                }
            }

            let source = InterfaceSourceDispatchV1::try_new(owner, exacts, members.clone())
                .map_err(|error| Error::Key(error.to_string()))?;
            context.interfaces.insert(owner, source);
        }
    }
    Ok(())
}
