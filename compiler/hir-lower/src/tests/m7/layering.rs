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

/// A lexical declaration only shadows lower layers when it is applicable.
/// The local `choose(Int)` therefore does not prevent the top-level
/// `choose(String)` from handling this call.
#[test]
fn inapplicable_local_layer_falls_through_to_top_level() {
    let file = file(vec![
        fun_expr(
            "choose",
            vec![],
            vec![("value", ty_named("String"))],
            Some(ty_named("String")),
            var("value"),
        ),
        fun(
            "main",
            vec![
                local_fun_sig(
                    "choose",
                    vec![],
                    vec![("value", ty_named("Int"))],
                    Some(ty_named("Int")),
                    vec![ret(Some(var("value")))],
                ),
                val("result", call("choose", vec![str_lit("selected")])),
            ],
        ),
    ]);
    let module = lower_user(file).expect("an inapplicable local layer must be skipped");
    let body = body_of(&module, module.entry);
    let hir::StatementKind::ValDecl { init, .. } = &body.statements[1].kind else {
        panic!("expected the result declaration")
    };
    let hir::ExprKind::Call { callee, .. } = &init.kind else {
        panic!("the top-level layer must win")
    };
    assert_eq!(
        module.callable_function(*callee),
        top_level_fn(&module, "choose", &["String"])
    );
}

/// An implicit member layer is likewise skipped when none of its candidates
/// accepts the source arguments.
#[test]
fn inapplicable_member_layer_falls_through_to_top_level() {
    let file = file(vec![
        fun_expr(
            "choose",
            vec![],
            vec![("value", ty_named("String"))],
            Some(ty_named("String")),
            var("value"),
        ),
        class_decl(
            Final,
            "Host",
            vec![],
            None,
            vec![],
            vec![
                method_expr(
                    "choose",
                    vec![("value", ty_named("Int"))],
                    Some(ty_named("Int")),
                    var("value"),
                ),
                method_expr(
                    "probe",
                    vec![],
                    Some(ty_named("String")),
                    call("choose", vec![str_lit("selected")]),
                ),
            ],
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user(file).expect("an inapplicable member layer must be skipped");
    let probe = method_fn(&module, "Host", "probe", &[]);
    let body = body_of(&module, probe);
    let hir::StatementKind::Return { value: Some(value) } = &body.statements[0].kind else {
        panic!("expected a return")
    };
    let hir::ExprKind::Call { callee, .. } = &value.kind else {
        panic!("the top-level layer must win")
    };
    assert_eq!(
        module.callable_function(*callee),
        top_level_fn(&module, "choose", &["String"])
    );
}

/// For an explicit receiver, an inapplicable real member does not shadow an
/// applicable extension in the next layer.
#[test]
fn inapplicable_member_layer_falls_through_to_extension() {
    let file = file(vec![
        class_decl(
            Final,
            "Host",
            vec![],
            None,
            vec![],
            vec![method_expr(
                "choose",
                vec![("value", ty_named("Int"))],
                Some(ty_named("Int")),
                var("value"),
            )],
        ),
        extension_expr(
            ty_named("Host"),
            "choose",
            vec![],
            vec![("value", ty_named("String"))],
            Some(ty_named("String")),
            var("value"),
        ),
        fun(
            "main",
            vec![val(
                "result",
                method_call(
                    struct_init("Host", vec![]),
                    "choose",
                    vec![str_lit("selected")],
                ),
            )],
        ),
    ]);
    let module = lower_user(file).expect("an applicable extension layer must be reached");
    let body = body_of(&module, module.entry);
    let hir::StatementKind::ValDecl { init, .. } = &body.statements[0].kind else {
        panic!("expected the result declaration")
    };
    let hir::ExprKind::Call { callee, args } = &init.kind else {
        panic!("an extension is emitted as a direct call")
    };
    assert_eq!(
        args.len(),
        2,
        "the receiver is the first extension argument"
    );
    assert_eq!(
        module.callable_function(*callee),
        top_level_fn(&module, "choose", &["Host", "String"])
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

/// A same-side declaration with the requested name does not shadow the core
/// implicit-import layer unless it is applicable.
#[test]
fn inapplicable_user_layer_falls_through_to_core() {
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
    let module = lower_user(file).expect("the applicable core layer must be reached");
    let (target, _) = call_in_main(&module, 0, false);
    assert_eq!(target, top_level_fn(&module, "print", &["T0"]));
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
