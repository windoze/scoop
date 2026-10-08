use super::super::*;

// --- positive: structs ---

#[test]
fn struct_construction_and_field_access() {
    let file = file(vec![
        struct_decl(
            "Point",
            vec![("x", ty_named("Int")), ("y", ty_named("Int"))],
        ),
        fun(
            "main",
            vec![
                val("p", call("Point", vec![int_lit(1), int_lit(2)])),
                val("x", field(var("p"), "x")),
                stmt(call("println", vec![var("x")])),
            ],
        ),
    ]);
    let module = lower_user(file).expect("struct program must lower");

    // The struct type is allocated right after the well-known types.
    assert!(matches!(
        module.types[int_type(&module)],
        Type::Integer(hir::IntegerKind::SIGNED_32)
    ));
    let expected = include_str!("snapshots/struct_construction_and_field_access.hir.txt");
    assert_eq!(hir::dump(&module), expected);
}

#[test]
fn struct_init_node_also_constructs() {
    let file = file(vec![
        struct_decl("Point", vec![("x", ty_named("Int"))]),
        fun(
            "main",
            vec![val("p", struct_init("Point", vec![int_lit(1)]))],
        ),
    ]);
    let module = lower_user(file).expect("StructInit node must lower");
    let dump = hir::dump(&module);
    assert!(dump.contains("StructInit Point : Point"), "{dump}");
}

/// A struct and a function may share a name (separate namespaces). In
/// `Name(...)` call position the struct namespace wins: the expression
/// is a construction, not a call.
#[test]
fn struct_shadows_function_in_call_position() {
    let file = file(vec![
        struct_decl("Point", vec![("x", ty_named("Int"))]),
        fun("Point", vec![]),
        fun("main", vec![val("p", call("Point", vec![int_lit(5)]))]),
    ]);
    let module = lower_user(file).expect("struct/function name sharing must lower");
    let dump = hir::dump(&module);
    assert!(dump.contains("StructInit Point : Point"), "{dump}");
    assert!(dump.contains("fun Point"), "{dump}");
}

#[test]
fn struct_fields_may_reference_later_structs() {
    let file = file(vec![
        struct_decl("A", vec![("b", ty_named("B"))]),
        struct_decl("B", vec![("v", ty_named("Int"))]),
        fun(
            "main",
            vec![
                val("a", call("A", vec![call("B", vec![int_lit(1)])])),
                val("v", field(field(var("a"), "b"), "v")),
                stmt(call("println", vec![var("v")])),
            ],
        ),
    ]);
    lower_user(file).expect("forward struct references must lower");
}
