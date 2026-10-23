use super::*;
use scoop_analysis::ProjectInput;

pub(super) fn label(
    analysis: &mut DeclarationAnalysis,
    callable: hir::DefaultCallableDeclarationV1,
    snapshot: &AnalysisSnapshot,
    project: &ProjectInput,
) -> Result<Option<String>, ResponseError> {
    let Some(signature) = analysis
        .imported_signature(callable)
        .map_err(|error| convert::error(ErrorCode::RequestFailed, error.message))?
    else {
        return Ok(None);
    };
    let binders = signature.declaration_binders();
    let mut parameters = Vec::with_capacity(signature.parameters.len());
    for parameter in &signature.parameters {
        let (ty, vararg, default) = match &parameter.calling {
            hir::SemanticParameterCalling::Required => {
                (parameter.value_type.declaration, false, None)
            }
            hir::SemanticParameterCalling::Default(default) => {
                (parameter.value_type.declaration, false, Some(default))
            }
            hir::SemanticParameterCalling::Vararg {
                element_type,
                omission,
            } => (
                element_type.declaration,
                true,
                match omission {
                    hir::SemanticVarargOmission::EmptyArray => None,
                    hir::SemanticVarargOmission::Default(default) => Some(default),
                },
            ),
        };
        let prefix = if vararg { "vararg " } else { "" };
        let mut label = format!(
            "{prefix}{}: {}",
            parameter.name,
            type_label(analysis, &binders, ty)?
        );
        if let Some(default) = default {
            label.push_str(" = ");
            label.push_str(default_text(analysis, snapshot, project, default)?.unwrap_or("…"));
        }
        parameters.push(label);
    }
    let generics = if binders.callable.is_empty() {
        String::new()
    } else {
        format!(
            "<{}> ",
            binders
                .callable
                .iter()
                .map(|parameter| parameter.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let receiver = signature
        .receiver
        .map(|ty| type_label(analysis, &binders, ty).map(|name| format!("{name}.")))
        .transpose()?
        .unwrap_or_default();
    let result = type_label(analysis, &binders, signature.result)?;
    let suspend = if signature.is_suspend { "suspend " } else { "" };
    Ok(Some(format!(
        "{suspend}fun {generics}{receiver}{}({}): {result}",
        signature.name,
        parameters.join(", ")
    )))
}

fn type_label(
    analysis: &mut DeclarationAnalysis,
    binders: &hir::SemanticDeclarationBinders,
    ty: hir::TypeId,
) -> Result<String, ResponseError> {
    analysis
        .describe_declaration_type(binders, ty)
        .map(|ty| ty.name)
        .map_err(|error| convert::error(ErrorCode::RequestFailed, error.message))
}

fn default_text<'a>(
    analysis: &DeclarationAnalysis,
    snapshot: &'a AnalysisSnapshot,
    project: &ProjectInput,
    default: &hir::SemanticDefaultUse,
) -> Result<Option<&'a str>, ResponseError> {
    let hir::SemanticDefaultReference::Imported(key) = default.definition else {
        return Ok(None);
    };
    let Some(origin) = analysis.imported_default_origin(key) else {
        return Ok(None);
    };
    source_text(snapshot, project, &origin)
}

pub(super) fn source_text<'a>(
    snapshot: &'a AnalysisSnapshot,
    project: &ProjectInput,
    origin: &hir::SemanticSourceLocation,
) -> Result<Option<&'a str>, ResponseError> {
    let Some(file) = project
        .cones
        .values()
        .filter_map(|cone| cone.source())
        .flat_map(|cone| &cone.sources)
        .find(|file| file.identity == origin.source)
    else {
        return Ok(None);
    };
    let source = snapshot
        .source(file.source)
        .map_err(|_| semantic::cancelled())?;
    Ok(source.and_then(|source| {
        source
            .text
            .as_str()
            .get(origin.span.start as usize..origin.span.end as usize)
    }))
}
