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
    let local_units = roots.iter().map(|root| root.identity()).collect::<Vec<_>>();
    for usage in input
        .public
        .external_references()
        .materialized_singleton_uses(input.mir.module().cone, input.identities)
        .map_err(|error| Error::SingletonOccurrences(Box::new(error)))?
    {
        let Some(local_unit) = usage
            .initialization_root(input.identities, &local_units)
            .map_err(|error| Error::InitializationSource(Box::new(error)))?
        else {
            continue;
        };
        let object = input
            .dependency_objects
            .iter()
            .find(|object| object.provider() == usage.provider && object.value() == usage.value)
            .ok_or(Error::MissingDependencyObject(usage.value))?;
        records.push(
            mir::SelectedExternalInitializationUseV1::try_new(
                input.mir.module().cone,
                input.identities,
                local_unit,
                usage.provider,
                object.unit(),
                mir::MirExternalInitializationCauseV1::ObjectValue(usage.value),
            )
            .map_err(Error::InitializationUse)?,
        );
    }
    records.sort_unstable();
    records.dedup();
    mir::CanonicalMirExternalInitializationUsesV1::try_new(records)
        .map_err(Error::InitializationUse)
}
