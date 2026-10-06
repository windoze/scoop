use super::super::*;

/// Reuse real numeric signatures with the synthetic spans used by this fixture.
/// Codec companions are exercised by the complete sysroot fixtures.
pub(super) fn floating_declarations() -> Vec<Decl> {
    static DECLARATIONS: std::sync::OnceLock<Vec<Decl>> = std::sync::OnceLock::new();
    DECLARATIONS
        .get_or_init(|| {
            let mut source = scoop_parser::parse(include_str!(
                "../../../../../sysroot/lib/scoop.core/src/floating.scoop"
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
                    ast::StructMember::Function(method) => {
                        Some(synthetic_function(*method, Some(&value.name.text)))
                    }
                    ast::StructMember::Companion(_) => None,
                    _ => panic!("numeric core only declares methods and a companion"),
                })
                .collect();
            let Decl::Struct(mut result) =
                struct_decl_full(&value.name.text, Vec::new(), vec!["ToString"], methods)
            else {
                unreachable!()
            };
            result.annotations = synthetic_annotations(value.annotations);
            result.fields = value.fields;
            Decl::Struct(result)
        }
        Decl::Function(value) => Decl::Function(synthetic_function(value, None)),
        Decl::TypeAlias(value) => Decl::TypeAlias(ast::TypeAliasDecl {
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
        FunctionBody::Expr(_) => {
            assert_eq!(value.name.text, "toString");
            let helper = format!("core{}ToString", owner.expect("toString is a member"));
            FunctionBody::Expr(Box::new(call(&helper, vec![this_expr()])))
        }
        FunctionBody::Block(_) => panic!("numeric signatures have no block body"),
    };
    let parameters = value
        .params
        .iter()
        .map(|parameter| (parameter.name.text.as_str(), synthetic_type(&parameter.ty)))
        .collect();
    let mut result = method_full(
        value.is_override,
        false,
        &value.name.text,
        parameters,
        value.return_ty.as_ref().map(synthetic_type),
        body,
    );
    result.annotations = synthetic_annotations(value.annotations);
    result.operator = value.operator.map(|_| ast::OperatorModifier { span: sp() });
    result
}

fn synthetic_type(value: &TypeRef) -> TypeRef {
    let ast::TypeRefKind::Named(name) = &value.kind else {
        panic!("numeric core signatures use named scalar types")
    };
    ty_named(&name.text)
}

fn synthetic_annotations(values: Vec<ast::Annotation>) -> Vec<ast::Annotation> {
    values
        .into_iter()
        .map(|value| ast::Annotation {
            name: ident(&value.name.text),
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
