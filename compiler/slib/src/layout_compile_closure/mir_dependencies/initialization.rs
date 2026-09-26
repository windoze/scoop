use super::{SharedMirDependencyGraphError as Error, *};

pub(super) fn replay(
    source: scoop_hir::SharedTypeMetadataV1<'_>,
    dependencies: &[scoop_hir::SharedTypeMetadataV1<'_>],
    mir: &mir::DependencyResolvedCrossConeMirTypeBridgeSectionV1,
    units: &[mir::MirTypeBridgeInitializationUnitV1],
) -> Result<(), Error> {
    let uses = source
        .materialized_property_initialization_uses(dependencies)
        .map_err(|error| Error::InitializationOccurrences(Box::new(error)))?;
    let path = WirePath::root();
    let mut expected = Vec::new();

    scoop_wire::allocation::try_reserve(&mut expected, uses.len(), &path)?;
    for usage in uses {
        if !units.iter().any(|unit| unit.unit() == usage.local_unit()) {
            return Err(Error::MissingInitializationUnit(usage.local_unit()));
        }
        expected.push(
            mir::SelectedExternalInitializationUseV1::try_new(
                source.provider,
                source.identities,
                usage.local_unit(),
                usage.provider(),
                usage.dependency_unit(),
                mir::MirExternalInitializationCauseV1::PropertyAccessor(usage.accessor()),
            )
            .map_err(|error| Error::InitializationUse(Box::new(error)))?,
        );
    }
    let local_units = units.iter().map(|unit| unit.unit()).collect::<Vec<_>>();
    for usage in source
        .public
        .external_references()
        .materialized_singleton_uses(source.provider, source.identities)
        .map_err(|error| Error::TypeOccurrences(Box::new(error)))?
    {
        let Some(local_unit) = usage
            .initialization_root(source.identities, &local_units)
            .map_err(|error| Error::InitializationOccurrences(Box::new(error)))?
        else {
            continue;
        };
        let dependency = dependencies
            .iter()
            .find(|dependency| dependency.provider == usage.provider)
            .ok_or(Error::MissingObjectUnit(usage.value))?;
        let mut object_units = dependency
            .source_initialization_units()
            .iter()
            .filter(|unit| {
                matches!(unit.key(), scoop_identity::InitializationUnitKey::Object(owner)
                | scoop_identity::InitializationUnitKey::Companion(owner) if *owner == usage.owner)
            });
        let dependency_unit = object_units
            .next()
            .ok_or(Error::MissingObjectUnit(usage.value))?
            .id();
        if object_units.next().is_some() {
            return Err(Error::MissingObjectUnit(usage.value));
        }
        expected.push(
            mir::SelectedExternalInitializationUseV1::try_new(
                source.provider,
                source.identities,
                local_unit,
                usage.provider,
                dependency_unit,
                mir::MirExternalInitializationCauseV1::ObjectValue(usage.value),
            )
            .map_err(|error| Error::InitializationUse(Box::new(error)))?,
        );
    }
    expected.sort_unstable();
    expected.dedup();
    let actual = mir.exports().initialization_uses().records();

    if expected != actual {
        return Err(Error::InitializationUseInventory);
    }
    Ok(())
}
