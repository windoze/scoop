use super::*;

pub(super) fn project(
    output: &DependencyHirOutput,
    parameters: &CanonicalInheritanceSourceParameterProtocolsV1,
) -> Result<
    (
        CanonicalProtectedDeclarationInterfacesV1,
        CanonicalProtectedCallableSourceInterfacesV1,
    ),
    Error,
> {
    let (_, declarations, protected_protocols) =
        ProtectedDeclarationSourceProductionV1::from_export_hir(&output.output().export)?
            .into_parts();

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
    let protocols =
        super::super::nested_sources::project_protocols(output.output().export.module(), owners)?;
    Ok((declarations, protocols))
}
