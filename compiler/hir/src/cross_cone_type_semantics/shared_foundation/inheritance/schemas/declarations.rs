use super::*;
use scoop_wire::WirePath;

pub(super) fn collect<'a>(
    context: &mut SchemaDeclarations<'a>,
    provider: CheckedSharedTypeFoundationV1<'a>,
    dependencies: &[CheckedSharedTypeFoundationV1<'a>],
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let metadata = provider.metadata;
    let path = WirePath::root();
    metadata
        .public
        .nominal_interfaces()
        .validate_dispatch_declarations(metadata.public.callable_interfaces(), meter)
        .map_err(Error::DispatchDeclarations)?;
    for record in provider.section.inheritance().records() {
        let owner = record.owner();
        let node = graph.get(owner).ok_or(Error::SlotOrder(owner))?;
        contracts::lookup(
            metadata.public.nominal_interfaces().declaration_count(),
            meter,
        )?;
        let declaration = metadata
            .public
            .nominal_interfaces()
            .declaration(node.source())
            .ok_or(Error::InheritanceSource(node.source()))?;
        let order = declaration.declaration_details().dispatch_order();
        meter.charge_collection_slots(2, &path)?;
        contracts::lookup(context.orders.len(), meter)?;
        context.orders.insert(owner, order);
        context.schemas.insert(owner, record.slot_schemas());
        for slot in order
            .declared_slots()
            .chain(record.slots().records().iter().map(|slot| slot.slot()))
        {
            keys::collect(context, metadata, slot, meter)?;
        }
        if let NominalDispatchOrderV1::Interface { parents, members } = order {
            let types = MetadataTypes {
                current: metadata,
                dependencies,
            };
            let mut exacts = Vec::new();
            meter.try_reserve_collection_slots(&mut exacts, parents.len(), &path)?;
            for parent in parents {
                exacts.push(types.exact(parent, 1, meter)?);
            }
            for member in members {
                for overridden in member.overrides().values() {
                    keys::collect(context, metadata, *overridden, meter)?;
                }
                let length = scoop_wire::encoded_length(member)
                    .map_err(|error| Error::Key(error.to_string()))?;
                meter.charge_work(length, &path)?;
                meter.charge_owned_bytes(length, &path)?;
                meter.charge_collection_slots(member.overrides().values().len() as u64, &path)?;
            }
            meter.charge_collection_slots(members.len() as u64 + 1, &path)?;
            let source = InterfaceSourceDispatchV1::try_new(owner, exacts, members.clone(), meter)
                .map_err(|error| Error::Key(error.to_string()))?;
            context.interfaces.insert(owner, source);
        }
    }
    Ok(())
}
