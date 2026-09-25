use super::*;
use scoop_wire::WirePath;

pub(super) fn validate(
    production: &ReplayedStrongProductionSectionV2,
    section: &crate::LayoutAbiExportConstituentsV1,
    meter: &mut BudgetMeter,
) -> Result<(), StrongProductionLayoutJoinError> {
    meter.charge_work(
        (section.layouts().records().len() as u64)
            .saturating_mul(production.canonical_definitions().plans().len().max(1) as u64)
            .saturating_mul(2),
        &WirePath::root(),
    )?;
    for record in section.layouts().records() {
        if !matches_definition(
            production.canonical_definitions(),
            record.identity().physical_definition(),
        ) {
            return Err(StrongProductionLayoutJoinError::LayoutProduction(
                record.identity().layout(),
            ));
        }
        if !matches_definition(production.canonical_definitions(), record.scan_definition()) {
            return Err(StrongProductionLayoutJoinError::ScanProduction(
                record.scan(),
            ));
        }
    }
    Ok(())
}

pub(super) fn matches_definition(
    definitions: &crate::StrongObjectSymbolSurfaceV1,
    physical: crate::StrongShapeDefinitionRefV1,
) -> bool {
    definitions.plan(physical.definition()).is_some_and(|plan| {
        plan.primary_atom() == physical.primary() && plan.primary_symbol() == physical.symbol()
    })
}
