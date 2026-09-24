//! Metered physical checks for a source-derived ordinary callable table.

use super::*;
use scoop_wire::{BudgetMeter, WirePath};

use CrossConeLirBridgeBuildError as Error;

impl CrossConeLirBridgeSectionV1 {
    /// Uses the same typed Strong definition relation as exact callable ABI
    /// exports. Source eligibility and terminal-provider joins remain the
    /// responsibility of the containing Compile closure.
    pub fn try_new_with_meter(
        foundation: &OdrFreeLirFoundation,
        mut exports: Vec<ParamFreeLirCallableExportV1>,
        mut selected: Vec<SelectedDependencyLirCallableV1>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        let path = WirePath::root();
        for count in [exports.len(), selected.len()] {
            meter
                .check_table_entries(count as u64, &path)
                .map_err(Error::Resource)?;
            meter
                .charge_work(
                    (count as u64).saturating_mul(u64::from(count.max(1).ilog2()) + 2),
                    &path,
                )
                .map_err(Error::Resource)?;
        }
        validation::canonical_order(&mut exports, &mut selected)?;
        for (index, export) in exports.iter().enumerate() {
            validate_export(index, export, foundation, meter)?;
        }
        validation::validate_selected(foundation.producer(), &selected).map_err(Error::Relation)?;
        Ok(Self {
            artifact: foundation.producer(),
            exports,
            selected,
        })
    }
}

fn validate_export(
    index: usize,
    export: &ParamFreeLirCallableExportV1,
    foundation: &OdrFreeLirFoundation,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let (body, symbol, definition) = export
        .callable
        .link_contract(foundation.producer())
        .map_err(Error::Callable)?;
    meter
        .charge_work(foundation.callable_bodies().len() as u64, &WirePath::root())
        .map_err(Error::Resource)?;
    if !foundation.contains_callable_body(body) {
        return Err(Error::Relation(
            CrossConeLirBridgeRelationError::MissingExportBody { index, body },
        ));
    }
    let physical = crate::StrongShapeDefinitionRefV1::from_foundation(
        crate::ExternalStrongShapeSubjectV1::Callable(export.target()),
        foundation,
        meter,
    )
    .map_err(Error::Physical)?;
    if physical.symbol() != symbol || physical.definition() != definition {
        return Err(Error::Relation(
            CrossConeLirBridgeRelationError::ExportDefinitionMismatch { index, definition },
        ));
    }
    Ok(())
}
