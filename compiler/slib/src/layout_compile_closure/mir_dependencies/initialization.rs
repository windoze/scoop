use scoop_wire::BudgetMeter;

use super::{SharedMirDependencyGraphError as Error, *};

pub(super) fn replay(
    source: scoop_hir::SharedTypeMetadataV1<'_>,
    dependencies: &[scoop_hir::SharedTypeMetadataV1<'_>],
    mir: &mir::DependencyResolvedCrossConeMirTypeBridgeSectionV1,
    units: &[mir::MirTypeBridgeInitializationUnitV1],
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let uses = source
        .materialized_property_initialization_uses(dependencies, meter)
        .map_err(|error| Error::InitializationOccurrences(Box::new(error)))?;
    let path = WirePath::root();
    let mut expected = Vec::new();
    meter.charge_owned_bytes(
        (uses.len() as u64)
            .saturating_mul(std::mem::size_of::<mir::SelectedExternalInitializationUseV1>() as u64),
        &path,
    )?;
    meter.try_reserve_collection_slots(&mut expected, uses.len(), &path)?;
    for usage in uses {
        meter.charge_work(units.len() as u64, &path)?;
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
                meter,
            )
            .map_err(|error| Error::InitializationUse(Box::new(error)))?,
        );
    }
    let actual = mir.exports().initialization_uses().records();
    meter.charge_work(expected.len().max(actual.len()) as u64, &path)?;
    if expected != actual {
        return Err(Error::InitializationUseInventory);
    }
    Ok(())
}
