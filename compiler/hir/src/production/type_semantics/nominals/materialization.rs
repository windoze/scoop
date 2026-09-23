use super::*;
use scoop_wire::BudgetMeter;

pub(super) fn closure(
    output: &Output,
    meter: &mut BudgetMeter,
) -> Result<NominalMaterializationClosure, Error> {
    NominalMaterializationClosure::from_export_hir(output.export.module(), meter).map_err(|error| {
        match error {
            PublicNominalShapeProjectionError::Materialization(
                NominalMaterializationClosureError::Resource(error),
            ) => inheritance::source_resources::resource(error),
            other => inheritance::source_resources::invalid(other),
        }
    })
}
