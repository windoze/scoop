use super::*;

pub(super) fn closure(output: &Output) -> Result<NominalMaterializationClosure, Error> {
    NominalMaterializationClosure::from_current_declarations(output.export.module()).map_err(
        |error| match error {
            PublicNominalShapeProjectionError::Materialization(
                NominalMaterializationClosureError::Resource(error),
            ) => inheritance::source_errors::resource(error),
            other => inheritance::source_errors::invalid(other),
        },
    )
}
