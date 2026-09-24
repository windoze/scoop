use super::*;
use crate::DirectClassBaseV1;

pub(super) fn select(
    data: &Data<'_>,
    schemas: &SchemaDeclarations<'_>,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    owner: PersistentExactTypeId,
    slot: &InheritanceSlotContractV1,
    meter: &mut BudgetMeter,
) -> Result<Selection, Error> {
    let root = data.member(slot.declaration(), meter)?;
    let owner_kind = kind(graph, owner)?;
    if owner_kind == SourceDeclarationKind::Interface {
        return Ok(selected(root));
    }
    if kind(graph, root.owner)? == SourceDeclarationKind::Class {
        return virtual_slot(data, graph, owner, slot.slot(), root, meter);
    }
    let mut current = Some(owner);
    let mut depth = 1;
    while let Some(exact) = current {
        meter.check_semantic_depth(depth, &WirePath::root())?;
        let mut found = None;
        for declaration in data.owner_members(exact, meter)? {
            let member = data.member(*declaration, meter)?;
            if visible(member, graph, owner, meter)? && declarations::matches(member, root, meter)?
            {
                if found.is_some() {
                    return Err(selection_error(owner, slot.slot()));
                }
                found = Some(member);
            }
        }
        if let Some(member) = found {
            return Ok(selected(member));
        }
        current = base(graph, exact)?;
        depth += 1;
    }
    interfaces::select(data, schemas, graph, owner, slot.slot(), root, meter)
}

fn virtual_slot(
    data: &Data<'_>,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    owner: PersistentExactTypeId,
    slot: PersistentDispatchSlotId,
    root: &Member<'_>,
    meter: &mut BudgetMeter,
) -> Result<Selection, Error> {
    let mut current = Some(owner);
    let mut chosen = None;
    let mut depth = 1;
    while let Some(exact) = current {
        meter.check_semantic_depth(depth, &WirePath::root())?;
        let mut found = None;
        for declaration in data.owner_members(exact, meter)? {
            let member = data.member(*declaration, meter)?;
            let slots = member.source.slot_relations().values();
            contracts::lookup(slots.len(), meter)?;
            if slots.binary_search(&slot).is_err() {
                continue;
            }
            if found.is_some()
                || !declarations::matches(member, root, meter)?
                || (chosen.is_some() && member.source.modality() == CallableModalityV1::Final)
            {
                return Err(selection_error(owner, slot));
            }
            found = Some(member);
        }
        if chosen.is_none() {
            chosen = found.map(selected);
        }
        current = base(graph, exact)?;
        depth += 1;
    }
    chosen.ok_or_else(|| selection_error(owner, slot))
}

fn kind(
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    exact: PersistentExactTypeId,
) -> Result<SourceDeclarationKind, Error> {
    let node = graph.get(exact).ok_or(Error::SlotOrder(exact))?;
    Ok(graph
        .source(node.source())
        .ok_or(Error::InheritanceSource(node.source()))?
        .key
        .declaration_kind())
}

pub(super) fn visible(
    member: &Member<'_>,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    owner: PersistentExactTypeId,
    meter: &mut BudgetMeter,
) -> Result<bool, Error> {
    meter.charge_work(1, &WirePath::root())?;
    match member.source.declared_visibility() {
        DeclaredVisibilityV1::Public | DeclaredVisibilityV1::Protected => Ok(true),
        DeclaredVisibilityV1::Private => Ok(false),
        DeclaredVisibilityV1::Internal => {
            let node = graph.get(owner).ok_or(Error::SlotOrder(owner))?;
            let source = graph
                .source(node.source())
                .ok_or(Error::InheritanceSource(node.source()))?;
            Ok(member.metadata.provider == source.key.origin())
        }
    }
}

fn base(
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    exact: PersistentExactTypeId,
) -> Result<Option<PersistentExactTypeId>, Error> {
    let node = graph.get(exact).ok_or(Error::SlotOrder(exact))?;
    Ok(match node.edges().direct_base() {
        DirectClassBaseV1::NoClassBase => None,
        DirectClassBaseV1::ClassBase { exact } => Some(exact),
    })
}
