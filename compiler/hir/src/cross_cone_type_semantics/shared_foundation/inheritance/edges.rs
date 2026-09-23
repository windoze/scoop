use super::*;
use crate::{DirectClassBaseV1, NominalInterfaceRecordV1, PublicNominalKindV1};
use scoop_identity::SignatureTypeKey;

pub(super) fn collect(
    context: &mut Context<'_>,
    provider: CheckedSharedTypeFoundationV1<'_>,
    dependencies: &[CheckedSharedTypeFoundationV1<'_>],
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let table = provider.section.inheritance();
    let representations = provider.representations.table().records();
    let path = WirePath::root();
    meter.charge_work(
        table.records().len() as u64 + representations.len() as u64,
        &path,
    )?;
    if table.records().len() != representations.len() {
        return Err(Error::InheritanceInventory(provider.provider()));
    }
    let types = MetadataTypes {
        current: provider.metadata,
        dependencies,
    };
    for representation in representations {
        let owner = representation.owner();
        let exact = types.nominal_exact(owner, meter)?;
        let declaration = types.nominal(owner, meter)?;
        let expected = project(types, exact, declaration, meter)?;
        let actual = table.get(exact).ok_or(Error::InheritanceEdges(exact))?;
        let work =
            scoop_wire::encoded_length(&expected).map_err(|error| Error::Key(error.to_string()))?;
        meter.charge_work(work, &path)?;
        if actual.edges() != &expected {
            return Err(Error::InheritanceEdges(exact));
        }
        meter.charge_collection_slots(1, &path)?;
        context.exacts.insert(exact, types.key(exact, meter)?);
        meter.try_reserve_collection_slots(&mut context.edges, 1, &path)?;
        context.edges.push(expected);
    }
    Ok(())
}

fn project(
    types: MetadataTypes<'_, '_>,
    exact: PersistentExactTypeId,
    declaration: &NominalInterfaceRecordV1,
    meter: &mut BudgetMeter,
) -> Result<NominalInheritanceEdgesV1, Error> {
    let path = WirePath::root();
    let mut base = DirectClassBaseV1::NoClassBase;
    let mut interfaces = Vec::new();
    for parent in declaration.exact_supertypes().values() {
        let SignatureTypeKey::Nominal(owner) = parent else {
            return Err(Error::NonConcreteSignature);
        };
        let target = types.nominal(*owner, meter)?;
        let parent_exact = types.nominal_exact(*owner, meter)?;
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
                meter.try_reserve_collection_slots(&mut interfaces, 1, &path)?;
                interfaces.push(parent_exact);
            }
            _ => return Err(Error::InheritanceEdges(exact)),
        }
    }
    meter.charge_work(
        (interfaces.len() as u64).saturating_mul(1 + u64::from(interfaces.len().max(1).ilog2())),
        &path,
    )?;
    NominalInheritanceEdgesV1::try_new(
        exact,
        declaration.declaration_details().modality(),
        base,
        interfaces,
    )
    .map_err(|_| Error::InheritanceEdges(exact))
}
