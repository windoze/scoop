use super::*;

pub(super) fn describe(
    queries: &mut dyn hir::SemanticQueries,
    context: hir::SemanticContext,
    reference: DefaultReference,
) -> Result<Value> {
    let Some(reference_id) = queries
        .resolve_default_reference(reference.owner.into(), reference.parameter)
        .map_err(one)?
    else {
        return Ok(Value::Null);
    };
    let Some(definition) = queries.projected_default(reference_id).map_err(one)? else {
        return Ok(Value::Null);
    };
    let applied = if let Some(arguments) = reference.arguments {
        let arguments = arguments
            .into_iter()
            .map(|argument| resolve_type(queries, context, argument))
            .collect::<Result<Vec<_>>>()?;
        let application = queries
            .default_application(context.into(), reference_id, &arguments)?
            .expect("a resolved default retains its definition in the fixed view");
        encode(crate::applied_default(&application))?
    } else {
        Value::Null
    };
    Ok(json!({
        "definition": encode(crate::BodyJson::new(definition.as_ref()))?,
        "applied": applied,
    }))
}
