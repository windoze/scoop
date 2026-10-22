use super::*;

pub(super) fn named(
    analysis: &mut DeclarationAnalysis,
    function: hir::FunctionId,
    target: Span,
    cancellation: &Cancellation,
) -> Result<Option<Description>, ResponseError> {
    let cancelled = cancellation.clone();
    let argument = analysis
        .named_argument(function, target, move || cancelled.check().is_err())
        .map_err(|_| semantic::cancelled())?;
    let EditingValue::Known(Some(argument)) = argument else {
        return Ok(None);
    };
    let Some(name) = &argument.parameter.name else {
        return Ok(None);
    };
    let (prefix, ty) = argument
        .parameter
        .vararg_element_type
        .map_or(("", argument.parameter.value_type), |ty| ("vararg ", ty));
    let ty = analysis
        .describe_body_type(function, argument.context, ty)
        .map_err(|error| convert::error(ErrorCode::RequestFailed, error.message))?;
    let EditingValue::Known(ty) = ty else {
        return Ok(None);
    };
    Ok(Some(Description {
        label: format!("{prefix}{name}: {}", ty.name),
        documentation: argument
            .callee
            .zip(argument.definition)
            .map(|(callee, origin)| documentation::Query {
                symbol: documentation::applied_parameter(callee, argument.index),
                origin,
            }),
    }))
}
