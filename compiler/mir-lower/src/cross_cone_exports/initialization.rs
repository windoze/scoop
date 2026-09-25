use super::*;

pub(super) fn project(
    input: MirTypeBridgeExportInputV1<'_>,
) -> Result<mir::CanonicalMirExternalInitializationUsesV1, Error> {
    let uses = input
        .hir
        .materialized_property_initialization_uses(input.identities)
        .map_err(|error| Error::InitializationSource(Box::new(error)))?;
    let roots = input.mir.materialization().initialization_roots();
    let mut records = reserve(uses.len())?;
    for usage in uses {
        if !roots
            .iter()
            .any(|root| root.identity() == usage.local_unit())
        {
            return Err(Error::MissingInitializationUnit(usage.local_unit()));
        }
        records.push(
            mir::SelectedExternalInitializationUseV1::try_new(
                input.mir.module().cone,
                input.identities,
                usage.local_unit(),
                usage.provider(),
                usage.dependency_unit(),
                mir::MirExternalInitializationCauseV1::PropertyAccessor(usage.accessor()),
            )
            .map_err(Error::InitializationUse)?,
        );
    }
    mir::CanonicalMirExternalInitializationUsesV1::try_new(records)
        .map_err(Error::InitializationUse)
}
