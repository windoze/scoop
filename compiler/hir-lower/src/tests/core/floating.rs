use super::super::*;

/// Reuse real numeric signatures with the synthetic spans used by this fixture.
/// Codecs, raw bit access and IEEE comparison bodies use full sysroot fixtures.
pub(super) fn floating_declarations() -> Vec<Decl> {
    static DECLARATIONS: std::sync::OnceLock<Vec<Decl>> = std::sync::OnceLock::new();
    DECLARATIONS
        .get_or_init(|| {
            let mut source = scoop_parser::parse(include_str!(
                "../../../../../sysroot/src/scoop.core/src/floating.scoop"
            ))
            .expect("the actual floating core parses");
            source
                .declarations
                .drain(..)
                .map(synthetic_declaration)
                .collect()
        })
        .clone()
}

fn synthetic_declaration(declaration: Decl) -> Decl {
    match declaration {
        Decl::Struct(value) => {
            let methods = value
                .members
                .into_iter()
                .filter_map(|member| match member {
                    ast::StructMember::Function(method)
                        if matches!(method.name.text.as_str(), "ieeeCompare" | "toBits") =>
                    {
                        None
                    }
                    ast::StructMember::Function(method) => {
                        Some(synthetic_function(*method, Some(&value.header.name.text)))
                    }
                    ast::StructMember::Companion(_) => None,
                    _ => panic!("numeric core only declares methods and a companion"),
                })
                .collect();
            let Decl::Struct(mut result) = struct_decl_full(
                &value.header.name.text,
                Vec::new(),
                vec!["ToString"],
                methods,
            ) else {
                unreachable!()
            };
            result.annotations = synthetic_annotations(value.header.annotations);
            result.fields = value.header.fields;
            Decl::Struct(result)
        }
        Decl::Function(value) => Decl::Function(synthetic_function(value, None)),
        Decl::TypeAlias(value) => Decl::TypeAlias(ast::TypeAliasDecl {
            annotations: Vec::new(),
            type_params: Vec::new(),
            where_clause: None,
            visibility: ast::VisibilitySyntax::Omitted,
            name: ident(&value.name.text),
            target: synthetic_type(&value.target),
            span: sp(),
        }),
        _ => panic!("unexpected declaration in the numeric core"),
    }
}

fn synthetic_function(value: ast::FunctionDecl, owner: Option<&str>) -> ast::FunctionDecl {
    let body = match value.body {
        FunctionBody::None => FunctionBody::None,
        FunctionBody::Expr(_) => match value.name.text.as_str() {
            "toString" => {
                let helper = format!("core{}ToString", owner.expect("toString is a member"));
                FunctionBody::Expr(Box::new(call(&helper, vec![this_expr()])))
            }
            _ => panic!("unexpected numeric source body"),
        },
        FunctionBody::Block(_) => panic!("numeric signatures have no block body"),
    };
    let parameters = value
        .params
        .iter()
        .map(|parameter| (parameter.name.text.as_str(), synthetic_type(&parameter.ty)))
        .collect();
    let mut result = method_full(
        // This minimal fixture retains only ToString and Equals conformance.
        // Full sysroot tests cover the ordinary operator interfaces.
        value.is_override && matches!(value.name.text.as_str(), "toString" | "equals"),
        false,
        &value.name.text,
        parameters,
        value.return_ty.as_ref().map(synthetic_type),
        body,
    );
    result.annotations = synthetic_annotations(value.signature.annotations);
    result.operator = value
        .signature
        .operator
        .map(|_| ast::OperatorModifier { span: sp() });
    result
}

fn synthetic_type(value: &TypeRef) -> TypeRef {
    let ast::TypeRefKind::Named(name) = &value.kind else {
        panic!("numeric core signatures use named scalar types")
    };
    // This fixture models scalar operations with canonical types. Public alias
    // declarations and their source spellings are covered by sysroot fixtures.
    static ALIASES: std::sync::OnceLock<std::collections::HashMap<String, String>> =
        std::sync::OnceLock::new();
    let aliases = ALIASES.get_or_init(|| {
        scoop_parser::parse(include_str!(
            "../../../../../sysroot/src/scoop.core/src/types.scoop"
        ))
        .expect("the actual integer aliases parse")
        .declarations
        .into_iter()
        .filter_map(|declaration| {
            let Decl::TypeAlias(alias) = declaration else {
                return None;
            };
            let ast::TypeRefKind::Named(target) = alias.target.kind else {
                panic!("integer core aliases name scalar types")
            };
            Some((alias.name.text, target.text))
        })
        .collect()
    });
    ty_named(aliases.get(&name.text).unwrap_or(&name.text))
}

fn synthetic_annotations(values: Vec<ast::Annotation>) -> Vec<ast::Annotation> {
    values
        .into_iter()
        .map(|value| ast::Annotation {
            use_site: None,
            name: value.name.clone(),
            args: value
                .args
                .into_iter()
                .map(|argument| ast::AnnotationArg {
                    name: argument.name.map(|name| ident(&name.text)),
                    value: argument.value,
                    span: sp(),
                })
                .collect(),
            span: sp(),
        })
        .collect()
}
