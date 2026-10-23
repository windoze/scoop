use super::*;

pub(super) fn description(
    analysis: &mut DeclarationAnalysis,
    cursor: &semantic::Cursor<'_>,
    snapshot: &AnalysisSnapshot,
    cancellation: &Cancellation,
) -> Result<Option<Description>, ResponseError> {
    let Some(reference) = semantic::static_variant(analysis, cursor, snapshot, cancellation)?
    else {
        return Ok(None);
    };
    let binders = hir::SemanticDeclarationBinders {
        owner: Vec::new(),
        callable: Vec::new(),
    };
    let ty = analysis
        .describe_declaration_type(&binders, reference.value_type)
        .map_err(|error| convert::error(ErrorCode::RequestFailed, error.message))?;
    Ok(Some(Description {
        label: format!("{}: {}", reference.name, ty.name),
        documentation: reference.definition.map(|origin| documentation::Query {
            symbol: documentation::variant(reference.target),
            origin,
        }),
    }))
}
