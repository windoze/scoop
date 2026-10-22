use crate::{QueryJson, Reference, signature::Result};
use scoop_hir as hir;
use scoop_identity::{
    DeclarationName, PersistentAnnotationId, SignatureTypeKey, SourceDeclarationKey,
};
use serde_json::{Value, json};

pub fn annotation_declaration(
    queries: &dyn hir::SemanticQueries,
    annotation: PersistentAnnotationId,
) -> Result<Option<Value>> {
    let Some(declaration) = queries.annotation_declaration(annotation) else {
        return Ok(None);
    };
    let (name, parameters, rules, visibility, origin) = match declaration {
        hir::SemanticAnnotationDeclaration::Current(declaration) => {
            let binders = hir::SemanticDeclarationBinders {
                owner: Vec::new(),
                callable: Vec::new(),
            };
            let parameters = declaration
                .parameters
                .iter()
                .enumerate()
                .map(|(index, parameter)| {
                    let value_type = match parameter.value_type {
                        hir::AnnotationParameterType::Value(ty) => {
                            hir::AnnotationParameterType::Value(
                                queries.type_signature((&binders).into(), ty)?,
                            )
                        }
                        hir::AnnotationParameterType::Annotation(id) => {
                            hir::AnnotationParameterType::Annotation(id)
                        }
                        hir::AnnotationParameterType::AnnotationArray { array, element } => {
                            hir::AnnotationParameterType::AnnotationArray { array, element }
                        }
                    };
                    Ok(parameter_json(
                        annotation,
                        index,
                        &parameter.name,
                        &value_type,
                        &parameter.default,
                    ))
                })
                .collect::<Result<Vec<_>>>()?;
            (
                name(declaration.identity.key()),
                parameters,
                declaration.rules,
                hir::DeclaredVisibilityV1::from(declaration.visibility),
                json!(QueryJson(
                    &queries.source_location(declaration.definition_origin)
                )),
            )
        }
        hir::SemanticAnnotationDeclaration::Imported(declaration) => {
            let value = &declaration.declaration;
            let parameters = value
                .parameters
                .iter()
                .enumerate()
                .map(|(index, parameter)| {
                    parameter_json(
                        annotation,
                        index,
                        parameter.name.as_str(),
                        &parameter.value_type,
                        &parameter.default,
                    )
                })
                .collect();
            let origin = value.definition_origin.origin();
            (
                name(&declaration.source),
                parameters,
                value.rules,
                value.visibility,
                json!(crate::json::Location {
                    source: origin.source(),
                    span: origin.span()
                }),
            )
        }
    };
    let targets = hir::AnnotationTargetKind::ALL
        .into_iter()
        .filter(|&target| rules.permits(target))
        .map(hir::AnnotationTargetKind::name)
        .collect::<Vec<_>>();
    Ok(Some(
        json!({"annotation": Reference::from(annotation), "name": name,
        "parameters": parameters, "targets": targets, "repeatable": rules.repeatable,
        "visibility": QueryJson(&visibility), "origin": origin}),
    ))
}

fn name(key: &SourceDeclarationKey) -> &str {
    match key.name() {
        DeclarationName::Named(name) => name.as_str(),
        _ => unreachable!("annotation declarations retain their actual source name"),
    }
}

fn parameter_json(
    annotation: PersistentAnnotationId,
    index: usize,
    name: &str,
    value_type: &hir::AnnotationParameterType<SignatureTypeKey>,
    default: &Option<hir::CanonicalAnnotationValueV1>,
) -> Value {
    json!({"declaration": {"annotation": Reference::from(annotation), "index": index},
        "name": name, "valueType": QueryJson(value_type), "default": QueryJson(default)})
}
