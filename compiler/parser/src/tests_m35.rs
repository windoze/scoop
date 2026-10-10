//! M35 syntax boundaries: patterns are explicitly introduced by `case`.

use scoop_ast::{Pattern, StatementKind};

use crate::tests::{block_body, err, ok, only_function};

#[test]
fn case_is_contextual_and_introduces_the_entire_recursive_pattern() {
    let file = ok(
        "fun case(case: Int) {\n    when (case) {\n        case Some((value, _)) -> {}\n        case None -> {}\n    }\n}",
    );
    let function = only_function(&file);
    assert_eq!(function.name.text, "case");
    let StatementKind::When(when) = &block_body(function).statements[0].kind else {
        panic!("expected when");
    };
    let Pattern::Positional { elements, .. } = crate::tests::case_pattern(&when.arms[0]) else {
        panic!("expected variant pattern");
    };
    assert!(matches!(&elements[0], Pattern::Tuple { .. }));
    assert!(
        matches!(crate::tests::case_pattern(&when.arms[1]), Pattern::Binding(name) if name.text == "None")
    );
}

#[test]
fn missing_case_reports_the_pattern_start() {
    let source = "fun f() { when (value) { _ -> {} } }";
    let (span, message) = err(source);
    assert_eq!(&source[span.start as usize..span.end as usize], "_");
    assert_eq!(message, "match patterns require `case` before the pattern");
}

#[test]
fn binding_positions_do_not_require_case() {
    ok("fun f() { val (first, _) = pair; for ((value, _) in values) {} }");
}

#[test]
fn ordinary_conditions_and_subject_declarations_have_distinct_nodes() {
    let file = ok(
        "fun f() { when (val (first, second): (Int, Int) = pair) { 1, 2, -> first; is Pair if ready -> second; else if retry -> 0; else -> -1 } }",
    );
    let StatementKind::When(when) = &block_body(only_function(&file)).statements[0].kind else {
        panic!("expected when")
    };
    assert!(matches!(
        when.subject,
        scoop_ast::WhenSubject::Declaration(_)
    ));
    let scoop_ast::WhenArmCondition::Conditions(conditions) = &when.arms[0].condition else {
        panic!("expected ordinary conditions")
    };
    assert_eq!(conditions.as_slice().len(), 2);
    assert!(matches!(
        when.arms[2].condition,
        scoop_ast::WhenArmCondition::Else
    ));
    assert!(when.arms[1].guard.is_some());
    assert!(when.else_body.is_some());
}

#[test]
fn subjectless_when_has_boolean_conditions_and_control_bodies() {
    let file =
        ok("fun f() { when { ready -> return; !done -> throw failure; else -> value = 1 } }");
    let StatementKind::When(when) = &block_body(only_function(&file)).statements[0].kind else {
        panic!("expected when")
    };
    assert!(matches!(when.subject, scoop_ast::WhenSubject::Absent));
    assert!(matches!(
        when.arms[0].body.statements[0].kind,
        StatementKind::Return { .. }
    ));
    assert!(matches!(
        when.arms[1].body.statements[0].kind,
        StatementKind::Throw(_)
    ));
}

#[test]
fn case_and_ordinary_conditions_can_alternate() {
    let file = ok(
        "fun f() { when (value) { case Some(x) if x > 0 -> x; None -> 0; in allowed -> 1; !in denied -> 2; !is Missing -> 3; else -> -1 } }",
    );
    let StatementKind::When(when) = &block_body(only_function(&file)).statements[0].kind else {
        panic!("expected when")
    };
    assert!(matches!(
        when.arms[0].condition,
        scoop_ast::WhenArmCondition::Case(_)
    ));
    assert_eq!(when.arms.len(), 5);
}

#[test]
fn ordinary_arm_newlines_do_not_extend_the_previous_expression() {
    let file = ok("fun f() { when (x) {
 is A -> call()
 !is B -> 1
 in items -> 2
 !in items -> 3
 -1 -> (1
 + 2)
 +2 -> 3 +
 4
 else -> 5
} }");
    let StatementKind::When(when) = &block_body(only_function(&file)).statements[0].kind else {
        panic!("expected when")
    };
    assert_eq!(when.arms.len(), 6);
}

#[test]
fn elvis_jump_expressions_retain_their_operands() {
    let file = ok("fun f() { val a = value ?: return fallback ?: throw failure }");
    let StatementKind::ValDecl(decl) = &block_body(only_function(&file)).statements[0].kind else {
        panic!("declaration")
    };
    let scoop_ast::Expr::Elvis { rhs, .. } = &decl.init else {
        panic!("elvis")
    };
    let scoop_ast::Expr::Return {
        value: Some(value), ..
    } = rhs.as_ref()
    else {
        panic!("return")
    };
    let scoop_ast::Expr::Elvis { rhs, .. } = value.as_ref() else {
        panic!("returned elvis")
    };
    assert!(matches!(rhs.as_ref(), scoop_ast::Expr::Throw { .. }));
}

#[test]
fn bare_return_respects_nested_delimiters_and_newlines() {
    ok(
        "fun f() { call(value ?: return, next); val a = [return]; val b = (return); return\n call() }",
    );
}

#[test]
fn throw_and_return_are_expressions_in_copy_update_values() {
    ok(
        "fun f() { val a = record.{ value: optional ?: return }; val b = record.{ value: throw failure } }",
    );
}
