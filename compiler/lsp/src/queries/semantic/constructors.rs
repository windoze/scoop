use super::*;

pub(in crate::queries) fn constructor_declaration(
    analysis: &mut DeclarationAnalysis,
    cursor: &Cursor<'_>,
    snapshot: &AnalysisSnapshot,
    cancellation: &Cancellation,
) -> Result<Option<hir::EditingConstructorDeclaration>, ResponseError> {
    let declaration = cursor
        .source
        .declaration_source_span(cursor.span)
        .map_err(|error| convert::error(ErrorCode::RequestFailed, error))?;
    let spans = std::iter::once(cursor.span).chain(declaration.filter(|span| *span != cursor.span));
    for span in spans {
        let cancellation = cancellation.clone();
        let revision = snapshot.clone();
        let selected = analysis
            .constructor_declaration_at(&cursor.file.identity, span, move || {
                cancellation.check().is_err() || revision.check_cancelled().is_err()
            })
            .map_err(|_| cancelled())?;
        if let EditingValue::Known(Some(selected)) = selected {
            return Ok(Some(selected));
        }
    }
    Ok(None)
}
