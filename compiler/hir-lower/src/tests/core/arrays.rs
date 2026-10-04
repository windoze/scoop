use super::super::*;

pub(super) fn add_length_getter(class: &mut ast::ClassDecl, intrinsic: &str) {
    let mut length = method_full(
        false,
        false,
        "arrayLength",
        Vec::new(),
        Some(ty_named("Long")),
        FunctionBody::None,
    );
    length.annotations.push(ast::Annotation {
        name: ident("Intrinsic"),
        args: vec![ast::AnnotationArg {
            name: None,
            value: ast::AnnotationLiteral::String(intrinsic.into()),
            span: sp(),
        }],
        span: sp(),
    });
    class.members.push(ast::ClassMember::Function(length));
    class
        .members
        .push(ast::ClassMember::StoredProperty(ast::PropertyDecl {
            context_parameters: Vec::new(),
            annotations: Vec::new(),
            visibility: ast::VisibilitySyntax::Omitted,
            modifier: ast::MethodModifier::Final,
            is_override: false,
            mutable: false,
            receiver_ty: None,
            type_params: Vec::new(),
            where_clause: None,
            name: ident("size"),
            ty: ty_named("Long"),
            body: ast::PropertyBodySyntax::Computed(ast::AccessorSyntax {
                getter: Some(ast::GetterDecl {
                    annotations: Vec::new(),
                    body: ast::AccessorBodySyntax::Expr(Box::new(method_call(
                        this_expr(),
                        "arrayLength",
                        Vec::new(),
                    ))),
                    span: sp(),
                }),
                setter: None,
            }),
            span: sp(),
        }));
}
