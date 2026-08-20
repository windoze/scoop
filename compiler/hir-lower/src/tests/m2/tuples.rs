use super::super::*;

// --- positive: tuples ---

#[test]
fn tuples_and_indexing() {
    let file = file(vec![fun(
        "main",
        vec![
            var_("q", tuple_lit(vec![int_lit(1), str_lit("hello")])),
            val("s", index(var("q"), 2)),
            stmt(call("println", vec![var("s")])),
            val("u", unit_lit()),
            val_ty(
                "t",
                Some(ty_tuple(vec![ty_named("Int")])),
                tuple_lit(vec![int_lit(42)]),
            ),
            stmt(call("print", vec![index(var("t"), 1)])),
        ],
    )]);
    let module = lower_user(file).expect("tuple program must lower");
    let expected = include_str!("snapshots/tuples_and_indexing.hir.txt");
    assert_eq!(hir::dump(&module), expected);
}

#[test]
fn structs_and_tuples_nest() {
    let file = file(vec![
        struct_decl(
            "Wrap",
            vec![("pair", ty_tuple(vec![ty_named("Int"), ty_named("String")]))],
        ),
        struct_decl(
            "Point",
            vec![("x", ty_named("Int")), ("y", ty_named("Int"))],
        ),
        fun(
            "main",
            vec![
                val(
                    "w",
                    call("Wrap", vec![tuple_lit(vec![int_lit(1), str_lit("one")])]),
                ),
                val("n", index(field(var("w"), "pair"), 1)),
                val(
                    "p",
                    tuple_lit(vec![
                        call("Point", vec![int_lit(1), int_lit(2)]),
                        int_lit(3),
                    ]),
                ),
                val("q", index(var("p"), 1)),
                val(
                    "same",
                    binary(
                        BinOp::Eq,
                        var("q"),
                        call("Point", vec![int_lit(1), int_lit(2)]),
                    ),
                ),
                val(
                    "same2",
                    binary(
                        BinOp::Eq,
                        tuple_lit(vec![int_lit(1), str_lit("a")]),
                        tuple_lit(vec![int_lit(1), str_lit("a")]),
                    ),
                ),
                stmt(call("println", vec![var("n")])),
                stmt(call("println", vec![var("same")])),
                stmt(call("println", vec![var("same2")])),
            ],
        ),
    ]);
    lower_user(file).expect("nested struct/tuple program must lower");
}
