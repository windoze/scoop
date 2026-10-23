use super::*;

pub(in crate::queries) fn static_callable(
    analysis: &DeclarationAnalysis,
    cursor: &Cursor<'_>,
    snapshot: &AnalysisSnapshot,
    cancellation: &Cancellation,
) -> Result<Option<hir::EditingCallableReference>, ResponseError> {
    match analysis
        .static_callable_reference(&cursor.file.identity, cursor.span, || {
            cancellation.check().is_err() || snapshot.check_cancelled().is_err()
        })
        .map_err(|_| cancelled())?
    {
        EditingValue::Known(reference) => Ok(reference),
        EditingValue::Unavailable(_) => Ok(None),
    }
}

pub(in crate::queries) fn static_named_argument(
    analysis: &DeclarationAnalysis,
    cursor: &Cursor<'_>,
    snapshot: &AnalysisSnapshot,
    cancellation: &Cancellation,
) -> Result<Option<hir::EditingNamedArgument>, ResponseError> {
    match analysis
        .static_named_argument(&cursor.file.identity, cursor.span, || {
            cancellation.check().is_err() || snapshot.check_cancelled().is_err()
        })
        .map_err(|_| cancelled())?
    {
        EditingValue::Known(argument) => Ok(argument),
        EditingValue::Unavailable(_) => Ok(None),
    }
}
