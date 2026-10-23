use super::*;

pub fn type_description(
    queries: &mut dyn hir::SemanticQueries,
    function: hir::FunctionId,
    ty: hir::TypeId,
) -> Result<Value> {
    let context = hir::SemanticContext::Function(function);
    let description = queries.describe_type(context.into(), ty)?;
    let value_type = queries.type_signature(context.into(), ty)?;
    let parents = description
        .parents
        .iter()
        .map(|&ty| {
            Ok(json!(QueryJson(
                &queries.type_signature(context.into(), ty)?
            )))
        })
        .collect::<Result<Vec<_>>>()?;
    use hir::SemanticTypeStructure as Shape;
    let shape = match &description.structure {
        Shape::Unit => json!({"kind": "unit"}),
        Shape::Any => json!({"kind": "any"}),
        Shape::Nominal(nominal) => {
            let arguments = nominal
                .arguments
                .iter()
                .map(|&ty| {
                    Ok(json!(QueryJson(
                        &queries.type_signature(context.into(), ty)?
                    )))
                })
                .collect::<Result<Vec<_>>>()?;
            let companion = nominal
                .companion
                .map(|ty| queries.type_signature(context.into(), ty))
                .transpose()?;
            json!({"kind": "nominal", "declaration": NominalId::from(nominal.declaration),
                "nominalKind": QueryJson(&nominal.kind), "arguments": arguments,
                "parameters": nominal_parameters(queries, nominal)?, "companion": QueryJson(&companion),
                "annotations": Annotations {queries, target: nominal.annotations},
                "origin": QueryJson(&nominal.origin)})
        }
        Shape::Tuple(elements) => {
            let elements = elements
                .iter()
                .map(|&ty| {
                    Ok(json!(QueryJson(
                        &queries.type_signature(context.into(), ty)?
                    )))
                })
                .collect::<Result<Vec<_>>>()?;
            json!({"kind": "tuple", "elements": elements})
        }
        Shape::Function(_) => {
            let mut shape = json!(QueryJson(&value_type));
            shape["kind"] = json!("function");
            shape
        }
        Shape::NativeFunctionPointer(_) => {
            let mut shape = json!(QueryJson(&value_type));
            shape["kind"] = json!("nativeFunctionPointer");
            shape
        }
        Shape::Pointer(pointee) => {
            json!({"kind": "pointer", "pointee": QueryJson(&queries.type_signature(context.into(), *pointee)?)})
        }
        Shape::Parameter(parameter) => {
            json!({"kind": "parameter", "parameter": lexical_parameter(queries, function, parameter)?})
        }
        Shape::Companion { host, .. } => {
            json!({"kind": "companion", "host": lexical_parameter(queries, function, host)?})
        }
    };
    Ok(
        json!({"type": QueryJson(&value_type), "name": description.name, "parents": parents, "shape": shape}),
    )
}

fn lexical_parameter(
    queries: &mut dyn hir::SemanticQueries,
    function: hir::FunctionId,
    parameter: &hir::TypeParamDecl,
) -> Result<Value> {
    let signature = queries.signature(function);
    let binders = signature.declaration_binders();
    let references = binders.signature_binders();
    let index = signature
        .type_parameters
        .iter()
        .position(|p| p.id == parameter.id)
        .ok_or_else(|| {
            error(
                parameter.span,
                "the parameter is outside the method context",
            )
        })?;
    let (declaration, source) = if index < signature.owner_parameter_count {
        let receiver = signature
            .receiver
            .ok_or_else(|| error(parameter.span, "the owner parameter has no receiver"))?;
        let owner = queries.nominal_declaration(receiver)?.ok_or_else(|| {
            error(
                parameter.span,
                "the owner parameter has no nominal declaration",
            )
        })?;
        (
            json!({"kind": "nominal", "owner": NominalId::from(owner.declaration), "index": index}),
            owner.origin.source,
        )
    } else {
        let method = crate::MethodId::from(&queries.method_identity(function)?);
        (
            json!({"kind": "callable", "owner": method, "index": index - signature.owner_parameter_count}),
            signature.origin.source,
        )
    };
    SignatureProjection {
        queries,
        context: (&binders).into(),
        source: &source,
        binders: &references,
        application: None,
    }
    .type_parameter(parameter, &references[index], declaration)
}
