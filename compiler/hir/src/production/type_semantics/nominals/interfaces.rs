use super::*;

pub(super) fn project(
    output: &DependencyHirOutput,
    parameters: &CanonicalInheritanceSourceParameterProtocolsV1,
    meter: &mut scoop_wire::BudgetMeter,
) -> Result<
    (
        CanonicalProtectedDeclarationInterfacesV1,
        CanonicalProtectedCallableSourceInterfacesV1,
    ),
    Error,
> {
    let (_, declarations, protected_protocols) =
        ProtectedDeclarationSourceProductionV1::from_export_hir(&output.output().export, meter)?
            .into_parts();
    let count = parameters
        .records()
        .len()
        .saturating_add(protected_protocols.records().len());
    meter
        .charge_collection_slots(count as u64, &scoop_wire::WirePath::root())
        .map_err(inheritance::source_resources::resource)?;
    meter
        .charge_work(
            (count as u64).saturating_mul(u64::from(count.max(1).ilog2()) + 1),
            &scoop_wire::WirePath::root(),
        )
        .map_err(inheritance::source_resources::resource)?;
    let owners = parameters
        .records()
        .iter()
        .map(InheritanceSourceParameterProtocolV1::owner)
        .chain(
            protected_protocols
                .records()
                .iter()
                .map(ProtectedCallableSourceInterfaceV1::owner),
        )
        .collect::<BTreeSet<_>>();
    let protocols = super::super::nested_sources::project_protocols(
        output.output().export.module(),
        owners,
        meter,
    )?;
    for protocol in protocols.records() {
        if protocol
            .parameters()
            .parameters()
            .iter()
            .any(|parameter| parameter.calling().template().is_some())
        {
            return Err(Error::DefaultTemplateAuthorityRequired(protocol.owner()));
        }
    }
    Ok((declarations, protocols))
}
