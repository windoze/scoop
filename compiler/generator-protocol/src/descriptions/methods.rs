use super::*;

pub(super) fn method(
    queries: &dyn hir::SemanticQueries,
    method: &hir::SemanticMethodDescription,
    arguments: &hir::CanonicalBinderUseListV1,
) -> Result<Value> {
    let declaration = CallableDeclaration::from(method.identity);
    let binders = method.declaration_binders();
    let references = binders.signature_binders();
    let projection = MemberProjection {
        declaration: SignatureProjection {
            queries,
            context: (&binders).into(),
            source: &method.origin.source,
            binders: &references,
            application: None,
        },
        applied: SignatureProjection {
            queries,
            context: (&binders).into(),
            source: &method.origin.source,
            binders: &references,
            application: Some((arguments, u32::from(!method.type_parameters.is_empty()))),
        },
    };
    let parameters = method.type_parameters.iter()
        .zip(&references[method.owner_parameters.len()..])
        .enumerate().map(|(index, (parameter, binder))| {
            let reference = json!({"kind": "callable", "owner": declaration, "index": index});
            Ok(json!({
                "declaration": projection.declaration.type_parameter(&parameter.declaration, binder, reference.clone())?,
                "applied": projection.applied.type_parameter(&parameter.declaration, binder, reference)?,
            }))
        }).collect::<Result<Vec<_>>>()?;
    let context_parameters = method.context_parameters.iter().enumerate().map(|(index, parameter)| {
        Ok(json!({
            "declaration": {"owner": declaration, "index": index},
            "label": QueryJson(&parameter.label), "valueType": projection.type_use(parameter.value_type)?,
        }))
    }).collect::<Result<Vec<_>>>()?;
    Ok(json!({
        "declaration": declaration, "name": method.name,
        "receiver": projection.type_use(method.receiver)?, "typeParameters": parameters,
        "contextParameters": context_parameters, "parameters": projection.parameters(&method.parameters)?,
        "result": projection.type_use(method.result)?,
        "effects": projection.declaration.effects(method.is_suspend, &method.modifiers, &method.attributes, &method.release_callability, method.origin.span)?,
        "modality": QueryJson(&method.modality), "access": QueryJson(&method.access),
        "body": projection.body(declaration, method.callable, &method.origin)?,
        "annotations": projection.annotations(method.annotations), "origin": QueryJson(&method.origin),
    }))
}
