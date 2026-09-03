use super::*;

fn find_function(module: &hir::Module, name: &str) -> hir::FunctionId {
    module
        .functions
        .iter()
        .find_map(|(id, function)| (function.name == name).then_some(id))
        .unwrap_or_else(|| panic!("function `{name}` must exist"))
}

fn function_body<'module>(module: &'module hir::Module, name: &str) -> &'module hir::Body {
    match &module.functions[find_function(module, name)].kind {
        hir::FunctionKind::User(body) => body,
        other => panic!("function `{name}` must have a user body, found {other:?}"),
    }
}

fn concrete_return_value(statements: &[hir::concrete::Statement]) -> &hir::concrete::Expr {
    statements
        .iter()
        .rev()
        .find_map(|statement| match &statement.kind {
            hir::concrete::StatementKind::Return { value: Some(value) } => Some(value),
            _ => None,
        })
        .expect("a concrete return with a value must exist")
}

#[test]
fn super_call_keeps_a_direct_base_target_in_both_hir_products() {
    let base_method = with_method_modifier(
        method_expr("value", vec![], Some(ty_named("Int")), int_lit(1)),
        ast::MethodModifier::Open,
    );
    let derived_method = override_method_expr(
        "value",
        vec![],
        Some(ty_named("Int")),
        super_method_call("value", vec![]),
    );
    let file = file(vec![
        class_decl(
            ast::ClassModifier::Open,
            "Base",
            vec![],
            None,
            vec![],
            vec![base_method],
        ),
        class_decl(
            ast::ClassModifier::Final,
            "Derived",
            vec![],
            Some(("Base", vec![])),
            vec![],
            vec![derived_method],
        ),
        fun("main", vec![val("derived", call("Derived", vec![]))]),
    ]);
    let output = lower_user_output(file).expect("a concrete direct-base call must lower");
    let base = find_function(&output.export, "Base.value");
    let derived = function_body(&output.export, "Derived.value");
    let value = return_value(&derived.statements);
    let hir::ExprKind::DirectSuperMethodCall { callee, .. } = value.kind else {
        panic!("Export HIR must preserve the direct-super proof")
    };
    assert_eq!(output.export.callable_function(callee), base);
    assert!(matches!(
        output.export.functions[base]
            .method
            .expect("base method metadata")
            .dispatch,
        hir::MethodDispatch::Virtual(_)
    ));

    let concrete = output
        .local
        .functions
        .iter()
        .find_map(|(_, function)| (function.name == "Derived.value").then_some(function))
        .expect("concrete derived method");
    let hir::concrete::FunctionKind::User(body) = &concrete.kind else {
        panic!("derived method has a concrete body")
    };
    let value = concrete_return_value(&body.statements);
    assert!(matches!(
        value.kind,
        hir::concrete::ExprKind::DirectSuperMethodCall { .. }
    ));
}

#[test]
fn super_call_without_a_direct_base_is_rejected() {
    let file = file(vec![
        class_decl(
            ast::ClassModifier::Final,
            "Root",
            vec![],
            None,
            vec![],
            vec![method_expr(
                "value",
                vec![],
                Some(ty_named("Int")),
                super_method_call("value", vec![]),
            )],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("a root class has no super member layer");
    assert!(
        errors
            .iter()
            .any(|error| { error.message == "class `Root` has no direct base for `super.value`" })
    );
}

#[test]
fn abstract_super_target_is_rejected() {
    let abstract_value = method_full(
        false,
        true,
        "value",
        vec![],
        Some(ty_named("Int")),
        FunctionBody::None,
    );
    let file = file(vec![
        class_decl(
            ast::ClassModifier::Abstract,
            "Base",
            vec![],
            None,
            vec![],
            vec![abstract_value],
        ),
        class_decl(
            ast::ClassModifier::Final,
            "Derived",
            vec![],
            Some(("Base", vec![])),
            vec![],
            vec![override_method_expr(
                "value",
                vec![],
                Some(ty_named("Int")),
                super_method_call("value", vec![]),
            )],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("an abstract super target is not callable");
    assert!(
        errors
            .iter()
            .any(|error| error.message
                == "abstract base method `value` cannot be called with `super`")
    );
}
