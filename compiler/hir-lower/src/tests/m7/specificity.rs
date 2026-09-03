use super::*;

// --- resolution: most specific candidate (DESIGN 1.2, step 3) ---

/// `open class Shape(name) : Describable`, `class Circle(r) : Shape("c")`,
/// and `tag(Shape)` / `tag(Describable)` overloads (the DESIGN.md 1
/// example): `tag(Circle(1))` must pick the `Shape` overload.
#[test]
fn most_specific_subtype_wins() {
    let file = file(vec![
        interface_decl(
            "Describable",
            vec![bodyless_method(
                false,
                "describe",
                vec![],
                Some(ty_named("String")),
            )],
        ),
        class_decl(
            Open,
            "Shape",
            vec![(false, "name", ty_named("String"))],
            None,
            vec!["Describable"],
            vec![override_method_expr(
                "describe",
                vec![],
                Some(ty_named("String")),
                var("name"),
            )],
        ),
        class_decl(
            Final,
            "Circle",
            vec![(false, "r", ty_named("Int"))],
            Some(("Shape", vec![str_lit("c")])),
            vec![],
            vec![],
        ),
        fun_expr(
            "tag",
            vec![],
            vec![("s", ty_named("Shape"))],
            Some(ty_named("String")),
            str_lit("shape"),
        ),
        fun_expr(
            "tag",
            vec![],
            vec![("d", ty_named("Describable"))],
            Some(ty_named("String")),
            str_lit("desc"),
        ),
        fun(
            "main",
            vec![stmt(call(
                "println",
                vec![call("tag", vec![struct_init("Circle", vec![int_lit(1)])])],
            ))],
        ),
    ]);
    let module = lower_user(file).expect("subtype resolution must succeed");
    let (target, _) = call_in_main(&module, 0, true);
    assert_eq!(target, top_level_fn(&module, "tag", &["Shape"]));
}

/// No separate boxing preference rule: `Int <: Any` makes the `Int`
/// overload dominate naturally; an `Any`-typed argument only matches
/// the `Any` overload (and needs no adaptation there).
#[test]
fn boxing_candidate_is_naturally_less_specific() {
    let file = file(vec![
        fun_expr(
            "f",
            vec![],
            vec![("x", ty_named("Int"))],
            Some(ty_named("String")),
            str_lit("int"),
        ),
        fun_expr(
            "f",
            vec![],
            vec![("x", ty_named("Any"))],
            Some(ty_named("String")),
            str_lit("any"),
        ),
        fun(
            "main",
            vec![
                val_ty("a", Some(ty_named("Any")), int_lit(1)),
                stmt(call("println", vec![call("f", vec![int_lit(1)])])),
                stmt(call("println", vec![call("f", vec![var("a")])])),
            ],
        ),
    ]);
    let module = lower_user(file).expect("boxing resolution must succeed");
    let (first, args) = call_in_main(&module, 1, true);
    assert_eq!(first, top_level_fn(&module, "f", &["Int"]));
    // The exact-match overload takes the literal unboxed.
    assert!(matches!(args[0].kind, hir::ExprKind::IntLiteral(1)));
    let (second, _) = call_in_main(&module, 2, true);
    assert_eq!(second, top_level_fn(&module, "f", &["Any"]));
}

/// `f(A, B)` and `f(B, A)` with `B <: A` are mutually non-dominating
/// for a `(B, B)` call — ambiguous.
#[test]
fn mutually_non_dominating_overloads_are_ambiguous() {
    let file = file(vec![
        class_decl(Open, "A", vec![], None, vec![], vec![]),
        class_decl(Final, "B", vec![], Some(("A", vec![])), vec![], vec![]),
        fun_expr(
            "f",
            vec![],
            vec![("a", ty_named("A")), ("b", ty_named("B"))],
            Some(ty_named("String")),
            str_lit("ab"),
        ),
        fun_expr(
            "f",
            vec![],
            vec![("a", ty_named("B")), ("b", ty_named("A"))],
            Some(ty_named("String")),
            str_lit("ba"),
        ),
        fun(
            "main",
            vec![stmt(call(
                "f",
                vec![struct_init("B", vec![]), struct_init("B", vec![])],
            ))],
        ),
    ]);
    let errors = lower_user(file).expect_err("the call must be ambiguous");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "call to `f` is ambiguous in current-unit top-level candidate layer:\n  - fun f(a: A, b: B): String — tied after pairwise declaration forwarding\n  - fun f(a: B, b: A): String — tied after pairwise declaration forwarding"
    );
}

/// A generic and a non-generic candidate dominating each other
/// (the generic one with its inferred type arguments) is a tie,
/// broken in favor of the non-generic one; arguments the non-generic
/// candidate does not match still go to the generic one.
#[test]
fn non_generic_wins_ties_against_generic() {
    let file = file(vec![
        fun_expr(
            "id",
            vec!["T"],
            vec![("x", ty_named("T"))],
            Some(ty_named("T")),
            var("x"),
        ),
        fun_expr(
            "id",
            vec![],
            vec![("x", ty_named("Int"))],
            Some(ty_named("Int")),
            var("x"),
        ),
        fun(
            "main",
            vec![
                stmt(call("println", vec![call("id", vec![int_lit(1)])])),
                stmt(call("println", vec![call("id", vec![str_lit("s")])])),
            ],
        ),
    ]);
    let module = lower_user(file).expect("generic overloads must resolve");
    let generic = top_level_fn(&module, "id", &["T0"]);
    let concrete = top_level_fn(&module, "id", &["Int"]);

    // `id(1)`: the non-generic candidate wins the tie; no type
    // arguments, no instantiation request.
    let (first, _) = call_in_main(&module, 0, true);
    assert_eq!(first, concrete);
    let body = body_of(&module, module.entry);
    let hir::StatementKind::Expr(outer) = &body.statements[0].kind else {
        panic!("expected a call statement")
    };
    let hir::ExprKind::Call { args, .. } = &outer.kind else {
        panic!("expected a call")
    };
    let hir::ExprKind::Call { callee, .. } = &args[0].kind else {
        panic!("expected a nested call")
    };
    assert!(matches!(callee, hir::Callable::Function(id) if *id == concrete));

    // `id("s")`: only the generic candidate is applicable, with
    // `T = String` inferred and recorded.
    let (second, _) = call_in_main(&module, 1, true);
    assert_eq!(second, generic);
    assert!(has_instantiation(&module, generic, &[module.string]));
    assert!(!has_instantiation(&module, generic, &[module.int]));
}

#[test]
fn generic_forwarding_does_not_read_call_inference_results() {
    let file = file(vec![
        fun_expr(
            "merge",
            vec!["T"],
            vec![("first", ty_named("T")), ("second", ty_named("T"))],
            Some(ty_named("String")),
            str_lit("same"),
        ),
        fun_expr(
            "merge",
            vec!["U"],
            vec![("first", ty_named("U")), ("second", ty_named("Any"))],
            Some(ty_named("String")),
            str_lit("wide"),
        ),
        fun(
            "main",
            vec![stmt(call("merge", vec![str_lit("left"), str_lit("right")]))],
        ),
    ]);
    let errors = lower_user(file).expect_err("fresh declaration forwarding stays tied");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "call to `merge` is ambiguous in current-unit top-level candidate layer:\n  - fun merge<T>(first: T, second: T): String — tied after pairwise declaration forwarding\n  - fun merge<U>(first: U, second: Any): String — tied after pairwise declaration forwarding"
    );
}
