use super::*;

// --- declaration rules (DESIGN 1.1) ---

#[test]
fn distinguishable_overloads_are_accepted() {
    let file = file(vec![
        fun_expr(
            "show",
            vec![],
            vec![("value", ty_named("Int"))],
            Some(ty_named("String")),
            str_lit("int"),
        ),
        fun_expr(
            "show",
            vec![],
            vec![("value", ty_named("String"))],
            Some(ty_named("String")),
            str_lit("string"),
        ),
        fun_expr(
            "show",
            vec![],
            vec![("value", ty_named("Int")), ("extra", ty_named("Int"))],
            Some(ty_named("String")),
            str_lit("two"),
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user(file).expect("distinguishable overloads must lower");
    top_level_fn(&module, "show", &["Int"]);
    top_level_fn(&module, "show", &["String"]);
    top_level_fn(&module, "show", &["Int", "Int"]);
}

#[test]
fn same_signature_duplicate_is_an_error() {
    let file = file(vec![
        fun_expr(
            "f",
            vec![],
            vec![("x", ty_named("Int"))],
            Some(ty_named("Int")),
            var("x"),
        ),
        fun_expr(
            "f",
            vec![],
            vec![("y", ty_named("Int"))],
            Some(ty_named("Int")),
            var("y"),
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("same-signature overloads must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "function `f` is already declared with the same signature"
    );
}

#[test]
fn differing_only_in_return_type_is_a_duplicate() {
    let file = file(vec![
        fun_expr(
            "f",
            vec![],
            vec![("x", ty_named("Int"))],
            Some(ty_named("Int")),
            var("x"),
        ),
        fun_expr(
            "f",
            vec![],
            vec![("x", ty_named("Int"))],
            Some(ty_named("String")),
            str_lit("s"),
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("return-type-only difference must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "function `f` is already declared with the same signature"
    );
}

#[test]
fn same_signature_method_duplicate_is_an_error() {
    let file = file(vec![
        class_decl(
            Final,
            "C",
            vec![],
            None,
            vec![],
            vec![
                method_expr(
                    "m",
                    vec![("x", ty_named("Int"))],
                    Some(ty_named("Int")),
                    var("x"),
                ),
                method_expr(
                    "m",
                    vec![("y", ty_named("Int"))],
                    Some(ty_named("Int")),
                    var("y"),
                ),
            ],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("same-signature methods must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "function `m` in class `C` is already declared with the same signature"
    );
}

#[test]
fn rejected_duplicate_bodies_are_still_diagnosed() {
    let file = file(vec![
        fun_expr(
            "f",
            vec![],
            vec![("x", ty_named("Int"))],
            Some(ty_named("Int")),
            call("missingFirst", vec![]),
        ),
        fun_expr(
            "f",
            vec![],
            vec![("y", ty_named("Int"))],
            Some(ty_named("Int")),
            call("missingSecond", vec![]),
        ),
        fun("main", vec![]),
    ]);

    let errors = lower_user(file).expect_err("duplicate bodies must still be lowered");
    assert!(errors.iter().any(|error| {
        error.message == "function `f` is already declared with the same signature"
    }));
    assert!(
        errors
            .iter()
            .any(|error| error.message.starts_with("unknown function `missingFirst`"))
    );
    assert!(errors.iter().any(|error| {
        error
            .message
            .starts_with("unknown function `missingSecond`")
    }));
}

#[test]
fn rejected_duplicate_members_block_extension_fallback() {
    let file = file(vec![
        class_decl(
            Final,
            "C",
            vec![],
            None,
            vec![],
            vec![
                method_expr("m", vec![], Some(ty_named("Int")), int_lit(1)),
                method_expr("m", vec![], Some(ty_named("Int")), int_lit(2)),
            ],
        ),
        extension_expr(
            ty_named("C"),
            "m",
            vec![],
            vec![("required", ty_named("Int"))],
            Some(ty_named("Int")),
            var("required"),
        ),
        fun(
            "main",
            vec![stmt(method_call(call("C", vec![]), "m", vec![]))],
        ),
    ]);

    let errors = lower_user(file).expect_err("the duplicate member surface is invalid");
    assert_eq!(
        errors.len(),
        1,
        "a lower extension layer must not be probed"
    );
    assert_eq!(
        errors[0].message,
        "function `m` in class `C` is already declared with the same signature"
    );
}

#[test]
fn global_initializer_uses_the_frozen_duplicate_surface() {
    let global = Decl::Global(ast::PropertyDecl {
        context_parameters: Vec::new(),
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable: false,
        receiver_ty: None,
        type_params: Vec::new(),
        where_clause: None,
        name: ident("probe"),
        ty: ty_named("Unit"),
        body: ast::PropertyBodySyntax::Initializer {
            expression: Box::new(call("println", Vec::new())),
            accessors: ast::AccessorSyntax::default(),
        },
        span: sp(),
    });
    let file = file(vec![
        fun_expr(
            "println",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Unit")),
            unit_lit(),
        ),
        fun_expr(
            "println",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Unit")),
            unit_lit(),
        ),
        global,
        fun("main", Vec::new()),
    ]);

    let errors = lower_user(file).expect_err("the duplicate surface is invalid");
    assert_eq!(
        errors
            .iter()
            .filter(|error| error.severity == ast::DiagnosticSeverity::Error)
            .count(),
        1,
        "the initializer must neither probe duplicates nor fall through to core println"
    );
    assert_eq!(
        errors[0].message,
        "function `println` is already declared with the same signature"
    );
}

#[test]
fn rejected_function_spelling_blocks_contextual_variant_fallback() {
    let file = file(vec![
        enum_decl("State", Vec::new(), vec![variant_unit("Ready")]),
        fun_expr(
            "Ready",
            Vec::new(),
            vec![("first", ty_named("Int")), ("second", ty_named("Int"))],
            Some(ty_named("Unit")),
            unit_lit(),
        ),
        fun_expr(
            "Ready",
            Vec::new(),
            vec![("left", ty_named("Int")), ("right", ty_named("Int"))],
            Some(ty_named("Unit")),
            unit_lit(),
        ),
        fun(
            "main",
            vec![val_ty(
                "state",
                Some(ty_named("State")),
                call("Ready", vec![int_lit(1)]),
            )],
        ),
    ]);

    let errors = lower_user(file).expect_err("the duplicate surface is invalid");
    assert_eq!(errors.len(), 1, "contextual enum lookup must not bypass it");
    assert_eq!(
        errors[0].message,
        "function `Ready` is already declared with the same signature"
    );
}

#[test]
fn rejected_top_level_functions_block_callable_reference_fallback() {
    let reference = Expr::CallableReference {
        id: ast::CallableReferenceId(0),
        receiver: None,
        name: ident("f"),
        span: sp(),
    };
    let file = file(vec![
        fun_expr(
            "f",
            Vec::new(),
            vec![("value", ty_named("Int"))],
            Some(ty_named("Int")),
            var("value"),
        ),
        fun_expr(
            "f",
            Vec::new(),
            vec![("other", ty_named("Int"))],
            Some(ty_named("Int")),
            var("other"),
        ),
        fun(
            "main",
            vec![val_ty(
                "reference",
                Some(ty_function(false, vec![ty_named("Int")], ty_named("Int"))),
                reference,
            )],
        ),
    ]);

    let errors = lower_user(file).expect_err("the duplicate surface is invalid");
    assert_eq!(errors.len(), 1, "reference lookup must stop at the blocker");
    assert_eq!(
        errors[0].message,
        "function `f` is already declared with the same signature"
    );
}

#[test]
fn rejected_invoke_members_keep_a_local_value_call_from_falling_through() {
    let invoke = |marker| {
        let mut method = method_expr("invoke", Vec::new(), Some(ty_named("Unit")), marker);
        method.operator = Some(ast::OperatorModifier { span: sp() });
        method
    };
    let file = file(vec![
        struct_decl_methods(
            "Handler",
            Vec::new(),
            vec![invoke(unit_lit()), invoke(unit_lit())],
        ),
        fun_expr(
            "handler",
            Vec::new(),
            vec![("required", ty_named("Int"))],
            Some(ty_named("Unit")),
            unit_lit(),
        ),
        fun(
            "main",
            vec![
                val("handler", call("Handler", Vec::new())),
                stmt(call("handler", Vec::new())),
            ],
        ),
    ]);

    let errors = lower_user(file).expect_err("the duplicate invoke surface is invalid");
    assert_eq!(
        errors.len(),
        1,
        "the local value must retain the call spelling"
    );
    assert_eq!(
        errors[0].message,
        "function `invoke` in struct `Handler` is already declared with the same signature"
    );
}

#[test]
fn override_with_unmatched_signature_is_an_error() {
    // An overload of `m` exists in the base class, but with a
    // different signature — it is not overridden.
    let file = file(vec![
        class_decl(
            Open,
            "B",
            vec![],
            None,
            vec![],
            vec![method_expr(
                "m",
                vec![("x", ty_named("Int"))],
                Some(ty_named("String")),
                str_lit("i"),
            )],
        ),
        class_decl(
            Final,
            "C",
            vec![],
            Some(("B", vec![])),
            vec![],
            vec![override_method_expr(
                "m",
                vec![("x", ty_named("String"))],
                Some(ty_named("String")),
                var("x"),
            )],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("unmatched override must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "`m` is marked `override` but does not override any method"
    );
}
