use super::*;

pub(super) fn signature_help(
    analysis: &mut DeclarationAnalysis,
    cursor: &semantic::Cursor<'_>,
    snapshot: &AnalysisSnapshot,
    cancellation: &Cancellation,
) -> Result<Value, ResponseError> {
    let mut selected: Option<hir::EditingIncompleteCall> = None;
    for function in semantic::functions(analysis, cursor) {
        let calls = analysis
            .incomplete_calls(function.function, || {
                cancellation.check().is_err() || snapshot.check_cancelled().is_err()
            })
            .map_err(|_| semantic::cancelled())?;
        for call in calls {
            if contains(call, cursor.span.start)
                && selected.as_ref().is_none_or(|previous| {
                    call.span.end - call.span.start < previous.span.end - previous.span.start
                })
            {
                selected = Some(call.clone());
            }
        }
    }
    let Some(call) = selected else {
        return Ok(Value::Null);
    };
    let EditingValue::Known(candidates) = &call.candidates else {
        return Ok(Value::Null);
    };
    if candidates.is_empty() {
        return Ok(Value::Null);
    }
    let name = call
        .name
        .as_deref()
        .map(scoop_analysis::identifier_spelling)
        .unwrap_or_default();
    let signatures = candidates
        .iter()
        .map(|candidate| {
            let labels = candidate
                .parameters
                .iter()
                .map(|parameter| {
                    let prefix = if parameter.vararg { "vararg " } else { "" };
                    let default = if parameter.has_default { " = …" } else { "" };
                    match &parameter.name {
                        Some(name) => format!(
                            "{prefix}{}: {}{default}",
                            scoop_analysis::identifier_spelling(name),
                            parameter.type_label
                        ),
                        None => parameter.type_label.clone(),
                    }
                })
                .collect::<Vec<_>>();
            let result = match &candidate.result {
                EditingValue::Known(result) => result.as_str(),
                EditingValue::Unavailable(_) => "…",
            };
            SignatureInformation {
                label: format!("{name}({}): {result}", labels.join(", ")),
                documentation: None,
                parameters: Some(
                    labels
                        .into_iter()
                        .map(|label| ParameterInformation {
                            label: ParameterLabel::Simple(label),
                            documentation: None,
                        })
                        .collect(),
                ),
                active_parameter: active_parameter(&call, candidate, cursor.span.start)
                    .map(|index| index as u32),
            }
        })
        .collect();
    super::super::json(SignatureHelp {
        signatures,
        active_signature: None,
        active_parameter: None,
    })
}

fn contains(call: &hir::EditingIncompleteCall, offset: u32) -> bool {
    if call
        .trailing
        .is_some_and(|span| span.start <= offset && offset <= span.end)
    {
        return true;
    }
    let end = call.arguments_end - u32::from(call.closed);
    call.opening.end <= offset && offset <= end
}

fn active_parameter(
    call: &hir::EditingIncompleteCall,
    candidate: &hir::EditingCallCandidate,
    offset: u32,
) -> Option<usize> {
    if call
        .trailing
        .is_some_and(|span| span.start <= offset && offset <= span.end)
    {
        return candidate
            .source_parameters
            .get(call.argument_spans.len())
            .copied();
    }
    let slot = call
        .commas
        .iter()
        .filter(|comma| comma.end <= offset)
        .count();
    candidate
        .source_parameters
        .get(slot)
        .copied()
        .or_else(|| candidate.missing_parameters.first().copied())
        .or_else(|| {
            candidate
                .parameters
                .iter()
                .position(|parameter| parameter.vararg)
        })
}
