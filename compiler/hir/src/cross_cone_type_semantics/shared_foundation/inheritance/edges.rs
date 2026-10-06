use super::*;
use crate::{DirectClassBaseV1, PublicNominalKindV1};

pub(super) fn collect(
    context: &mut Context<'_>,
    provider: CheckedSharedTypeFoundationV1<'_>,
    dependencies: &[CheckedSharedTypeFoundationV1<'_>],
) -> Result<(), Error> {
    let table = provider.section.inheritance();
    let representations = provider.representations.table().records();

    if table.records().len() != representations.len() {
        return Err(Error::InheritanceInventory(provider.provider()));
    }
    let types = MetadataTypes {
        current: provider.metadata,
        dependencies,
    };
    for representation in representations {
        let owner = representation.owner();
        let exact = types.nominal_exact(owner)?;
        let expected = project(types, exact)?;
        let actual = table.get(exact).ok_or(Error::InheritanceEdges(exact))?;

        if actual.edges() != &expected {
            return Err(Error::InheritanceEdges(exact));
        }

        context.exacts.insert(exact, types.key(exact)?);
        if context.edges.insert(exact, expected).is_some() {
            return Err(Error::InheritanceInventory(provider.provider()));
        }
    }
    Ok(())
}

pub(super) fn close_applications(
    context: &mut Context<'_>,
    types: MetadataTypes<'_, '_>,
) -> Result<(), Error> {
    let mut pending = context.edges.values().flat_map(parents).collect::<Vec<_>>();
    while let Some(exact) = pending.pop() {
        if context.edges.contains_key(&exact) {
            continue;
        }
        let key = types.key(exact)?;
        if !matches!(key.as_ref(), ExactTypeKey::NominalApplication { .. }) {
            return Err(Error::InheritanceEdges(exact));
        }
        let edges = project(types, exact)?;
        pending.extend(parents(&edges));
        context.exacts.insert(exact, key);
        context.edges.insert(exact, edges);
    }
    Ok(())
}

fn parents(edges: &NominalInheritanceEdgesV1) -> impl Iterator<Item = PersistentExactTypeId> + '_ {
    let base = match edges.direct_base() {
        DirectClassBaseV1::NoClassBase => None,
        DirectClassBaseV1::ClassBase { exact } => Some(exact),
    };
    base.into_iter()
        .chain(edges.direct_interfaces().iter().copied())
}

pub(super) fn project(
    types: MetadataTypes<'_, '_>,
    exact: PersistentExactTypeId,
) -> Result<NominalInheritanceEdgesV1, Error> {
    let application = types.applied_nominal(exact)?;
    let declaration = application.declaration;
    let bindings = application.bindings();
    let path = WirePath::root();
    let mut base = DirectClassBaseV1::NoClassBase;
    let mut interfaces = Vec::new();
    for parent in declaration.exact_supertypes().values() {
        let parent_exact = types.exact_with_bindings(parent, &bindings)?;
        let target = types.applied_nominal(parent_exact)?;
        match target.declaration.kind() {
            PublicNominalKindV1::Class => {
                if base != DirectClassBaseV1::NoClassBase {
                    return Err(Error::InheritanceEdges(exact));
                }
                base = DirectClassBaseV1::ClassBase {
                    exact: parent_exact,
                };
            }
            PublicNominalKindV1::Interface => {
                scoop_wire::allocation::try_reserve(&mut interfaces, 1, &path)?;
                interfaces.push(parent_exact);
            }
            _ => return Err(Error::InheritanceEdges(exact)),
        }
    }

    if let Some(interface) = types.encoding_parent(exact)? {
        interfaces.push(interface);
    }
    NominalInheritanceEdgesV1::try_new(
        exact,
        declaration.declaration_details().modality(),
        base,
        interfaces,
    )
    .map_err(|_| Error::InheritanceEdges(exact))
}
