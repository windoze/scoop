use super::*;

// --- method overloads and layering (DESIGN 1.2, step 1) ---

/// Method overloads resolve by the same algorithm, for explicit
/// receivers (`c.m("a")`) and bare calls inside a method body
/// (`m(1)` meaning `this.m(1)`).
#[test]
fn method_overloads_resolve() {
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
                    Some(ty_named("String")),
                    str_lit("int"),
                ),
                method_expr(
                    "m",
                    vec![("x", ty_named("String"))],
                    Some(ty_named("String")),
                    str_lit("string"),
                ),
                method_expr(
                    "probe",
                    vec![],
                    Some(ty_named("String")),
                    call("m", vec![int_lit(1)]),
                ),
            ],
        ),
        fun(
            "main",
            vec![stmt(call(
                "println",
                vec![method_call(
                    struct_init("C", vec![]),
                    "m",
                    vec![str_lit("a")],
                )],
            ))],
        ),
    ]);
    let module = lower_user(file).expect("method overloads must resolve");

    // `c.m("a")` picks the `String` overload.
    let body = body_of(&module, module.entry);
    let hir::StatementKind::Expr(outer) = &body.statements[0].kind else {
        panic!("expected a call statement")
    };
    let hir::ExprKind::Call { args, .. } = &outer.kind else {
        panic!("expected a call")
    };
    let hir::ExprKind::MethodCall { callee, .. } = &args[0].kind else {
        panic!("expected a method call")
    };
    assert_eq!(
        module.callable_function(*callee),
        method_fn(&module, "C", "m", &["String"])
    );

    // The bare `m(1)` inside `probe` is `this.m(1)` and picks the
    // `Int` overload.
    let probe = method_fn(&module, "C", "probe", &[]);
    let body = body_of(&module, probe);
    let hir::StatementKind::Return { value: Some(value) } = &body.statements[0].kind else {
        panic!("expected a return")
    };
    let hir::ExprKind::MethodCall {
        callee, receiver, ..
    } = &value.kind
    else {
        panic!("expected a method call")
    };
    assert_eq!(
        module.callable_function(*callee),
        method_fn(&module, "C", "m", &["Int"])
    );
    assert!(matches!(receiver.kind, hir::ExprKind::Local(_)));
}

/// The member layer wins whole over the top-level layer: inside `C`'s
/// methods a bare `value()` is the member even though a same-name
/// top-level function exists.
#[test]
fn member_layer_shadows_top_level() {
    let file = file(vec![
        fun_expr("value", vec![], vec![], Some(ty_named("Int")), int_lit(1)),
        class_decl(
            Final,
            "C",
            vec![],
            None,
            vec![],
            vec![
                method_expr("value", vec![], Some(ty_named("Int")), int_lit(2)),
                method_expr(
                    "probe",
                    vec![],
                    Some(ty_named("Int")),
                    call("value", vec![]),
                ),
            ],
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user(file).expect("member shadowing must resolve");
    let probe = method_fn(&module, "C", "probe", &[]);
    let body = body_of(&module, probe);
    let hir::StatementKind::Return { value: Some(value) } = &body.statements[0].kind else {
        panic!("expected a return")
    };
    let hir::ExprKind::MethodCall { callee, .. } = &value.kind else {
        panic!(
            "the bare call must resolve to the member, found {:?}",
            value.kind
        )
    };
    assert_eq!(
        module.callable_function(*callee),
        method_fn(&module, "C", "value", &[])
    );
}

/// The user layer wins whole over the core implicit-import layer for
/// calls from the user file: a user `print()` shadows all three core
/// `print` overloads there.
#[test]
fn user_layer_shadows_core_overloads() {
    let file = file(vec![
        fun_expr(
            "print",
            vec![],
            vec![],
            None,
            call("write", vec![str_lit("user")]),
        ),
        fun("main", vec![stmt(call("print", vec![]))]),
    ]);
    let module = lower_user(file).expect("user shadowing must resolve");
    let (target, _) = call_in_main(&module, 0, false);
    // The user's zero-parameter `print` (declared in file 1), not a
    // core overload.
    assert_eq!(target, top_level_fn(&module, "print", &[]));
}

/// The discarded layer really is gone: with only the user's `print()`
/// in scope, `print("x")` has no matching overload even though core
/// declares `print(String)`.
#[test]
fn shadowed_core_overloads_do_not_participate() {
    let file = file(vec![
        fun_expr(
            "print",
            vec![],
            vec![],
            None,
            call("write", vec![str_lit("user")]),
        ),
        fun("main", vec![stmt(call("print", vec![str_lit("x")]))]),
    ]);
    let errors = lower_user(file).expect_err("the core overloads must be discarded");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "function `print` takes exactly 0 arguments, but 1 were supplied"
    );
}

/// The layering is relative to the call site's file
/// (tests/fixtures/m7-overload/overload-core.scoop): the user's
/// `write(Int)` shadows core's `write` overloads for calls from the
/// user file, but the core library's own bodies still resolve their
/// internal calls against the core layer.
#[test]
fn layering_is_relative_to_the_call_site_file() {
    let file = file(vec![
        fun_expr(
            "write",
            vec![],
            vec![("v", ty_named("Int"))],
            Some(ty_named("String")),
            str_lit("user-write"),
        ),
        fun(
            "main",
            vec![
                stmt(call("println", vec![call("write", vec![int_lit(1)])])),
                stmt(call("println", vec![str_lit("c")])),
            ],
        ),
    ]);
    let module = lower_user(file).expect("call-site-relative layering must resolve");

    // The user call `write(1)` picks the user's `write(Int)` (same
    // side; core's `write(String)` is discarded).
    let (target, _) = call_in_main(&module, 0, true);
    assert_eq!(target, top_level_fn(&module, "write", &["Int"]));

    // Core's generic `print<T : ToString>` body still calls the core managed
    // `write` extern; the user overload does not leak into core's own layer.
    let core_write = module
        .top_level
        .iter()
        .copied()
        .find(|&id| {
            let f = &module.functions[id];
            f.name == "write" && matches!(f.kind, hir::FunctionKind::Extern(_))
        })
        .expect("core declares the write extern");
    let core_print = top_level_fn(&module, "print", &["T0"]);
    let body = body_of(&module, core_print);
    let hir::StatementKind::Expr(value) = &body.statements[0].kind else {
        panic!("expected the call statement")
    };
    let hir::ExprKind::Call { callee, .. } = &value.kind else {
        panic!("expected a call")
    };
    assert_eq!(module.callable_function(*callee), core_write);

    // Core's `println` body is likewise unaffected: both of its
    // `write` calls target the core extern.
    let core_println = top_level_fn(&module, "println", &["T0"]);
    let body = body_of(&module, core_println);
    for statement in &body.statements {
        let hir::StatementKind::Expr(value) = &statement.kind else {
            panic!("expected a call statement")
        };
        let hir::ExprKind::Call { callee, .. } = &value.kind else {
            panic!("expected a call")
        };
        assert_eq!(module.callable_function(*callee), core_write);
    }
}
