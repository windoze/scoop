use super::*;

pub(super) fn selected(
    analysis: &mut DeclarationAnalysis,
    cursor: &semantic::Cursor<'_>,
    snapshot: &AnalysisSnapshot,
    cancellation: &Cancellation,
) -> Result<Option<Description>, ResponseError> {
    let Some(body) = semantic::body(analysis, cursor, snapshot, cancellation)? else {
        return Ok(None);
    };
    for reference in semantic::callables_at(analysis, body.id, cursor.span, snapshot, cancellation)?
    {
        if reference.target.callable().is_none() {
            return description(analysis, reference.target, snapshot, cursor);
        }
    }
    Ok(None)
}

pub(super) fn declaration(
    analysis: &mut DeclarationAnalysis,
    cursor: &semantic::Cursor<'_>,
    snapshot: &AnalysisSnapshot,
    cancellation: &Cancellation,
) -> Result<Option<Description>, ResponseError> {
    let Some(selected) =
        semantic::constructor_declaration(analysis, cursor, snapshot, cancellation)?
    else {
        return Ok(None);
    };
    match selected {
        hir::EditingConstructorDeclaration::Constructor(callee) => {
            description(analysis, callee, snapshot, cursor)
        }
        hir::EditingConstructorDeclaration::Parameter { callee, index } => {
            let Some(signature) = signature(analysis, callee)? else {
                return Ok(None);
            };
            let parameter = &signature.parameters[index];
            let label = parameter_label(
                analysis,
                &signature.binders,
                parameter,
                true,
                snapshot,
                cursor,
            )?;
            Ok(Some(Description {
                label,
                documentation: Some(documentation::Query {
                    symbol: documentation::applied_parameter(callee, index),
                    origin: parameter.origin.clone(),
                }),
            }))
        }
    }
}

pub(super) fn description(
    analysis: &mut DeclarationAnalysis,
    callee: hir::EditingCallee,
    snapshot: &AnalysisSnapshot,
    cursor: &semantic::Cursor<'_>,
) -> Result<Option<Description>, ResponseError> {
    let Some(signature) = signature(analysis, callee)? else {
        return Ok(None);
    };
    let result = label_type(analysis, &signature.binders, signature.result)?;
    let named = !matches!(
        signature.form,
        hir::EditingConstructorForm::Variant {
            style: hir::VariantStyle::Positional,
            ..
        }
    );
    let parameters = signature
        .parameters
        .iter()
        .map(|parameter| {
            parameter_label(
                analysis,
                &signature.binders,
                parameter,
                named,
                snapshot,
                cursor,
            )
        })
        .collect::<Result<Vec<_>, _>>()?
        .join(", ");
    let label = match signature.form {
        hir::EditingConstructorForm::Nominal => format!("constructor {result}({parameters})"),
        hir::EditingConstructorForm::Variant { name, style } => match style {
            hir::VariantStyle::Unit => format!("{result}.{name}"),
            hir::VariantStyle::Named => format!("{result}.{name} {{ {parameters} }}"),
            hir::VariantStyle::Constructor | hir::VariantStyle::Positional => {
                format!("{result}.{name}({parameters})")
            }
        },
    };
    Ok(Some(Description {
        label,
        documentation: Some(documentation::Query {
            symbol: documentation::callee(callee),
            origin: signature.origin,
        }),
    }))
}

fn signature(
    analysis: &mut DeclarationAnalysis,
    callee: hir::EditingCallee,
) -> Result<Option<hir::EditingConstructorSignature>, ResponseError> {
    analysis
        .constructor_signature(callee)
        .map_err(|error| convert::error(ErrorCode::RequestFailed, error.message))
}

fn label_type(
    analysis: &mut DeclarationAnalysis,
    binders: &hir::SemanticDeclarationBinders,
    ty: hir::TypeId,
) -> Result<String, ResponseError> {
    analysis
        .describe_declaration_type(binders, ty)
        .map(|ty| ty.name)
        .map_err(|error| convert::error(ErrorCode::RequestFailed, error.message))
}

fn parameter_label(
    analysis: &mut DeclarationAnalysis,
    binders: &hir::SemanticDeclarationBinders,
    parameter: &hir::EditingParameter,
    named: bool,
    snapshot: &AnalysisSnapshot,
    cursor: &semantic::Cursor<'_>,
) -> Result<String, ResponseError> {
    let (prefix, ty) = parameter
        .vararg_element_type
        .map_or(("", parameter.value_type), |ty| ("vararg ", ty));
    let ty = label_type(analysis, binders, ty)?;
    let mut label = if named {
        format!("{prefix}{}: {ty}", parameter.name)
    } else {
        ty
    };
    if let Some(default) = &parameter.declared_default {
        label.push_str(" = ");
        label.push_str(imported::source_text(snapshot, cursor.project, default)?.unwrap_or("…"));
    }
    Ok(label)
}
