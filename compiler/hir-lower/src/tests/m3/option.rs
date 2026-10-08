use super::super::*;

// --- positive: Option and nullables ---

#[test]
fn some_none_and_nullable_annotations() {
    let file = file(vec![fun(
        "main",
        vec![
            val_ty("a", Some(ty_nullable(ty_named("Int"))), some(int_lit(41))),
            val_ty("b", Some(ty_nullable(ty_named("Int"))), none()),
            // `T??` is `Option<Option<T>>` and does not collapse; the
            // expected type lets `Some(None)` type the inner `None`.
            val_ty(
                "c",
                Some(ty_nullable(ty_nullable(ty_named("Int")))),
                some(none()),
            ),
        ],
    )]);
    let module = lower_user(file).expect("Option constructors must lower");
    let expected = include_str!("snapshots/some_none_and_nullable_annotations.hir.txt");
    assert_eq!(hir::dump(&module), expected);
}

#[test]
fn safe_field_access_desugars_to_hidden_locals() {
    let file = file(vec![
        struct_decl(
            "Point",
            vec![("x", ty_named("Int")), ("y", ty_named("Int"))],
        ),
        fun(
            "main",
            vec![
                val_ty(
                    "p",
                    Some(ty_nullable(ty_named("Point"))),
                    some(call("Point", vec![int_lit(1), int_lit(2)])),
                ),
                val("x", safe_field(var("p"), "x")),
            ],
        ),
    ]);
    let module = lower_user(file).expect("safe field access must lower");
    let expected = include_str!("snapshots/safe_field_access_desugars_to_hidden_locals.hir.txt");
    assert_eq!(hir::dump(&module), expected);
}

#[test]
fn elvis_desugars_to_hidden_locals() {
    let file = file(vec![fun(
        "main",
        vec![
            val_ty("a", Some(ty_nullable(ty_named("Int"))), some(int_lit(41))),
            val("b", elvis(var("a"), int_lit(0))),
        ],
    )]);
    let module = lower_user(file).expect("elvis must lower");
    let expected = include_str!("snapshots/elvis_desugars_to_hidden_locals.hir.txt");
    assert_eq!(hir::dump(&module), expected);
}

/// `w?.p?.x` chains: `w?.p` is an `Option<Point>`, whose `?.x` is an
/// `Option<Int>` — the desugarings nest through the sink.
#[test]
fn chained_safe_field_access() {
    let file = file(vec![
        struct_decl("Point", vec![("x", ty_named("Int"))]),
        struct_decl("Wrap", vec![("p", ty_named("Point"))]),
        fun(
            "main",
            vec![
                val_ty(
                    "w",
                    Some(ty_nullable(ty_named("Wrap"))),
                    some(call("Wrap", vec![call("Point", vec![int_lit(1)])])),
                ),
                val("x", safe_field(safe_field(var("w"), "p"), "x")),
            ],
        ),
    ]);
    let module = lower_user(file).expect("chained safe access must lower");
    let dump = hir::dump(&module);
    // w?.p : Option<Point>, then ?.x : Option<Int>. Call materialization may
    // add locals before these desugaring sinks, so their typed names are the
    // stable part of this contract.
    assert!(dump.contains("Local w : Option<Wrap>"), "{dump}");
    assert!(dump.contains("SomeWrap : Option<Point>"), "{dump}");
    assert!(dump.contains("Local $res.1 : Option<Point>"), "{dump}");
    assert!(dump.contains("Local $res.3 : Option<Int>"), "{dump}");
}

#[test]
fn null_assert_unwraps_with_trap() {
    let file = file(vec![fun(
        "main",
        vec![
            val_ty("a", Some(ty_nullable(ty_named("Int"))), some(int_lit(41))),
            val("x", null_assert(var("a"))),
            stmt(call("println", vec![var("x")])),
        ],
    )]);
    let module = lower_user(file).expect("null assert must lower");
    let dump = hir::dump(&module);
    assert!(
        dump.contains("Unwrap trap=true : Int\n        Local a : Option<Int>"),
        "{dump}"
    );
}

/// `x == None` and `None == x`: the literal takes the other side's
/// type from the expected-type hint.
#[test]
fn none_equality_comparison() {
    for comparison in [
        binary(BinOp::Eq, var("a"), none()),
        binary(BinOp::Ne, none(), var("a")),
    ] {
        let file = file(vec![fun(
            "main",
            vec![
                val_ty("a", Some(ty_nullable(ty_named("Int"))), none()),
                if_stmt(
                    comparison,
                    vec![stmt(call("println", vec![str_lit("empty")]))],
                    None,
                ),
            ],
        )]);
        let module = lower_user(file).expect("`== None` must lower");
        let dump = hir::dump(&module);
        assert!(
            dump.contains("VariantConstruct Option.None<Int> : Option<Int>"),
            "{dump}"
        );
    }
}
