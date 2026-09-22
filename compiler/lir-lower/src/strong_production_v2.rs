use scoop_identity::{ConeIdentity, PersistentInitializationUnitId};
use scoop_lir as lir;
use scoop_mir as mir;
use scoop_wire::{BudgetMeter, WirePath};

/// Projects every checked MIR initialization use into the typed LIR input for
/// `strong-production/4`. Multiple causes may point at the same dependency;
/// each cause crosses this validation boundary before the Strong writer
/// canonicalizes the wire-level dependency set.
pub fn project_external_initialization_uses_v2(
    bridge: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
    definitions: &[lir::StrongInitializationUnitDefinitionRefV2],
    selected: &lir::StrongProductionDependencySelectionV2<'_>,
    meter: &mut BudgetMeter,
) -> Result<Vec<lir::StrongExternalInitializationUseV2>, StrongProductionV2ProjectionError> {
    if bridge.provider() != selected.consumer() {
        return Err(StrongProductionV2ProjectionError::ConsumerMismatch {
            mir: bridge.provider(),
            lir: selected.consumer(),
        });
    }
    let records = bridge.initialization_uses().records();
    let mut projected = Vec::new();
    meter.try_reserve_collection_slots(&mut projected, records.len(), &WirePath::root())?;
    for record in records {
        meter.charge_work(definitions.len() as u64, &WirePath::root())?;
        let mut matches = definitions.iter().copied().filter(|definition| {
            definition.provider() == record.provider()
                && definition.unit() == record.dependency_unit()
        });
        let Some(definition) = matches.next() else {
            return Err(StrongProductionV2ProjectionError::MissingDefinition {
                provider: record.provider(),
                unit: record.dependency_unit(),
            });
        };
        if matches.next().is_some() {
            return Err(StrongProductionV2ProjectionError::AmbiguousDefinition {
                provider: record.provider(),
                unit: record.dependency_unit(),
            });
        }
        projected.push(
            lir::StrongExternalInitializationUseV2::try_new(
                record.local_unit(),
                definition,
                selected,
                meter,
            )
            .map_err(StrongProductionV2ProjectionError::Use)?,
        );
    }
    Ok(projected)
}

#[derive(Debug)]
pub enum StrongProductionV2ProjectionError {
    Resource(scoop_wire::WireError),
    ConsumerMismatch {
        mir: ConeIdentity,
        lir: ConeIdentity,
    },
    MissingDefinition {
        provider: ConeIdentity,
        unit: PersistentInitializationUnitId,
    },
    AmbiguousDefinition {
        provider: ConeIdentity,
        unit: PersistentInitializationUnitId,
    },
    Use(lir::StrongExternalInitializationUseErrorV2),
}

impl From<scoop_wire::WireError> for StrongProductionV2ProjectionError {
    fn from(error: scoop_wire::WireError) -> Self {
        Self::Resource(error)
    }
}

impl std::fmt::Display for StrongProductionV2ProjectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "cannot project V2 Strong inputs: {self:?}")
    }
}

impl std::error::Error for StrongProductionV2ProjectionError {}
