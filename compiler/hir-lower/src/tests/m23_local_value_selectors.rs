use scoop_hir as hir;
use scoop_identity::{LocalValueSelector, StructuralDefinitionSiteRole, SyntheticLocalRole};

use super::*;

fn user_function<'module>(module: &'module hir::Module, name: &str) -> &'module hir::Function {
    module
        .functions
        .iter()
        .find_map(|(_, function)| (function.name == name).then_some(function))
        .unwrap_or_else(|| panic!("missing function `{name}`"))
}

fn user_body(function: &hir::Function) -> &hir::Body {
    let hir::FunctionKind::User(body) = &function.kind else {
        panic!("expected a user function body")
    };
    body
}

fn local_by_name<'body>(body: &'body hir::Body, name: &str) -> &'body hir::Local {
    body.locals
        .iter()
        .find_map(|(_, local)| (local.name == name).then_some(local))
        .unwrap_or_else(|| panic!("missing local `{name}`"))
}

#[test]
fn receiver_and_parameters_keep_typed_declaration_selectors() {
    let module = lower_user(file(vec![
        extension_expr(
            ty_named("Int"),
            "choose",
            Vec::new(),
            vec![("first", ty_named("Int")), ("second", ty_named("Int"))],
            Some(ty_named("Int")),
            var("first"),
        ),
        fun("main", Vec::new()),
    ]))
    .expect("an extension function has one receiver and two declared parameters");
    let function = user_function(&module, "choose");
    let body = user_body(function);

    assert!(matches!(
        body.locals[function.params[0].local].selector,
        LocalValueSelector::This
    ));
    assert!(matches!(
        body.locals[function.params[1].local].selector,
        LocalValueSelector::Parameter {
            declaration_index: 0
        }
    ));
    assert!(matches!(
        body.locals[function.params[2].local].selector,
        LocalValueSelector::Parameter {
            declaration_index: 1
        }
    ));
}

#[test]
fn source_declarations_and_desugaring_temporaries_use_separate_path_roles() {
    let module = lower_user(file(vec![
        fun_expr(
            "truth",
            Vec::new(),
            vec![("value", ty_named("Boolean"))],
            Some(ty_named("Boolean")),
            var("value"),
        ),
        fun_sig(
            "sample",
            Vec::new(),
            vec![
                ("input", ty_named("Int")),
                ("left", ty_named("Boolean")),
                ("right", ty_named("Boolean")),
            ],
            None,
            vec![
                val("value", var("input")),
                val(
                    "flag",
                    binary(
                        ast::BinOp::And,
                        var("left"),
                        call("truth", vec![var("right")]),
                    ),
                ),
            ],
        ),
        fun("main", Vec::new()),
    ]))
    .expect("source locals and a short-circuit temporary lower together");
    let function = user_function(&module, "sample");
    let body = user_body(function);

    assert!(matches!(
        local_by_name(body, "input").selector,
        LocalValueSelector::Parameter {
            declaration_index: 0
        }
    ));
    for (name, ordinal) in [("value", 0), ("flag", 1)] {
        let LocalValueSelector::LocalDeclaration { path } = &local_by_name(body, name).selector
        else {
            panic!("`{name}` must be a source local declaration")
        };
        assert_eq!(
            definition_path(path),
            vec![(StructuralDefinitionSiteRole::LocalDeclaration, ordinal)]
        );
    }
    let temporary = body
        .locals
        .iter()
        .find_map(|(_, local)| local.name.starts_with("$shortCircuit.").then_some(local))
        .expect("short-circuit lowering creates one hidden result local");
    let LocalValueSelector::Synthetic { path, role } = &temporary.selector else {
        panic!("the short-circuit result must be a typed synthetic local")
    };
    assert_eq!(*role, SyntheticLocalRole::Temporary);
    assert_eq!(
        definition_path(path),
        vec![(StructuralDefinitionSiteRole::SyntheticValue, 2)]
    );
}

#[test]
fn instantiated_default_internals_are_not_plain_call_temporaries() {
    let mut choose = fun_sig(
        "choose",
        Vec::new(),
        vec![("value", ty_named("Boolean"))],
        Some(ty_named("Boolean")),
        vec![ret(Some(var("value")))],
    );
    let ast::Decl::Function(function) = &mut choose else {
        unreachable!("fun_sig creates a function")
    };
    function.params[0].syntax = ast::ParameterSyntax::Default {
        expression: binary(
            ast::BinOp::And,
            bool_lit(true),
            call("truth", vec![bool_lit(false)]),
        ),
        equals_span: sp(),
    };
    let module = lower_user(file(vec![
        fun_expr(
            "truth",
            Vec::new(),
            vec![("value", ty_named("Boolean"))],
            Some(ty_named("Boolean")),
            var("value"),
        ),
        choose,
        fun("main", vec![stmt(call("choose", Vec::new()))]),
    ]))
    .expect("a default expression with internal control flow is instantiated");
    let body = user_body(user_function(&module, "main"));
    let default_local = body
        .locals
        .iter()
        .find_map(|(_, local)| local.name.starts_with("$shortCircuit.").then_some(local))
        .expect("default instantiation copies its internal result local");
    let LocalValueSelector::Synthetic { path, role } = &default_local.selector else {
        panic!("default internals must remain typed synthetic locals")
    };
    assert_eq!(*role, SyntheticLocalRole::DefaultValue);
    assert_eq!(
        path.segments().last().unwrap().site_role(),
        StructuralDefinitionSiteRole::SyntheticValue
    );
}
