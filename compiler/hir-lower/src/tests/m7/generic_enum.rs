use super::*;

// --- generic enum method overloads ---

/// Overloads on a generic enum instantiate with the receiver's type
/// arguments before applicability and dominance: `pick(T)` and
/// `pick(Int)` on a `Box<String>` select `pick(T)`; on a `Box<Int>`
/// the two tie after instantiation and the non-generic one wins.
#[test]
fn enum_method_overloads_instantiate_with_the_receiver() {
    let file = file(vec![
        enum_decl_methods(
            "Box",
            vec!["T"],
            vec![variant_positional("V", vec![ty_named("T")])],
            vec![
                method_expr(
                    "pick",
                    vec![("x", ty_named("T"))],
                    Some(ty_named("T")),
                    var("x"),
                ),
                method_expr(
                    "pick",
                    vec![("x", ty_named("Int"))],
                    Some(ty_named("Int")),
                    var("x"),
                ),
            ],
        ),
        fun(
            "main",
            vec![
                val("b", struct_init("Box.V", vec![str_lit("s")])),
                stmt(call(
                    "println",
                    vec![method_call(var("b"), "pick", vec![str_lit("a")])],
                )),
                val("i", struct_init("Box.V", vec![int_lit(1)])),
                stmt(call(
                    "println",
                    vec![method_call(var("i"), "pick", vec![int_lit(2)])],
                )),
            ],
        ),
    ]);
    let module = lower_user(file).expect("enum method overloads must resolve");
    let pick_t = method_fn(&module, "Box", "pick", &["T0"]);
    let pick_int = method_fn(&module, "Box", "pick", &["Int"]);

    let method_target = |index: usize| {
        let body = body_of(&module, module.entry);
        let hir::StatementKind::Expr(outer) = &body.statements[index].kind else {
            panic!("expected a call statement")
        };
        let hir::ExprKind::Call { args, .. } = &outer.kind else {
            panic!("expected a call")
        };
        let inner = match &args[0].kind {
            hir::ExprKind::Box(operand) => &operand.kind,
            kind => kind,
        };
        let hir::ExprKind::MethodCall { callee, .. } = inner else {
            panic!("expected a method call")
        };
        module.callable_function(*callee)
    };
    assert_eq!(method_target(1), pick_t);
    assert_eq!(method_target(3), pick_int);

    // The chosen enum methods request instantiations with the
    // receiver's type arguments.
    assert!(has_method_application(&module, pick_t, &[module.string]));
    assert!(has_method_application(&module, pick_int, &[module.int]));
}

// --- entry point ---

/// Overloads of `main` are ordinary functions; the zero-parameter one
/// is the entry point.
#[test]
fn overloaded_main_entry_is_the_zero_parameter_one() {
    let file = file(vec![
        fun_sig("main", vec![], vec![("x", ty_named("Int"))], None, vec![]),
        fun("main", vec![]),
    ]);
    let module = lower_user(file).expect("overloaded main must lower");
    let entry = &module.functions[module.entry];
    assert_eq!(entry.name, "main");
    assert!(entry.params.is_empty());
}
