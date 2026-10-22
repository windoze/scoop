use super::*;

pub(in crate::queries) fn callable_at(
    analysis: &mut DeclarationAnalysis,
    function: hir::FunctionId,
    span: Span,
    snapshot: &AnalysisSnapshot,
    cancellation: &Cancellation,
) -> Result<Option<hir::EditingCallableReference>, ResponseError> {
    Ok(
        body_facts(analysis, function, snapshot, cancellation)?.and_then(|facts| {
            facts
                .callables
                .iter()
                .find(|reference| reference.span == span)
                .cloned()
        }),
    )
}

pub(in crate::queries) fn property_at(
    analysis: &mut DeclarationAnalysis,
    function: hir::FunctionId,
    span: Span,
    snapshot: &AnalysisSnapshot,
    cancellation: &Cancellation,
) -> Result<Option<hir::EditingPropertyReference>, ResponseError> {
    Ok(
        body_facts(analysis, function, snapshot, cancellation)?.and_then(|facts| {
            facts
                .properties
                .iter()
                .find(|reference| reference.span == span)
                .cloned()
        }),
    )
}

fn body_facts<'a>(
    analysis: &'a mut DeclarationAnalysis,
    function: hir::FunctionId,
    snapshot: &AnalysisSnapshot,
    cancellation: &Cancellation,
) -> Result<Option<&'a hir::EditingBodyFacts>, ResponseError> {
    let snapshot = snapshot.clone();
    let cancellation = cancellation.clone();
    let facts = analysis
        .function_facts(function, move || {
            cancellation.check().is_err() || snapshot.check_cancelled().is_err()
        })
        .map_err(|_| cancelled())?;
    Ok(match facts {
        EditingValue::Known(facts) => Some(facts),
        EditingValue::Unavailable(_) => None,
    })
}
