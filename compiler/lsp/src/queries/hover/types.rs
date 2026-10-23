use super::*;

pub(super) fn description(
    analysis: &mut DeclarationAnalysis,
    cursor: &semantic::Cursor<'_>,
    cancellation: &Cancellation,
) -> Result<Option<Description>, ResponseError> {
    let cancelled = cancellation.clone();
    let description = analysis
        .type_description(&cursor.file.identity, cursor.span, move || {
            cancelled.check().is_err()
        })
        .map_err(|_| semantic::cancelled())?;
    let EditingValue::Known(Some(description)) = description else {
        return Ok(None);
    };
    let mut label = description.label;
    if let Some(application) = description.application {
        label.push_str(&format!("\ntype: {}", application.name));
    }
    Ok(Some(Description {
        label,
        documentation: description
            .documentation
            .zip(description.definition)
            .map(|(symbol, origin)| documentation::Query { symbol, origin }),
    }))
}
