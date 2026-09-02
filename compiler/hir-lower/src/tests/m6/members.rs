use super::*;

// --- negative: method calls, fields, assignment ---

#[test]
fn unknown_method_is_an_error() {
    let file = file(vec![
        describable(),
        shape(),
        fun_expr(
            "f",
            vec![],
            vec![("s", ty_named("Shape"))],
            Some(ty_named("Int")),
            method_call(var("s"), "nope", vec![]),
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("an unknown method must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "type `Shape` has no method `nope`");
}

#[test]
fn unknown_class_field_is_an_error() {
    let file = file(vec![
        describable(),
        shape(),
        fun_expr(
            "f",
            vec![],
            vec![("s", ty_named("Shape"))],
            Some(ty_named("Int")),
            field(var("s"), "zzz"),
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("an unknown field must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "class `Shape` has no field `zzz`");
}

#[test]
fn assigning_a_val_property_is_an_error() {
    let file = file(vec![
        class_decl(
            Final,
            "C",
            vec![(false, "x", ty_named("Int"))],
            None,
            vec![],
            vec![method("m", vec![], None, vec![assign("x", int_lit(1))])],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("assigning a val property must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "cannot assign to immutable property `x`");
}

#[test]
fn bare_var_property_assignment_in_a_method_stores_through_this() {
    let file = file(vec![
        class_decl(
            Final,
            "C",
            vec![(true, "y", ty_named("Int"))],
            None,
            vec![],
            vec![method("m", vec![], None, vec![assign("y", int_lit(1))])],
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user(file).expect("var property assignment must lower");
    let body = body_of(&module, "C.m");
    match &body.statements[0].kind {
        hir::StatementKind::Assign { target, value } => {
            match target {
                hir::AssignTarget::Field { receiver, field } => {
                    assert!(matches!(receiver.kind, hir::ExprKind::Local(_)));
                    assert!(matches!(field, hir::FieldRef::ClassField { index: 0, .. }));
                }
                other => panic!("expected a field store, found {other:?}"),
            }
            assert!(matches!(value.kind, hir::ExprKind::IntLiteral(1)));
        }
        other => panic!("expected an assignment, found {other:?}"),
    }
}

#[test]
fn assigning_a_struct_field_in_a_method_is_an_error() {
    let file = file(vec![
        struct_decl_methods(
            "S",
            vec![("v", ty_named("Int"))],
            vec![method("m", vec![], None, vec![assign("v", int_lit(1))])],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("value-type fields stay unwritable");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "cannot assign to immutable property `v`");
}

#[test]
fn this_outside_a_member_function_is_an_error() {
    let file = file(vec![fun_expr(
        "main",
        vec![],
        vec![],
        Some(ty_named("Any")),
        this_expr(),
    )]);
    let errors = lower_user(file).expect_err("`this` at top level must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "`this` is only allowed inside member functions"
    );
}

// --- positive: field assignment ---

#[test]
fn field_assignment_on_a_var_property() {
    let bad = file(vec![
        describable(),
        shape(),
        point(),
        fun_sig(
            "f",
            vec![],
            vec![("p", ty_named("Point"))],
            None,
            vec![
                // `var` property, absolute layout index 2.
                assign_field(var("p"), "y", int_lit(3)),
                // Inherited `val` property would be rejected (see the
                // negative test); the store adapts the value type.
                assign_field(var("p"), "x", int_lit(4)),
            ],
        ),
        fun("main", vec![]),
    ]);
    // `x` is a `val` property — the second store must be rejected.
    let errors = lower_user(bad).expect_err("assigning a val property must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "cannot assign to immutable property `x`");

    let ok = file(vec![
        describable(),
        shape(),
        point(),
        fun_sig(
            "f",
            vec![],
            vec![("p", ty_named("Point"))],
            None,
            vec![assign_field(var("p"), "y", int_lit(3))],
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user(ok).expect("a var property store must lower");
    match &body_of(&module, "f").statements[0].kind {
        hir::StatementKind::Assign { target, value } => {
            match target {
                hir::AssignTarget::Field { receiver, field } => {
                    assert!(matches!(receiver.kind, hir::ExprKind::Local(_)));
                    assert_eq!(
                        *field,
                        hir::FieldRef::ClassField {
                            application: class_application(&module, class_id(&module, "Point")),
                            index: 2
                        }
                    );
                }
                other => panic!("expected a field store, found {other:?}"),
            }
            assert!(matches!(value.kind, hir::ExprKind::IntLiteral(3)));
        }
        other => panic!("expected an assignment, found {other:?}"),
    }
}

#[test]
fn field_assignment_on_a_value_type_is_an_error() {
    let file = file(vec![
        struct_s(),
        fun_sig(
            "f",
            vec![],
            vec![("s", ty_named("S"))],
            None,
            vec![assign_field(var("s"), "v", int_lit(1))],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("value-type field stores must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "field assignment is not supported (value types are immutable)"
    );
}

#[test]
fn field_assignment_value_type_is_checked() {
    let file = file(vec![
        describable(),
        shape(),
        point(),
        fun_sig(
            "f",
            vec![],
            vec![("p", ty_named("Point"))],
            None,
            vec![assign_field(var("p"), "y", str_lit("s"))],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("a wrong value type must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "cannot assign value of type String to property `y` of type Int"
    );
}
