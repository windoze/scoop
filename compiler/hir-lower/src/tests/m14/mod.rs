//! M14 structured generic constraints and interface upper-bound validation.

use scoop_ast as ast;
use scoop_hir as hir;

use super::*;

fn upper(name: &str, interface: TypeRef) -> ast::TypeParamDecl {
    ast::TypeParamDecl {
        name: ident(name),
        inline_bound: Some(ast::TypeBound::Upper(interface)),
        span: sp(),
    }
}

fn where_clause(constraints: Vec<(&str, ast::TypeBound)>) -> ast::WhereClause {
    ast::WhereClause {
        constraints: constraints
            .into_iter()
            .map(|(parameter, bound)| ast::TypeConstraint {
                parameter: ident(parameter),
                bound,
                span: sp(),
            })
            .collect(),
        span: sp(),
    }
}

fn marker_world() -> Vec<Decl> {
    vec![
        interface_decl("Marker", vec![]),
        struct_decl_full("Marked", vec![], vec!["Marker"], vec![]),
    ]
}

fn bounded_identity() -> Decl {
    let mut declaration = fun_expr(
        "boundedIdentity",
        vec!["T"],
        vec![("value", ty_named("T"))],
        Some(ty_named("T")),
        var("value"),
    );
    let Decl::Function(function) = &mut declaration else {
        unreachable!()
    };
    function.type_params[0] = upper("T", ty_named("Marker"));
    declaration
}

fn generic_class(
    name: &str,
    parameters: Vec<ast::TypeParamDecl>,
    constructor: Vec<(bool, &str, TypeRef)>,
    methods: Vec<ast::FunctionDecl>,
) -> Decl {
    let mut declaration = class_decl(
        ast::ClassModifier::Final,
        name,
        constructor,
        None,
        Vec::new(),
        methods,
    );
    let Decl::Class(class) = &mut declaration else {
        unreachable!()
    };
    class.type_params = parameters;
    declaration
}

fn operator_equals(is_override: bool, bodyless: bool, other: TypeRef) -> ast::FunctionDecl {
    let mut method = method_full(
        is_override,
        bodyless,
        "equals",
        vec![("other", other)],
        Some(ty_named("Boolean")),
        if bodyless {
            FunctionBody::None
        } else {
            FunctionBody::Expr(Box::new(bool_lit(true)))
        },
    );
    method.operator = Some(ast::OperatorModifier { span: sp() });
    method.visibility = ast::VisibilitySyntax::Explicit {
        visibility: ast::DeclaredVisibility::Public,
        span: sp(),
    };
    method
}

mod bound_members;
mod bounds;
mod core_contracts;
mod generics;
mod intrinsic_types;
mod operators;
