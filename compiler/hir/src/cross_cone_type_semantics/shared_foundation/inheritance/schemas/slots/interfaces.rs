use super::*;
use crate::InheritanceSlotSchemaRoleV1;

pub(super) fn select(
    data: &Data<'_>,
    schemas: &SchemaDeclarations<'_>,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    owner: PersistentExactTypeId,
    slot: PersistentDispatchSlotId,
    root: &Member<'_>,
    meter: &mut BudgetMeter,
) -> Result<Selection, Error> {
    contracts::lookup(schemas.schemas.len(), meter)?;
    let tables = schemas.schemas.get(&owner).ok_or(Error::SlotOrder(owner))?;
    let mut candidates = Vec::new();
    for table in tables.records() {
        let InheritanceSlotSchemaRoleV1::Interface { interface_exact } = table.role() else {
            continue;
        };
        for declaration in data.owner_members(interface_exact, meter)? {
            let member = data.member(*declaration, meter)?;
            if selection::visible(member, graph, owner, meter)?
                && declarations::matches(member, root, meter)?
            {
                meter.try_reserve_collection_slots(&mut candidates, 1, &WirePath::root())?;
                candidates.push(member);
            }
        }
    }
    let mut winner = None;
    for candidate in &candidates {
        let mut hidden = false;
        for other in &candidates {
            meter.charge_work(1, &WirePath::root())?;
            if other.owner != candidate.owner
                && reaches(graph, other.owner, candidate.owner, meter)?
            {
                hidden = true;
                break;
            }
        }
        if !hidden && candidate.source.modality() == CallableModalityV1::InterfaceDefault {
            if winner.is_some() {
                return Err(selection_error(owner, slot));
            }
            winner = Some(candidate.declaration);
        }
    }
    Ok(winner
        .map(Selection::InterfaceDefault)
        .unwrap_or(Selection::Abstract))
}

fn reaches(
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    from: PersistentExactTypeId,
    target: PersistentExactTypeId,
    meter: &mut BudgetMeter,
) -> Result<bool, Error> {
    let path = WirePath::root();
    let mut pending = Vec::new();
    let mut visited = BTreeSet::new();
    meter.try_reserve_collection_slots(&mut pending, 1, &path)?;
    pending.push((from, 1));
    while let Some((exact, depth)) = pending.pop() {
        meter.check_semantic_depth(depth, &path)?;
        contracts::lookup(visited.len(), meter)?;
        if visited.contains(&exact) {
            continue;
        }
        if exact == target {
            return Ok(true);
        }
        meter.charge_collection_slots(1, &path)?;
        visited.insert(exact);
        let node = graph.get(exact).ok_or(Error::SlotOrder(exact))?;
        let parents = node.edges().direct_interfaces();
        meter.charge_edges(parents.len() as u64, &path)?;
        meter.try_reserve_collection_slots(&mut pending, parents.len(), &path)?;
        pending.extend(parents.iter().map(|parent| (*parent, depth + 1)));
    }
    Ok(false)
}
