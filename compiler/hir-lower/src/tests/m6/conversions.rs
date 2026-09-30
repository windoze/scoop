use super::*;

// --- positive: boxing ---

#[test]
fn boxing_at_subtype_crossings() {
    let file = file(vec![
        struct_s(),
        fun(
            "main",
            vec![
                // Value type → Any at a `val` annotation: Box.
                val_ty(
                    "a",
                    Some(ty_named("Any")),
                    struct_init("S", vec![int_lit(1)]),
                ),
                // Value type → Any at an argument position: Box.
                stmt(call("take", vec![struct_init("S", vec![int_lit(2)])])),
            ],
        ),
        fun_sig("take", vec![], vec![("x", ty_named("Any"))], None, vec![]),
        // Value type → Any at a return position: Box.
        fun_expr(
            "give",
            vec![],
            vec![],
            Some(ty_named("Any")),
            struct_init("S", vec![int_lit(3)]),
        ),
    ]);
    let module = lower_user(file).expect("boxing must lower");

    let main = body_of(&module, "main");
    let annotated = local_init(main, "a");
    assert!(matches!(annotated.kind, hir::ExprKind::Box(_)));
    assert_eq!(module.types[annotated.ty], hir::Type::Any);
    assert_eq!(
        main.statements
            .iter()
            .filter(|statement| matches!(
                statement.kind,
                hir::StatementKind::ValDecl {
                    init: hir::Expr {
                        kind: hir::ExprKind::Box(_),
                        ..
                    },
                    ..
                }
            ))
            .count(),
        2,
        "the annotation and the call argument each box once"
    );
    assert!(matches!(
        returned(body_of(&module, "give")).kind,
        hir::ExprKind::Box(_)
    ));
}

#[test]
fn class_to_interface_is_a_zero_cost_retype() {
    let file = file(vec![
        describable(),
        shape(),
        fun_expr(
            "up",
            vec![],
            vec![("s", ty_named("Shape"))],
            Some(ty_named("Describable")),
            var("s"),
        ),
        // Array elements adapt too (annotation drives the element type).
        fun_expr(
            "mk",
            vec![],
            vec![("s", ty_named("Shape"))],
            Some(ty_generic("Array", vec![ty_named("Describable")])),
            array_lit(vec![var("s")]),
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user(file).expect("upcasts must lower");

    // The upcast preserves the source class without allocating a box.
    let up = returned(body_of(&module, "up"));
    let hir::ExprKind::ReferenceUpcast(source) = &up.kind else {
        panic!("a reference upcast retains its source expression")
    };
    assert!(matches!(source.kind, hir::ExprKind::Local(_)));
    assert!(matches!(module.types[source.ty], hir::Type::Class(..)));
    assert!(matches!(module.types[up.ty], hir::Type::Interface(..)));

    match &returned(body_of(&module, "mk")).kind {
        hir::ExprKind::ArrayLiteral(elements) => {
            assert!(matches!(
                elements[0].kind,
                hir::ExprKind::ReferenceUpcast(_)
            ));
            assert!(matches!(
                module.types[elements[0].ty],
                hir::Type::Interface(..)
            ));
        }
        other => panic!("expected an array literal, found {other:?}"),
    }
}

// --- positive: is / as / as? / === ---

#[test]
fn is_cast_and_ref_eq() {
    let file = file(vec![
        describable(),
        shape(),
        struct_s(),
        fun_expr(
            "check",
            vec![],
            vec![("a", ty_named("Any"))],
            Some(ty_named("Boolean")),
            is_ty(var("a"), ty_named("S"), false),
        ),
        // `as` to a value type: check keeps the reference, Unbox
        // extracts the payload.
        fun_expr(
            "down",
            vec![],
            vec![("a", ty_named("Any"))],
            Some(ty_named("S")),
            cast_ty(var("a"), ty_named("S"), false),
        ),
        // `as?`: Option<T>.
        fun_expr(
            "down_opt",
            vec![],
            vec![("a", ty_named("Any"))],
            Some(ty_nullable(ty_named("S"))),
            cast_ty(var("a"), ty_named("S"), true),
        ),
        // `as?` to a reference type.
        fun_expr(
            "side",
            vec![],
            vec![("d", ty_named("Describable"))],
            Some(ty_nullable(ty_named("Shape"))),
            cast_ty(var("d"), ty_named("Shape"), true),
        ),
        fun_expr(
            "same",
            vec![],
            vec![("a", ty_named("Any")), ("s", ty_named("Shape"))],
            Some(ty_named("Boolean")),
            binary(BinOp::RefEq, var("a"), var("s")),
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user(file).expect("type operators must lower");

    match &returned(body_of(&module, "check")).kind {
        hir::ExprKind::IsInstance { check_ty, .. } => {
            assert!(matches!(module.types[*check_ty], hir::Type::Struct(..)));
        }
        other => panic!("expected an is-check, found {other:?}"),
    }
    match &returned(body_of(&module, "down")).kind {
        hir::ExprKind::Unbox(operand) => assert!(matches!(
            operand.kind,
            hir::ExprKind::Cast {
                optional: false,
                ..
            }
        )),
        other => panic!("expected Unbox(Cast), found {other:?}"),
    }
    let down_opt = returned(body_of(&module, "down_opt"));
    match &down_opt.kind {
        hir::ExprKind::Cast { optional: true, .. } => match &module.types[down_opt.ty] {
            hir::Type::Enum(application) => {
                let application = &module.enum_applications[*application];
                assert_eq!(
                    application.template,
                    module.nominal_identities[defined_export_core(&module).option.enumeration()]
                        .declaration_id()
                );
                assert_eq!(application.arguments.len(), 1);
                assert!(matches!(
                    module.types[application.arguments[0]],
                    hir::Type::Struct(..)
                ));
            }
            other => panic!("expected Option<S>, found {other:?}"),
        },
        other => panic!("expected an optional cast, found {other:?}"),
    }
    match &returned(body_of(&module, "side")).kind {
        hir::ExprKind::Cast { optional: true, .. } => {}
        other => panic!("expected an optional cast, found {other:?}"),
    }
    // Reference identity remains the non-overloadable `===` operation.
    assert!(matches!(
        returned(body_of(&module, "same")).kind,
        hir::ExprKind::Binary {
            op: hir::BinOp::RefEq,
            ..
        }
    ));
}

// --- positive: smart casts ---

#[test]
fn smart_cast_narrows_value_types_with_unbox() {
    let file = file(vec![
        struct_s(),
        fun_sig(
            "f",
            vec![],
            vec![("x", ty_named("Any"))],
            Some(ty_named("Int")),
            vec![
                if_stmt(
                    is_ty(var("x"), ty_named("S"), false),
                    vec![ret(Some(field(var("x"), "v")))],
                    None,
                ),
                ret(Some(int_lit(0))),
            ],
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user(file).expect("smart casts must lower");

    let f = body_of(&module, "f");
    match &f.statements[0].kind {
        hir::StatementKind::If { then_body, .. } => {
            let value = return_value(then_body);
            match &value.kind {
                hir::ExprKind::FieldAccess { receiver, field } => {
                    assert!(matches!(
                        field,
                        hir::FieldRef::StructField { field, .. } if module.field_identities.struct_declaration(*field).unwrap().local_index() == 0
                    ));
                    // The narrowed access unboxes the Any local.
                    assert!(matches!(receiver.kind, hir::ExprKind::Unbox(_)));
                }
                other => panic!("expected a field access, found {other:?}"),
            }
        }
        other => panic!("expected an if, found {other:?}"),
    }
}

#[test]
fn smart_cast_narrows_class_references_for_free() {
    let file = file(vec![
        describable(),
        shape(),
        fun_sig(
            "f",
            vec![],
            vec![("x", ty_named("Any"))],
            Some(ty_named("String")),
            vec![
                if_stmt(
                    is_ty(var("x"), ty_named("Shape"), false),
                    vec![ret(Some(method_call(var("x"), "describe", vec![])))],
                    None,
                ),
                ret(Some(str_lit("?"))),
            ],
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user(file).expect("smart casts must lower");

    let f = body_of(&module, "f");
    match &f.statements[0].kind {
        hir::StatementKind::If { then_body, .. } => {
            let value = return_value(then_body);
            match &value.kind {
                hir::ExprKind::MethodCall {
                    receiver, callee, ..
                } => {
                    assert_eq!(
                        module.functions[module.callable_function(*callee)].name,
                        "Shape.describe"
                    );
                    // The receiver is the same local, retyped — no Unbox.
                    assert!(matches!(receiver.kind, hir::ExprKind::Local(_)));
                    assert!(matches!(module.types[receiver.ty], hir::Type::Class(..)));
                }
                other => panic!("expected a method call, found {other:?}"),
            }
        }
        other => panic!("expected an if, found {other:?}"),
    }
}

#[test]
fn smart_cast_applies_in_negated_else_and_and_rhs() {
    let file = file(vec![
        struct_s(),
        // `if (x !is S) ... else { x: S }`.
        fun_sig(
            "f",
            vec![],
            vec![("x", ty_named("Any"))],
            Some(ty_named("Int")),
            vec![
                if_stmt(
                    is_ty(var("x"), ty_named("S"), true),
                    vec![ret(Some(int_lit(0)))],
                    Some(vec![ret(Some(field(var("x"), "v")))]),
                ),
                ret(Some(int_lit(1))),
            ],
        ),
        // `x is S && x.v > 0`: the RHS already sees the narrowing.
        fun_sig(
            "g",
            vec![],
            vec![("x", ty_named("Any"))],
            Some(ty_named("Int")),
            vec![
                if_stmt(
                    binary(
                        BinOp::And,
                        is_ty(var("x"), ty_named("S"), false),
                        binary(BinOp::Gt, field(var("x"), "v"), int_lit(0)),
                    ),
                    vec![ret(Some(field(var("x"), "v")))],
                    None,
                ),
                ret(Some(int_lit(0))),
            ],
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user(file).expect("smart casts must lower");

    let f = body_of(&module, "f");
    match &f.statements[0].kind {
        hir::StatementKind::If {
            else_body: Some(else_body),
            ..
        } => match &else_body[0].kind {
            hir::StatementKind::Return { value: Some(value) } => match &value.kind {
                hir::ExprKind::FieldAccess { receiver, .. } => {
                    assert!(matches!(receiver.kind, hir::ExprKind::Unbox(_)));
                }
                other => panic!("expected a field access, found {other:?}"),
            },
            other => panic!("expected a return, found {other:?}"),
        },
        other => panic!("expected an if/else, found {other:?}"),
    }

    let g = body_of(&module, "g");
    match &g.statements[0].kind {
        hir::StatementKind::If { then_body, .. } => {
            let field = then_body
                .iter()
                .find_map(|statement| match &statement.kind {
                    hir::StatementKind::ValDecl { init, .. }
                        if matches!(init.kind, hir::ExprKind::FieldAccess { .. }) =>
                    {
                        Some(init)
                    }
                    _ => None,
                })
                .expect("short-circuit RHS field access");
            let hir::ExprKind::FieldAccess { receiver, .. } = &field.kind else {
                unreachable!("the selected expression is a field access")
            };
            assert!(matches!(receiver.kind, hir::ExprKind::Unbox(_)));
        }
        other => panic!("expected short-circuit setup, found {other:?}"),
    }
    assert!(matches!(
        g.statements[1].kind,
        hir::StatementKind::If { .. }
    ));
}
