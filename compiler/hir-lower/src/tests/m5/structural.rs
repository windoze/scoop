use super::*;

// --- positive: structural ---

/// The expected type reaches literals in argument position (including
/// the empty literal) and at `return`.
#[test]
fn literal_inference_in_argument_and_return_positions() {
    let file = file(vec![
        fun_sig(
            "consume",
            vec![],
            vec![("m", ty_mutable_int_array())],
            None,
            vec![stmt(call("println", vec![field(var("m"), "size")]))],
        ),
        fun_sig(
            "make",
            vec![],
            vec![],
            Some(ty_int_array()),
            vec![ret(Some(array_lit(vec![int_lit(1)])))],
        ),
        fun(
            "main",
            vec![
                stmt(call(
                    "consume",
                    vec![array_lit(vec![int_lit(1), int_lit(2)])],
                )),
                stmt(call("consume", vec![array_lit(vec![])])),
                val("a", call("make", vec![])),
            ],
        ),
    ]);
    let module = lower_user(file).expect("argument-position literals must lower");

    // The argument literals take the parameter's kind, even the empty
    // one.
    let main_body = match &module.functions[module.entry].kind {
        FunctionKind::User(body) => body,
        FunctionKind::Intrinsic(_) | FunctionKind::Extern(_) | FunctionKind::DerivedEquality => {
            panic!("main is a user function")
        }
    };
    for statement in &main_body.statements[..2] {
        let hir::StatementKind::Expr(expr) = &statement.kind else {
            panic!("expected an expression statement");
        };
        let hir::ExprKind::Call { args, .. } = &expr.kind else {
            panic!("expected a call");
        };
        assert!(matches!(args[0].kind, hir::ExprKind::ArrayLiteral(_)));
        assert_eq!(hir::type_name(&module, args[0].ty), "MutableArray<Int>");
    }

    // The literal at `return` takes the function's return type.
    let make = module
        .top_level
        .iter()
        .find(|&&id| module.functions[id].name == "make")
        .copied()
        .expect("make is declared");
    let make_body = match &module.functions[make].kind {
        FunctionKind::User(body) => body,
        FunctionKind::Intrinsic(_) | FunctionKind::Extern(_) | FunctionKind::DerivedEquality => {
            panic!("make is a user function")
        }
    };
    let hir::StatementKind::Return { value: Some(value) } = &make_body.statements[0].kind else {
        panic!("expected a return with a value");
    };
    assert!(matches!(value.kind, hir::ExprKind::ArrayLiteral(_)));
    assert_eq!(hir::type_name(&module, value.ty), "Array<Int>");
}

/// Value-type elements: structs inline in the literal, and a field of a
/// subscripted element is directly accessible.
#[test]
fn struct_elements_and_field_through_subscript() {
    let file = file(vec![
        struct_decl(
            "Point",
            vec![("x", ty_named("Int")), ("y", ty_named("Int"))],
        ),
        fun(
            "main",
            vec![
                val(
                    "ps",
                    array_lit(vec![
                        struct_init("Point", vec![int_lit(1), int_lit(2)]),
                        struct_init("Point", vec![int_lit(3), int_lit(4)]),
                    ]),
                ),
                val("x", field(subscript(var("ps"), int_lit(1)), "x")),
            ],
        ),
    ]);
    let module = lower_user(file).expect("struct element program must lower");
    let body = match &module.functions[module.entry].kind {
        FunctionKind::User(body) => body,
        FunctionKind::Intrinsic(_) | FunctionKind::Extern(_) | FunctionKind::DerivedEquality => {
            panic!("main is a user function")
        }
    };
    let locals: Vec<String> = body
        .locals
        .iter()
        .map(|(_, local)| hir::type_name(&module, local.ty))
        .collect();
    assert_eq!(locals, ["Array<Point>", "Int"]);
}

/// Array types are interned: the annotation and the literal share a
/// `TypeId`, and `Array<Int>` / `MutableArray<Int>` stay distinct
/// (invariance, spec 10.4).
#[test]
fn array_types_are_interned() {
    let file = file(vec![fun(
        "main",
        vec![
            val_ty("a", Some(ty_int_array()), array_lit(vec![int_lit(1)])),
            val_ty("b", Some(ty_int_array()), array_lit(vec![int_lit(2)])),
            val_ty("m", Some(ty_mutable_int_array()), array_lit(vec![])),
        ],
    )]);
    let module = lower_user(file).expect("interning program must lower");
    let body = match &module.functions[module.entry].kind {
        FunctionKind::User(body) => body,
        FunctionKind::Intrinsic(_) | FunctionKind::Extern(_) | FunctionKind::DerivedEquality => {
            panic!("main is a user function")
        }
    };
    let locals: Vec<TypeId> = body.locals.iter().map(|(_, local)| local.ty).collect();
    assert_eq!(locals[0], locals[1], "Array<Int> must be interned");
    assert_ne!(locals[0], locals[2], "Array and MutableArray differ");
    // The literal takes the annotated kind (`Expr::ty` is the
    // annotation's interned type).
    let hir::StatementKind::ValDecl { init, .. } = &body.statements[0].kind else {
        panic!("expected a val declaration");
    };
    assert_eq!(init.ty, locals[0]);
}

/// The conversion constructors resolve before user functions of the
/// same name (spec 10.4, milestone5 DESIGN.md 2.2).
#[test]
fn conversion_resolves_before_user_functions() {
    let file = file(vec![
        fun_expr(
            "Array",
            vec![],
            vec![("x", ty_named("Int"))],
            Some(ty_named("Int")),
            var("x"),
        ),
        fun(
            "main",
            vec![
                val_ty(
                    "m",
                    Some(ty_mutable_int_array()),
                    array_lit(vec![int_lit(1)]),
                ),
                val("b", call("Array", vec![var("m")])),
            ],
        ),
    ]);
    let module = lower_user(file).expect("conversion precedence program must lower");
    let body = match &module.functions[module.entry].kind {
        FunctionKind::User(body) => body,
        FunctionKind::Intrinsic(_) | FunctionKind::Extern(_) | FunctionKind::DerivedEquality => {
            panic!("main is a user function")
        }
    };
    let hir::StatementKind::ValDecl { init, .. } = &body.statements[1].kind else {
        panic!("expected a val declaration");
    };
    assert!(
        matches!(init.kind, hir::ExprKind::ArrayClone(_)),
        "`Array(m)` must be the conversion, not the user function"
    );
    assert_eq!(hir::type_name(&module, init.ty), "Array<Int>");
}

#[test]
fn conversion_uses_the_expected_result_to_type_its_source_literal() {
    let file = file(vec![fun(
        "main",
        vec![val_ty(
            "a",
            Some(ty_int_array()),
            call("Array", vec![array_lit(vec![int_lit(1)])]),
        )],
    )]);
    let module = lower_user(file).expect("conversion expectation must reach its source");
    let body = match &module.functions[module.entry].kind {
        FunctionKind::User(body) => body,
        FunctionKind::Intrinsic(_) | FunctionKind::Extern(_) | FunctionKind::DerivedEquality => {
            panic!("main is a user function")
        }
    };
    let hir::StatementKind::ValDecl { init, .. } = &body.statements[0].kind else {
        panic!("expected a val declaration")
    };
    let hir::ExprKind::ArrayClone(source) = &init.kind else {
        panic!("expected an array conversion")
    };
    assert_eq!(hir::type_name(&module, init.ty), "Array<Int>");
    assert_eq!(hir::type_name(&module, source.ty), "MutableArray<Int>");
}

#[test]
fn conversion_method_forms_clone_to_the_opposite_kind() {
    let file = file(vec![fun(
        "main",
        vec![
            val("a", array_lit(vec![int_lit(1)])),
            val_ty(
                "m",
                Some(ty_mutable_int_array()),
                array_lit(vec![int_lit(2)]),
            ),
            val("immutable", method_call(var("m"), "toArray", vec![])),
            val("mutable", method_call(var("a"), "toMutableArray", vec![])),
        ],
    )]);
    let module = lower_user(file).expect("array conversion methods must lower");
    let body = match &module.functions[module.entry].kind {
        FunctionKind::User(body) => body,
        FunctionKind::Intrinsic(_) | FunctionKind::Extern(_) | FunctionKind::DerivedEquality => {
            panic!("main is a user function")
        }
    };
    for (index, expected) in [(2, "Array<Int>"), (3, "MutableArray<Int>")] {
        let hir::StatementKind::ValDecl { init, .. } = &body.statements[index].kind else {
            panic!("expected a val declaration")
        };
        assert!(matches!(init.kind, hir::ExprKind::ArrayClone(_)));
        assert_eq!(hir::type_name(&module, init.ty), expected);
    }
}

#[test]
fn inapplicable_array_intrinsic_method_falls_through_to_an_extension() {
    let file = file(vec![
        extension_expr(
            ty_int_array(),
            "toMutableArray",
            vec![],
            vec![("fallback", ty_named("Int"))],
            Some(ty_named("Int")),
            var("fallback"),
        ),
        fun(
            "main",
            vec![
                val("a", array_lit(vec![int_lit(1)])),
                val(
                    "result",
                    method_call(var("a"), "toMutableArray", vec![int_lit(7)]),
                ),
            ],
        ),
    ]);
    let module =
        lower_user(file).expect("an inapplicable intrinsic member must not shadow extensions");
    let body = match &module.functions[module.entry].kind {
        FunctionKind::User(body) => body,
        FunctionKind::Intrinsic(_) | FunctionKind::Extern(_) | FunctionKind::DerivedEquality => {
            panic!("main is a user function")
        }
    };
    let hir::StatementKind::ValDecl { init, .. } = &body.statements[1].kind else {
        panic!("expected a val declaration")
    };
    assert!(matches!(init.kind, hir::ExprKind::Call { .. }));
    assert_eq!(hir::type_name(&module, init.ty), "Int");
}
