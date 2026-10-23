use super::*;

pub(super) fn declaration(
    analysis: &mut DeclarationAnalysis,
    cursor: &semantic::Cursor<'_>,
    cancellation: &Cancellation,
) -> Result<Option<Description>, ResponseError> {
    let cancelled = cancellation.clone();
    let declaration = analysis
        .property_declaration(&cursor.file.identity, cursor.span, move || {
            cancelled.check().is_err()
        })
        .map_err(|_| semantic::cancelled())?;
    let EditingValue::Known(Some(declaration)) = declaration else {
        return Ok(None);
    };
    let mut type_name = |ty| {
        analysis
            .describe_declaration_type(&declaration.binders, ty)
            .map(|description| description.name)
            .map_err(|error| convert::error(ErrorCode::RequestFailed, error.message))
    };
    let kind = if declaration.mutable { "var" } else { "val" };
    let parameters = &declaration.binders.callable;
    let generics = if parameters.is_empty() {
        String::new()
    } else {
        format!(
            "<{}> ",
            parameters
                .iter()
                .map(|parameter| parameter.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let receiver = declaration
        .receiver
        .map(&mut type_name)
        .transpose()?
        .map_or_else(String::new, |name| format!("{name}."));
    let value_type = type_name(declaration.value_type)?;
    Ok(Some(Description {
        label: format!(
            "{kind} {generics}{receiver}{}: {value_type}",
            declaration.name
        ),
        documentation: declaration.origin.map(|origin| documentation::Query {
            symbol: documentation::property(declaration.property),
            origin,
        }),
    }))
}
