use super::*;

// --- core `print` / `println` (DESIGN section 2, final form) ---

/// The core managed `write` extern.
fn core_write(module: &hir::Module) -> hir::FunctionId {
    module
        .top_level
        .iter()
        .copied()
        .find(|&id| {
            let f = &module.functions[id];
            f.name == "write" && matches!(f.kind, hir::FunctionKind::Extern(_))
        })
        .expect("core declares the write extern")
}

/// `print` / `println` are ordinary generic core functions bounded by
/// `ToString`; each call records its exact instantiation and preserves the
/// argument's concrete type. Their template body resolves `toString()`
/// through the declared interface bound.
#[test]
fn print_and_println_use_the_ordinary_to_string_bound() {
    let file = file(vec![fun(
        "main",
        vec![
            stmt(call("println", vec![int_lit(42)])),
            stmt(call("println", vec![str_lit("x")])),
            stmt(call("println", vec![bool_lit(true)])),
            stmt(call("print", vec![int_lit(1)])),
        ],
    )]);
    let module = lower_user(file).expect("the generic core functions must resolve");
    let println = top_level_fn(&module, "println", &["T0"]);
    let print = top_level_fn(&module, "print", &["T0"]);

    // All four calls resolve to the ordinary generic declarations.
    for (index, want) in [println, println, println, print].into_iter().enumerate() {
        let (target, _) = call_in_main(&module, index, false);
        assert_eq!(target, want);
    }

    assert!(has_instantiation(&module, println, &[module.int]));
    assert!(has_instantiation(&module, println, &[module.string]));
    assert!(has_instantiation(&module, println, &[module.boolean]));
    assert!(has_instantiation(&module, print, &[module.int]));

    // Arguments keep their exact types; formatting no longer crosses Any.
    let body = body_of(&module, module.entry);
    let hir::StatementKind::Expr(first) = &body.statements[0].kind else {
        panic!("expected a call statement")
    };
    let hir::ExprKind::Call { args, .. } = &first.kind else {
        panic!("expected a call")
    };
    assert!(matches!(args[0].kind, hir::ExprKind::IntLiteral(42)));
    assert_eq!(args[0].ty, module.int);
    let hir::StatementKind::Expr(second) = &body.statements[1].kind else {
        panic!("expected a call statement")
    };
    let hir::ExprKind::Call { args, .. } = &second.kind else {
        panic!("expected a call")
    };
    assert!(matches!(args[0].kind, hir::ExprKind::StringLiteral(_)));
    assert_eq!(args[0].ty, module.string);

    // Core's template body calls the managed write extern and carries an
    // exact bound-member identity for `ToString.toString`.
    let write = core_write(&module);
    let body = body_of(&module, print);
    let hir::StatementKind::Expr(value) = &body.statements[0].kind else {
        panic!("expected a call statement")
    };
    let hir::ExprKind::Call { callee, args } = &value.kind else {
        panic!("expected a call")
    };
    assert_eq!(module.callable_function(*callee), write);
    let hir::ExprKind::MethodCall {
        callee, receiver, ..
    } = &args[0].kind
    else {
        panic!("expected a `toString()` method call")
    };
    let hir::MethodCallee::Bound(bound) = callee else {
        panic!("generic print must retain a typed bound call")
    };
    let member = module.bound_callable_refs[*bound].member;
    let interface_method = module.interface_methods[member];
    assert_eq!(module.interfaces[interface_method.owner].name, "ToString");
    assert_eq!(
        module.functions[interface_method.function].name,
        "ToString.toString"
    );
    assert!(matches!(receiver.kind, hir::ExprKind::Local(_)));
}

/// `Any` has no capability members. A concrete runtime object cannot add
/// static ToString/Hash/equality support to an `Any` expression.
#[test]
fn any_receiver_has_no_implicit_capability_members() {
    let file = file(vec![fun(
        "main",
        vec![
            val_ty("a", Some(ty_named("Any")), int_lit(1)),
            val_ty("b", Some(ty_named("Any")), str_lit("x")),
            stmt(call(
                "println",
                vec![method_call(var("a"), "toString", vec![])],
            )),
            stmt(call(
                "println",
                vec![method_call(var("a"), "hashCode", vec![])],
            )),
            stmt(call(
                "println",
                vec![method_call(var("a"), "equals", vec![var("b")])],
            )),
        ],
    )]);
    let errors = lower_user(file).expect_err("`Any` must not expose implicit capabilities");
    assert_eq!(errors.len(), 3);
    assert!(
        errors[0]
            .message
            .contains("type `Any` has no method `toString`")
    );
    assert!(
        errors[1]
            .message
            .contains("type `Any` has no method `hashCode`")
    );
    assert!(
        errors[2]
            .message
            .contains("type `Any` has no method `equals`")
    );
}

/// Intrinsic value types expose the ordinary methods declared by their exact
/// source owner; method lookup does not use an `Any` fallback or a compiler
/// capability table.
#[test]
fn intrinsic_value_members_resolve_from_their_source_declaration() {
    let file = file(vec![fun(
        "main",
        vec![stmt(call(
            "println",
            vec![method_call(int_lit(1), "toString", vec![])],
        ))],
    )]);
    let module = lower_user(file).expect("Int.toString is declared in core source");
    let body = body_of(&module, module.entry);
    let hir::StatementKind::Expr(outer) = &body.statements[0].kind else {
        panic!("expected print call")
    };
    let hir::ExprKind::Call { args, .. } = &outer.kind else {
        panic!("expected print call")
    };
    let hir::ExprKind::MethodCall { callee, .. } = &args[0].kind else {
        panic!("expected Int.toString call")
    };
    let target = module.callable_function(*callee);
    assert_eq!(module.functions[target].name, "Int.toString");
    let body = body_of(&module, target);
    let hir::StatementKind::Return { value: Some(value) } = &body.statements[0].kind else {
        panic!("the ordinary core method must return its source expression")
    };
    let hir::ExprKind::Call { callee, .. } = value.kind else {
        panic!("the core method body must call its representation helper")
    };
    let helper = module.callable_function(callee);
    assert_eq!(module.functions[helper].name, "coreIntToString");
    assert!(matches!(
        module.functions[helper].kind,
        hir::FunctionKind::Extern(_)
    ));
}
