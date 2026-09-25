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
    let actual = mir.exports().initialization_uses().records();

    if expected != actual {
        return Err(Error::InitializationUseInventory);
    }
    Ok(())
}
