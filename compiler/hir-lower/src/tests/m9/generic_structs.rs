use super::*;

// --- generic struct declarations and applications ---

#[test]
fn generic_struct_fields_construct_access_and_instantiate_methods() {
    let file = file(vec![
        generic_struct_decl_full(
            "Pair",
            vec!["A", "B"],
            vec![("first", ty_named("A")), ("second", ty_named("B"))],
            vec![],
            vec![method_expr(
                "getSecond",
                vec![],
                Some(ty_named("B")),
                var("second"),
            )],
        ),
        fun(
            "main",
            vec![
                val(
                    "pair",
                    struct_init("Pair", vec![int_lit(7), str_lit("seven")]),
                ),
                val("first", field(var("pair"), "first")),
                val("second", method_call(var("pair"), "getSecond", vec![])),
            ],
        ),
    ]);
    let module = lower_user(file).expect("generic struct must lower");
    assert_eq!(local_ty(&module, "pair"), "Pair<Int, String>");
    assert_eq!(local_ty(&module, "first"), "Int");
    assert_eq!(local_ty(&module, "second"), "String");
    assert!(
        hir::dump(&module).contains(
            "struct Pair<A, B>\n    field0 first: T0\n    field1 second: T1\n    property9 val first: T0 getter9=storage <stored struct16-field0>\n    property10 val second: T1 getter10=storage <stored struct16-field1>"
        )
    );
}

#[test]
fn nested_generic_struct_fields_participate_in_inference() {
    let file = file(vec![
        generic_struct_decl("Box", vec!["T"], vec![("value", ty_named("T"))]),
        generic_struct_decl(
            "Wrap",
            vec!["T"],
            vec![("box", ty_generic("Box", vec![ty_named("T")]))],
        ),
        fun(
            "main",
            vec![
                val(
                    "wrapped",
                    struct_init("Wrap", vec![struct_init("Box", vec![str_lit("value")])]),
                ),
                val("value", field(field(var("wrapped"), "box"), "value")),
            ],
        ),
    ]);
    let module = lower_user(file).expect("nested generic structs must lower");
    assert_eq!(local_ty(&module, "wrapped"), "Wrap<String>");
    assert_eq!(local_ty(&module, "value"), "String");
}

#[test]
fn duplicate_generic_struct_type_parameter_is_an_error() {
    let file = file(vec![
        generic_struct_decl("Bad", vec!["T", "T"], vec![("value", ty_named("T"))]),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("duplicate type parameter must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "duplicate type parameter `T`");
}
