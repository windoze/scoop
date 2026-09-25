use scoop_identity::{ConcreteExpressionOrigin, EvaluationOrigin};

use super::ExternalHirReferenceProductionError as Error;

pub(super) fn project<E>(
    output: &crate::DependencyHirOutput,
    origin: crate::ConcreteExpressionOrigin,
) -> Result<ConcreteExpressionOrigin, Error<E>> {
    let export = output.output().export.module();

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
