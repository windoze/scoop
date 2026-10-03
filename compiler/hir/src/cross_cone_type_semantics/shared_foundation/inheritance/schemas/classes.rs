use super::*;
use crate::{DirectClassBaseV1, InheritanceSlotSchemaRoleV1};

pub(super) fn validate(
    owner: PersistentExactTypeId,
    order: &NominalDispatchOrderV1,
    context: &SchemaDeclarations<'_>,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
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
    scoop_wire::allocation::try_reserve(&mut expected, prefix.len() + slots.len(), &path)?;
    expected.extend_from_slice(prefix);

    let mut seen: BTreeSet<_> = prefix.iter().copied().collect();
    for slot in slots {
        if seen.insert(*slot) {
            expected.push(*slot);
        }
    }
    let schema = context
        .schemas(owner)?
        .get(role)
        .ok_or(Error::SlotOrder(owner))?;

    if expected != schema.slots() {
        return Err(Error::SlotOrder(owner));
    }
    Ok(())
}
