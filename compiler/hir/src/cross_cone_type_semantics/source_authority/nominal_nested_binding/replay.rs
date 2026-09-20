use super::*;

pub(super) fn validate<'c>(
    authority: &mut BoundNominalParameterProtocolsV1<'_, '_, '_, '_>,
    candidate: &'c NominalSupportNestedInterfaceV1,
    protocols: &'c CanonicalProtectedCallableSourceInterfacesV1,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    checked: &mut BoundNestedNominalSourceV1<'c, '_, '_, '_, '_>,
    meter: &mut BudgetMeter,
    depth: u64,
) -> Result<(), Error> {
    let path = WirePath::root();
    meter.check_semantic_depth(depth, &path)?;
    meter.charge_nodes(1, &path)?;
    contracts::nominal(authority, candidate, meter)?;
    candidate
        .validate_concrete_support(graph, checked.representations, meter)
        .map_err(Error::from_concrete)?;
    let records = candidate
        .payload()
        .source_interface()
        .source_support()
        .records();
    meter.check_table_entries(records.len() as u64, &path)?;
    for record in records {
        meter.charge_nodes(1, &path)?;
        if !matches!(record, NestedSourceSupportV1::NestedNominal(_)) {
            contracts::leaf(authority, record, meter)?;
        }
        let owner = match record {
            NestedSourceSupportV1::Callable(value) => match value.declaration() {
                CallableTemplateOrigin::Accessor(_) => continue,
                other => other,
            },
            NestedSourceSupportV1::Constructor(value) => {
                CallableTemplateOrigin::Constructor(value.declaration())
            }
            NestedSourceSupportV1::Property(_) => continue,
            NestedSourceSupportV1::NestedNominal(child) => {
                validate(
                    authority,
                    child,
                    protocols,
                    graph,
                    checked,
                    meter,
                    depth + 1,
                )?;
                continue;
            }
        };
        query(protocols.records().len(), meter)?;
        let protocol = protocols.get(owner).ok_or(Error::MissingProtocol(owner))?;
        let proof = authority
            .validate_source_protocol(protocol, meter)
            .map_err(Error::from_protocol)?;
        query(checked.protocols.len(), meter)?;
        meter.check_table_entries(checked.protocols.len() as u64 + 1, &path)?;
        meter.charge_collection_slots(1, &path)?;
        if checked.protocols.insert(owner, proof).is_some() {
            return Err(Error::DuplicateProtocol(owner));
        }
    }
    Ok(())
}
