use super::*;
use crate::{
    DirectClassBaseV1, InheritanceSlotSchemaRoleV1 as Role, InheritanceSlotSchemaV1,
    InterfaceSlotExpansion, NominalInheritanceModalityV1,
};

pub(super) fn complete(
    context: &mut SchemaDeclarations<'_>,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
) -> Result<(), Error> {
    let owners = context.orders.keys().copied().collect::<Vec<_>>();
    let mut expansions = BTreeMap::new();
    for owner in owners {
        application(context, graph, owner, &mut expansions)?;
    }
    Ok(())
}

fn application(
    context: &mut SchemaDeclarations<'_>,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    owner: PersistentExactTypeId,
    expansions: &mut BTreeMap<PersistentExactTypeId, InterfaceSlotExpansion>,
) -> Result<(), Error> {
    if context.schemas.contains_key(&owner) {
        return Ok(());
    }
    let node = graph.get(owner).ok_or(Error::SlotOrder(owner))?;
    if let DirectClassBaseV1::ClassBase { exact } = node.edges().direct_base() {
        application(context, graph, exact, expansions)?;
    }
    for interface in node.edges().direct_interfaces() {
        application(context, graph, *interface, expansions)?;
    }
    let mut schemas = Vec::new();
    if matches!(
        context.orders[&owner],
        NominalDispatchOrderV1::Interface { .. }
    ) {
        expand_interface(context, owner, expansions)?;
        schemas.push(schema(
            owner,
            Role::Interface {
                interface_exact: owner,
            },
            expansions[&owner].slots().collect(),
        )?);
    } else {
        if let NominalDispatchOrderV1::Class { slots } = context.orders[&owner] {
            let mut sequence = match node.edges().direct_base() {
                DirectClassBaseV1::NoClassBase => Vec::new(),
                DirectClassBaseV1::ClassBase { exact } => context
                    .schemas(exact)?
                    .get(Role::ClassVtable)
                    .ok_or(Error::SlotOrder(exact))?
                    .slots()
                    .to_vec(),
            };
            let mut seen: BTreeSet<_> = sequence.iter().copied().collect();
            sequence.extend(slots.iter().copied().filter(|slot| seen.insert(*slot)));
            schemas.push(schema(owner, Role::ClassVtable, sequence)?);
        }
        let mut interfaces = BTreeSet::new();
        interface_closure(graph, owner, &mut BTreeSet::new(), &mut interfaces)?;
        for interface_exact in interfaces {
            expand_interface(context, interface_exact, expansions)?;
            schemas.push(schema(
                owner,
                Role::Interface { interface_exact },
                expansions[&interface_exact].slots().collect(),
            )?);
        }
    }
    context.schemas.insert(
        owner,
        Cow::Owned(
            CanonicalInheritanceSlotSchemasV1::try_new(schemas)
                .map_err(|_| Error::SlotOrder(owner))?,
        ),
    );
    Ok(())
}

fn expand_interface(
    context: &SchemaDeclarations<'_>,
    owner: PersistentExactTypeId,
    expansions: &mut BTreeMap<PersistentExactTypeId, InterfaceSlotExpansion>,
) -> Result<(), Error> {
    if expansions.contains_key(&owner) {
        return Ok(());
    }
    let parents = context.interface_parent_order(owner)?;
    for parent in parents {
        expand_interface(context, *parent, expansions)?;
    }
    let mut expansion =
        InterfaceSlotExpansion::inherit(parents.iter().map(|parent| &expansions[parent]))?;
    for member in context.interface_members(owner)? {
        expansion.declare(member)?;
    }
    expansions.insert(owner, expansion);
    Ok(())
}

fn interface_closure(
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    owner: PersistentExactTypeId,
    visited: &mut BTreeSet<PersistentExactTypeId>,
    interfaces: &mut BTreeSet<PersistentExactTypeId>,
) -> Result<(), Error> {
    if !visited.insert(owner) {
        return Ok(());
    }
    let node = graph.get(owner).ok_or(Error::SlotOrder(owner))?;
    if node.edges().modality() == NominalInheritanceModalityV1::Interface {
        interfaces.insert(owner);
    }
    if let DirectClassBaseV1::ClassBase { exact } = node.edges().direct_base() {
        interface_closure(graph, exact, visited, interfaces)?;
    }
    for interface in node.edges().direct_interfaces() {
        interface_closure(graph, *interface, visited, interfaces)?;
    }
    Ok(())
}

fn schema(
    owner: PersistentExactTypeId,
    role: Role,
    slots: Vec<PersistentDispatchSlotId>,
) -> Result<InheritanceSlotSchemaV1, Error> {
    InheritanceSlotSchemaV1::try_new(role, slots).map_err(|_| Error::SlotOrder(owner))
}
