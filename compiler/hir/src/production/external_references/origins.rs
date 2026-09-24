use scoop_identity::{ConcreteExpressionOrigin, EvaluationOrigin};
use scoop_wire::{BudgetMeter, WirePath};

use super::ExternalHirReferenceProductionError as Error;

pub(super) fn project<E>(
    output: &crate::DependencyHirOutput,
    origin: crate::ConcreteExpressionOrigin,
    meter: &mut BudgetMeter,
) -> Result<ConcreteExpressionOrigin, Error<E>> {
    let path = WirePath::root();
    let export = output.output().export.module();
    for source in [origin.definition.file, origin.evaluation.file] {
        if let Some(file) = export.source_files.get(source as usize) {
            meter
                .charge_work(
                    (file.identity.logical_path().as_str().len() as u64)
                        .saturating_mul(2)
                        .saturating_add(4),
                    &path,
                )
                .map_err(Error::Resource)?;
            meter
                .charge_owned_bytes(
                    (file.identity.logical_path().as_str().len() * 2) as u64,
                    &path,
                )
                .map_err(Error::Resource)?;
        }
    }
    let definition = crate::production::project_definition_source(export, origin.definition)
        .map_err(Error::ExpressionOrigin)?;
    let evaluation = crate::production::project_definition_source(
        export,
        crate::DefinitionOrigin {
            provider: origin.evaluation.provider,
            file: origin.evaluation.file,
            span: origin.evaluation.span,
            context: origin.evaluation.context,
        },
    )
    .map_err(Error::ExpressionOrigin)?;
    Ok(ConcreteExpressionOrigin::new(
        definition.origin().clone(),
        EvaluationOrigin::at_definition(evaluation.origin()),
    ))
}
