use super::*;
use crate::{DirectClassBaseV1, NominalInterfaceRecordV1, PublicNominalKindV1};
use scoop_identity::SignatureTypeKey;

pub(super) fn collect(
    context: &mut Context<'_>,
    provider: CheckedSharedTypeFoundationV1<'_>,
    dependencies: &[CheckedSharedTypeFoundationV1<'_>],
) -> Result<(), Error> {
    let table = provider.section.inheritance();
    let representations = provider.representations.table().records();
    let path = WirePath::root();

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
        let declaration = types.nominal(owner)?;
        let expected = project(types, exact, declaration)?;
        let actual = table.get(exact).ok_or(Error::InheritanceEdges(exact))?;

        if actual.edges() != &expected {
            return Err(Error::InheritanceEdges(exact));
        }

        context.exacts.insert(exact, types.key(exact)?);
        scoop_wire::allocation::try_reserve(&mut context.edges, 1, &path)?;
        context.edges.push(expected);
    }
    Ok(())
}

fn project(
    types: MetadataTypes<'_, '_>,
    exact: PersistentExactTypeId,
    declaration: &NominalInterfaceRecordV1,
) -> Result<NominalInheritanceEdgesV1, Error> {
    let path = WirePath::root();
    let mut base = DirectClassBaseV1::NoClassBase;
    let mut interfaces = Vec::new();
    for parent in declaration.exact_supertypes().values() {
        let SignatureTypeKey::Nominal(owner) = parent else {
            return Err(Error::NonConcreteSignature);
        };
        let target = types.nominal(*owner)?;
        let parent_exact = types.nominal_exact(*owner)?;
        match target.kind() {
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

    NominalInheritanceEdgesV1::try_new(
        exact,
        declaration.declaration_details().modality(),
        base,
        interfaces,
    )
    .map_err(|_| Error::InheritanceEdges(exact))
}
