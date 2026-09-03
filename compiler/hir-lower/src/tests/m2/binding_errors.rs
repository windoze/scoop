use super::super::*;

// --- negative: declarations and assignments ---

#[test]
fn initializer_type_mismatch_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![val_ty("x", Some(ty_named("Int")), str_lit("s"))],
    )]);
    let errors = lower_user(file).expect_err("annotation mismatch must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "initializer of `x` must be of type Int, found String"
    );
}

#[test]
fn unknown_annotation_type_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![val_ty("x", Some(ty_named("Foo")), int_lit(1))],
    )]);
    let errors = lower_user(file).expect_err("unknown annotation must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "unknown type `Foo`");
}

#[test]
fn redeclaration_in_same_scope_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![val("x", int_lit(1)), val("x", int_lit(2))],
    )]);
    let errors = lower_user(file).expect_err("redeclaration must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "`x` is already declared in this scope");
}

#[test]
fn assign_to_immutable_variable_is_an_error_with_target_span() {
    let target_span = Span::new(20, 21);
    let mut assignment = assign("x", int_lit(2));
    if let StatementKind::Assign(a) = &mut assignment.kind {
        let ast::AssignTarget::Name(name) = &mut a.target else {
            panic!("the assign builder produces a local target");
        };
        name.span = target_span;
    }
    let file = file(vec![fun("main", vec![val("x", int_lit(1)), assignment])]);
    let errors = lower_user(file).expect_err("assigning to a val must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "cannot assign to immutable variable `x`");
    assert_eq!(errors[0].span, Some(target_span));
}

#[test]
fn assign_to_unknown_variable_is_an_error() {
    let file = file(vec![fun("main", vec![assign("x", int_lit(2))])]);
    let errors = lower_user(file).expect_err("assigning to an unknown name must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "unknown variable `x`");
}

#[test]
fn assign_type_mismatch_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![var_("x", int_lit(1)), assign("x", str_lit("s"))],
    )]);
    let errors = lower_user(file).expect_err("assignment mismatch must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "cannot assign value of type String to `x` of type Int"
    );
}

#[test]
fn variable_out_of_scope_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![
            block_stmt(vec![val("y", int_lit(1))]),
            stmt(call("println", vec![var("y")])),
        ],
    )]);
    let errors = lower_user(file).expect_err("out-of-scope reference must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "unknown variable `y`");
}

#[test]
fn if_body_does_not_leak_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![
            if_stmt(bool_lit(true), vec![val("y", int_lit(1))], None),
            stmt(call("println", vec![var("y")])),
        ],
    )]);
    let errors = lower_user(file).expect_err("out-of-scope reference must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "unknown variable `y`");
}

#[test]
fn unknown_variable_read_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![stmt(call("println", vec![var("y")]))],
    )]);
    let errors = lower_user(file).expect_err("unknown variable must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "unknown variable `y`");
}
