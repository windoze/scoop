use super::*;

pub(super) fn validate<'c>(
    authority: &mut BoundNominalParameterProtocolsV1<'_, '_, '_, '_>,
    candidate: &'c NominalSupportNestedInterfaceV1,
    protocols: &'c CanonicalProtectedCallableSourceInterfacesV1,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    checked: &mut BoundNestedNominalSourceV1<'c, '_, '_, '_, '_>,
) -> Result<(), Error> {
    contracts::nominal(authority, candidate)?;
    candidate
        .validate_concrete_support(graph, checked.representations)
        .map_err(Error::from_concrete)?;
    let records = candidate
        .payload()
        .source_interface()
        .source_support()
        .records();

    for record in records {
        if !matches!(record, NestedSourceSupportV1::NestedNominal(_)) {
            contracts::leaf(authority, record)?;
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
                validate(authority, child, protocols, graph, checked)?;
                continue;
            }
        };

        let protocol = protocols.get(owner).ok_or(Error::MissingProtocol(owner))?;
        let proof = authority
            .validate_source_protocol(protocol)
            .map_err(Error::from_protocol)?;

        if checked.protocols.insert(owner, proof).is_some() {
            return Err(Error::DuplicateProtocol(owner));
        }
    }
    Ok(())
}
