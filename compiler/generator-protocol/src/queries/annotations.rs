use super::*;

pub(super) fn describe(
    queries: &mut dyn hir::SemanticQueries,
    targets: Vec<AnnotationTarget>,
    declarations: Vec<Reference<PersistentAnnotationId>>,
) -> Result<Value> {
    let targets = targets
        .into_iter()
        .map(|target| {
            let Some(declaration) = queries.resolve_declaration(target.into()).map_err(one)? else {
                return Ok(Value::Null);
            };
            encode(crate::Annotations {
                queries,
                target: declaration.annotations,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let declarations = declarations
        .into_iter()
        .map(|annotation| {
            let Some(declaration) = queries
                .resolve_declaration(hir::DecodedAnnotationTargetV1::Annotation(
                    annotation.into(),
                ))
                .map_err(one)?
            else {
                return Ok(Value::Null);
            };
            let hir::AnnotationTargetV1::Annotation(annotation) = declaration.reference else {
                unreachable!("a resolved annotation retains its declaration kind")
            };
            Ok(crate::annotation_declaration(queries, annotation)
                .map_err(one)?
                .unwrap_or(Value::Null))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(json!({"targets": targets, "declarations": declarations}))
}
