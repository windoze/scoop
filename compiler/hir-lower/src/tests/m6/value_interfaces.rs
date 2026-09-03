use super::*;

// --- positive: value types implementing interfaces (spec 4.4.3) ---

/// `struct S(val v: Int) : Describable { override fun describe(): String = "S" }`.
fn describable_s() -> Decl {
    struct_decl_full(
        "S",
        vec![("v", ty_named("Int"))],
        vec!["Describable"],
        vec![override_method_expr(
            "describe",
            vec![],
            Some(ty_named("String")),
            str_lit("S"),
        )],
    )
}

#[test]
fn struct_implements_interface_and_boxes() {
    let file = file(vec![
        describable(),
        describable_s(),
        fun(
            "main",
            vec![
                // Value type → its interface: boxed (spec 4.4.4).
                val_ty(
                    "d",
                    Some(ty_named("Describable")),
                    struct_init("S", vec![int_lit(1)]),
                ),
            ],
        ),
        // Interface dispatch on a boxed value type resolves to the
        // interface method (the adjust thunk is mir-lower's job).
        fun_expr(
            "show",
            vec![],
            vec![("d", ty_named("Describable"))],
            Some(ty_named("String")),
            method_call(var("d"), "describe", vec![]),
        ),
    ]);
    let module = lower_user(file).expect("a value-type implementation must lower");

    // The interface list is recorded on the struct declaration.
    let s_id = module
        .structs
        .iter()
        .find(|(_, s)| s.name == "S")
        .map(|(id, _)| id)
        .unwrap();
    assert_eq!(
        module.structs[s_id].interfaces,
        vec![interface_ty(&module, "Describable")]
    );

    let main = body_of(&module, "main");
    let init = local_init(main, "d");
    assert!(matches!(init.kind, hir::ExprKind::Box(_)));
    assert!(matches!(module.types[init.ty], hir::Type::Interface(..)));
    match &returned(body_of(&module, "show")).kind {
        hir::ExprKind::MethodCall { callee, .. } => {
            assert_eq!(
                module.functions[module.callable_function(*callee)].name,
                "Describable.describe"
            );
        }
        other => panic!("expected a method call, found {other:?}"),
    }
}

#[test]
fn enum_implements_interface() {
    let file = file(vec![
        describable(),
        enum_decl_full(
            "Mark",
            vec![],
            vec![variant_unit("On"), variant_unit("Off")],
            vec!["Describable"],
            vec![override_method_expr(
                "describe",
                vec![],
                Some(ty_named("String")),
                str_lit("mark"),
            )],
        ),
        fun_expr(
            "f",
            vec![],
            vec![("m", ty_named("Mark"))],
            Some(ty_named("Describable")),
            var("m"),
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user(file).expect("an enum implementation must lower");
    // Enum → interface at a return position: Box.
    assert!(matches!(
        returned(body_of(&module, "f")).kind,
        hir::ExprKind::Box(_)
    ));
}

#[test]
fn value_type_missing_an_implementation_is_an_error() {
    let file = file(vec![
        describable(),
        struct_decl_full(
            "S",
            vec![("v", ty_named("Int"))],
            vec!["Describable"],
            vec![],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("an unimplemented interface must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "struct `S` does not implement interface method `Describable.describe`"
    );
}

#[test]
fn enum_missing_an_implementation_is_an_error() {
    let file = file(vec![
        describable(),
        enum_decl_full(
            "Mark",
            vec![],
            vec![variant_unit("On")],
            vec!["Describable"],
            vec![],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("an unimplemented interface must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "enum `Mark` does not implement interface method `Describable.describe`"
    );
}

#[test]
fn value_type_implementation_requires_the_override_modifier() {
    let file = file(vec![
        describable(),
        struct_decl_full(
            "S",
            vec![("v", ty_named("Int"))],
            vec!["Describable"],
            vec![method_expr(
                "describe",
                vec![],
                Some(ty_named("String")),
                str_lit("S"),
            )],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("a missing `override` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "`describe` overrides `Describable.describe` and must be marked `override`"
    );
}

#[test]
fn value_type_override_without_an_interface_is_an_error() {
    let file = file(vec![
        struct_decl_full(
            "S",
            vec![("v", ty_named("Int"))],
            vec![],
            vec![method_full(
                true,
                false,
                "m",
                vec![],
                None,
                FunctionBody::Block(block(vec![])),
            )],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("a spurious `override` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "`m` is marked `override` but does not override any method"
    );
}

#[test]
fn ref_ne_lowers_to_ref_ne() {
    let file = file(vec![
        describable(),
        shape(),
        fun_expr(
            "f",
            vec![],
            vec![("a", ty_named("Any")), ("s", ty_named("Shape"))],
            Some(ty_named("Boolean")),
            binary(BinOp::RefNe, var("a"), var("s")),
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user(file).expect("`!==` must lower");
    assert!(matches!(
        returned(body_of(&module, "f")).kind,
        hir::ExprKind::Binary {
            op: hir::BinOp::RefNe,
            ..
        }
    ));
}
