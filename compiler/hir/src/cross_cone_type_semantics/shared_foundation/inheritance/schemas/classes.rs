use super::*;
use crate::{DirectClassBaseV1, InheritanceSlotSchemaRoleV1};

pub(super) fn validate(
    owner: PersistentExactTypeId,
    order: &NominalDispatchOrderV1,
    context: &SchemaDeclarations<'_>,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let NominalDispatchOrderV1::Class { slots } = order else {
        return Ok(());
    };
    let role = InheritanceSlotSchemaRoleV1::ClassVtable;
    let node = graph.get(owner).ok_or(Error::SlotOrder(owner))?;
    let prefix = match node.edges().direct_base() {
        DirectClassBaseV1::NoClassBase => &[][..],
        DirectClassBaseV1::ClassBase { exact } => context
            .schemas(exact)?
            .get(role)
            .ok_or(Error::SlotOrder(exact))?
            .slots(),
    };
    let path = WirePath::root();
    let mut expected = Vec::new();
    meter.try_reserve_collection_slots(&mut expected, prefix.len() + slots.len(), &path)?;
    expected.extend_from_slice(prefix);
    meter.charge_collection_slots(prefix.len() as u64, &path)?;
    meter.charge_work(
        (prefix.len() as u64).saturating_mul(1 + u64::from(prefix.len().max(1).ilog2())),
        &path,
    )?;
    let mut seen: BTreeSet<_> = prefix.iter().copied().collect();
    for slot in slots {
        contracts::lookup(seen.len(), meter)?;
        meter.charge_collection_slots(1, &path)?;
        if seen.insert(*slot) {
            expected.push(*slot);
        }
    }
    let schema = context
        .schemas(owner)?
        .get(role)
        .ok_or(Error::SlotOrder(owner))?;
    meter.charge_work(expected.len() as u64 + schema.slots().len() as u64, &path)?;
    if expected != schema.slots() {
        return Err(Error::SlotOrder(owner));
    }
    Ok(())
}
