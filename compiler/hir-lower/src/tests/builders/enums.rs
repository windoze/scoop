use super::*;

pub(crate) fn enum_decl(name: &str, type_params: Vec<&str>, variants: Vec<VariantDecl>) -> Decl {
    Decl::Enum(ast::EnumDecl {
        annotations: vec![],
        visibility: ast::VisibilitySyntax::Omitted,
        name: ident(name),
        type_params: type_params.into_iter().map(type_param).collect(),
        variants,
        interfaces: Vec::new(),
        where_clause: None,
        methods: Vec::new(),
        properties: Vec::new(),
        nested: Vec::new(),
        companion: None,
        span: sp(),
    })
}

pub(crate) fn variant_unit(name: &str) -> VariantDecl {
    VariantDecl {
        annotations: Vec::new(),
        name: ident(name),
        kind: VariantDeclKind::Unit,
        span: sp(),
    }
}

pub(crate) fn variant_positional(name: &str, types: Vec<TypeRef>) -> VariantDecl {
    VariantDecl {
        annotations: Vec::new(),
        name: ident(name),
        kind: VariantDeclKind::Positional(
            types
                .into_iter()
                .map(|ty| ast::PositionalVariantFieldDecl {
                    span: ty.span,
                    ty,
                    annotations: Vec::new(),
                })
                .collect(),
        ),
        span: sp(),
    }
}

pub(crate) fn variant_named(name: &str, fields: Vec<(&str, TypeRef)>) -> VariantDecl {
    VariantDecl {
        annotations: Vec::new(),
        name: ident(name),
        kind: VariantDeclKind::Named(
            fields
                .into_iter()
                .map(|(name, ty)| VariantFieldDecl {
                    annotations: Vec::new(),
                    name: ident(name),
                    ty,
                    syntax: ast::ParameterSyntax::Required,
                    span: sp(),
                })
                .collect(),
        ),
        span: sp(),
    }
}

pub(crate) fn variant_constructor(
    name: &str,
    fields: Vec<(&str, TypeRef, Option<Expr>)>,
) -> VariantDecl {
    VariantDecl {
        annotations: Vec::new(),
        name: ident(name),
        kind: VariantDeclKind::Constructor(
            fields
                .into_iter()
                .map(|(name, ty, default)| VariantFieldDecl {
                    annotations: Vec::new(),
                    name: ident(name),
                    ty,
                    syntax: default.map_or(ast::ParameterSyntax::Required, |expression| {
                        ast::ParameterSyntax::Default {
                            expression,
                            equals_span: sp(),
                        }
                    }),
                    span: sp(),
                })
                .collect(),
        ),
        span: sp(),
    }
}
