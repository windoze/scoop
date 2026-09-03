use super::*;

pub(crate) fn struct_decl(name: &str, fields: Vec<(&str, TypeRef)>) -> Decl {
    generic_struct_decl(name, Vec::new(), fields)
}

pub(crate) fn generic_struct_decl(
    name: &str,
    type_params: Vec<&str>,
    fields: Vec<(&str, TypeRef)>,
) -> Decl {
    generic_struct_decl_full(name, type_params, fields, Vec::new(), Vec::new())
}

// --- M6: classes, interfaces, member functions ---

/// `class Name(props) : Base(args), I1, I2 { methods }`.
pub(crate) fn class_decl(
    modifier: ast::ClassModifier,
    name: &str,
    ctor: Vec<(bool, &str, TypeRef)>,
    base: Option<(&str, Vec<Expr>)>,
    interfaces: Vec<&str>,
    methods: Vec<FunctionDecl>,
) -> Decl {
    Decl::Class(ast::ClassDecl {
        annotations: vec![],
        modifier,
        name: ident(name),
        type_params: Vec::new(),
        constructor: ctor
            .into_iter()
            .map(|(mutable, name, ty)| ast::PrimaryClassParameter {
                property: if mutable {
                    ast::PrimaryParameterProperty::Var
                } else {
                    ast::PrimaryParameterProperty::Val
                },
                name: ident(name),
                ty,
                syntax: ast::ParameterSyntax::Required,
                span: sp(),
            })
            .collect(),
        supertypes: base
            .into_iter()
            .map(|(name, args)| ast::SupertypeSpec {
                ty: ty_named(name),
                constructor_arguments: Some(call_arguments(args)),
                span: sp(),
            })
            .chain(interfaces.into_iter().map(|name| ast::SupertypeSpec {
                ty: ty_named(name),
                constructor_arguments: None,
                span: sp(),
            }))
            .collect(),
        where_clause: None,
        members: methods
            .into_iter()
            .map(ast::ClassMember::Function)
            .collect(),
        span: sp(),
    })
}

/// `interface I { fun m(...): T ... }`.
pub(crate) fn interface_decl(name: &str, methods: Vec<FunctionDecl>) -> Decl {
    Decl::Interface(ast::InterfaceDecl {
        annotations: vec![],
        name: ident(name),
        type_params: Vec::new(),
        supertypes: Vec::new(),
        where_clause: None,
        methods,
        span: sp(),
    })
}

pub(crate) fn generic_interface_decl(
    name: &str,
    type_params: Vec<(ast::Variance, &str)>,
    methods: Vec<FunctionDecl>,
) -> Decl {
    Decl::Interface(ast::InterfaceDecl {
        annotations: vec![],
        name: ident(name),
        type_params: type_params
            .into_iter()
            .map(|(variance, name)| ast::TypeParamDecl {
                name: ident(name),
                variance,
                inline_bound: None,
                span: sp(),
            })
            .collect(),
        supertypes: Vec::new(),
        where_clause: None,
        methods,
        span: sp(),
    })
}

/// A member function with full control over flags and body shape.
pub(crate) fn method_full(
    is_override: bool,
    is_abstract: bool,
    name: &str,
    params: Vec<(&str, TypeRef)>,
    return_ty: Option<TypeRef>,
    body: FunctionBody,
) -> FunctionDecl {
    FunctionDecl {
        annotations: Vec::new(),
        is_suspend: false,
        is_override,
        operator: None,
        infix: None,
        modifier: if is_abstract {
            ast::MethodModifier::Abstract
        } else if is_override {
            ast::MethodModifier::Open
        } else {
            ast::MethodModifier::Final
        },
        receiver_ty: None,
        name: ident(name),
        type_params: Vec::new(),
        params: params
            .into_iter()
            .map(|(name, ty)| Param {
                name: ident(name),
                ty,
                syntax: ast::ParameterSyntax::Required,
                span: sp(),
            })
            .collect(),
        return_ty,
        where_clause: None,
        body,
        span: sp(),
    }
}

/// A plain block-bodied member function (no flags).
pub(crate) fn method(
    name: &str,
    params: Vec<(&str, TypeRef)>,
    return_ty: Option<TypeRef>,
    statements: Vec<Statement>,
) -> FunctionDecl {
    method_full(
        false,
        false,
        name,
        params,
        return_ty,
        FunctionBody::Block(block(statements)),
    )
}

/// A plain expression-bodied member function (no flags).
pub(crate) fn method_expr(
    name: &str,
    params: Vec<(&str, TypeRef)>,
    return_ty: Option<TypeRef>,
    expr: Expr,
) -> FunctionDecl {
    method_full(
        false,
        false,
        name,
        params,
        return_ty,
        FunctionBody::Expr(Box::new(expr)),
    )
}

/// Set the effective modality of a member declaration built by the
/// helpers above.
pub(crate) fn with_method_modifier(
    mut method: FunctionDecl,
    modifier: ast::MethodModifier,
) -> FunctionDecl {
    method.modifier = modifier;
    method
}

/// An `override` expression-bodied member function.
pub(crate) fn override_method_expr(
    name: &str,
    params: Vec<(&str, TypeRef)>,
    return_ty: Option<TypeRef>,
    expr: Expr,
) -> FunctionDecl {
    method_full(
        true,
        false,
        name,
        params,
        return_ty,
        FunctionBody::Expr(Box::new(expr)),
    )
}

/// A bodyless member declaration (interface signatures, `abstract`).
pub(crate) fn bodyless_method(
    is_abstract: bool,
    name: &str,
    params: Vec<(&str, TypeRef)>,
    return_ty: Option<TypeRef>,
) -> FunctionDecl {
    method_full(
        false,
        is_abstract,
        name,
        params,
        return_ty,
        FunctionBody::None,
    )
}

/// A struct with member functions.
pub(crate) fn struct_decl_methods(
    name: &str,
    fields: Vec<(&str, TypeRef)>,
    methods: Vec<FunctionDecl>,
) -> Decl {
    struct_decl_full(name, fields, vec![], methods)
}

/// A struct with an interface list and member functions (spec 4.4.3).
pub(crate) fn struct_decl_full(
    name: &str,
    fields: Vec<(&str, TypeRef)>,
    interfaces: Vec<&str>,
    methods: Vec<FunctionDecl>,
) -> Decl {
    generic_struct_decl_full(name, Vec::new(), fields, interfaces, methods)
}

pub(crate) fn generic_struct_decl_full(
    name: &str,
    type_params: Vec<&str>,
    fields: Vec<(&str, TypeRef)>,
    interfaces: Vec<&str>,
    methods: Vec<FunctionDecl>,
) -> Decl {
    Decl::Struct(AstStructDecl {
        annotations: vec![],
        name: ident(name),
        type_params: type_params.into_iter().map(type_param).collect(),
        fields: fields
            .into_iter()
            .map(|(name, ty)| FieldDecl {
                name: ident(name),
                ty,
                syntax: ast::ParameterSyntax::Required,
                span: sp(),
            })
            .collect(),
        supertypes: interfaces
            .into_iter()
            .map(|name| ast::SupertypeSpec {
                ty: ty_named(name),
                constructor_arguments: None,
                span: sp(),
            })
            .collect(),
        where_clause: None,
        members: methods
            .into_iter()
            .map(Box::new)
            .map(ast::StructMember::Function)
            .collect(),
        span: sp(),
    })
}

/// An enum with member functions.
pub(crate) fn enum_decl_methods(
    name: &str,
    type_params: Vec<&str>,
    variants: Vec<VariantDecl>,
    methods: Vec<FunctionDecl>,
) -> Decl {
    enum_decl_full(name, type_params, variants, vec![], methods)
}

/// An enum with an interface list and member functions (spec 4.4.3).
pub(crate) fn enum_decl_full(
    name: &str,
    type_params: Vec<&str>,
    variants: Vec<VariantDecl>,
    interfaces: Vec<&str>,
    methods: Vec<FunctionDecl>,
) -> Decl {
    Decl::Enum(ast::EnumDecl {
        annotations: vec![],
        name: ident(name),
        type_params: type_params.into_iter().map(type_param).collect(),
        variants,
        interfaces: interfaces.into_iter().map(ty_named).collect(),
        where_clause: None,
        methods,
        span: sp(),
    })
}
