use super::*;
use scoop_wire::{BudgetMeter, WirePath};

pub(super) fn retain_closed(
    output: &Output,
    nominals: &mut Vec<ConcreteNominal<'_>>,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let closure = NominalMaterializationClosure::from_export_hir(output.export.module(), meter)
        .map_err(|error| match error {
            PublicNominalShapeProjectionError::Materialization(
                NominalMaterializationClosureError::Resource(error),
            ) => inheritance::source_resources::resource(error),
            other => inheritance::source_resources::invalid(other),
        })?;
    meter
        .charge_work(
            (nominals.len() as u64)
                .saturating_mul(1 + u64::from(closure.sources().len().max(1).ilog2())),
            &WirePath::root(),
        )
        .map_err(inheritance::source_resources::resource)?;
    nominals.retain(|nominal| closure.contains(nominal.owner));
    Ok(())
}
