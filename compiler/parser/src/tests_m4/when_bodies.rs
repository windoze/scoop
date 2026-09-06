use super::*;

// --- single-expression arm bodies (spec 5.1) -----------------------------------

#[test]
fn when_arm_with_single_expression_body() {
    let when = when_with_arms("        Red -> println(\"red\")\n");
    assert_eq!(when.arms.len(), 1);
    let body = &when.arms[0].body;
    assert_eq!(body.statements.len(), 1);
    assert!(matches!(
        &body.statements[0].kind,
        StatementKind::Expr(Expr::Call(call)) if call.callee.text == "println"
    ));
    assert_eq!(body.span, body.statements[0].span);
}

#[test]
fn when_else_with_single_expression_body() {
    let when =
        when_with_arms("        Red -> println(\"r\")\n        else -> println(\"other\")\n");
    let else_body = when.else_body.as_ref().expect("else present");
    assert_eq!(else_body.statements.len(), 1);
    assert!(matches!(
        &else_body.statements[0].kind,
        StatementKind::Expr(Expr::Call(_))
    ));
}

#[test]
fn when_single_expression_arms_separate_like_statements() {
    let when = when_with_arms(
        "        Red -> println(\"r\")\n        Green -> println(\"g\")\n        Blue -> { println(\"b\") }\n",
    );
    assert_eq!(when.arms.len(), 3);
}

#[test]
fn when_guard_with_single_expression_body() {
    // The fixture shape from when-guards.scoop: guard, then a call body.
    let when = when_with_arms("        Num(n) if (n > 100) -> println(\"big\")\n");
    let arm = &when.arms[0];
    assert!(arm.guard.is_some());
    assert_eq!(arm.body.statements.len(), 1);
}

#[test]
fn when_single_expression_body_on_one_line_with_closing_brace() {
    let file = ok("fun main() { when (s) { Red -> println(\"r\") } }\n");
    let StatementKind::When(when) = &block_body(only_function(&file)).statements[0].kind else {
        panic!("expected a when statement");
    };
    assert_eq!(when.arms.len(), 1);
}

#[test]
fn when_single_expression_arm_body_needs_a_separator() {
    let (_, message) = err(
        "fun main() {\n    when (s) {\n        Red -> println(\"r\") Green -> println(\"g\")\n    }\n}\n",
    );
    assert_eq!(message, "expected expression, found `->`");
}

#[test]
fn when_arm_body_is_not_a_declaration() {
    // Only expression statements and M22 loop jumps are allowed without braces.
    let (_, message) = err("fun main() {\n    when (s) {\n        Red -> val x = 1\n    }\n}\n");
    assert_eq!(message, "expected expression, found `val`");
}
