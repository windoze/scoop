use super::*;

// --- M2 "not supported" diagnostics ----------------------------------------

#[test]
fn if_and_when_parse_in_expression_position() {
    let file = ok(
        "fun main() {\n    val x = if (true) { 1 } else { 2 }\n    val y = when (x) { else -> 3 }\n}",
    );
    let body = block_body(only_function(&file));
    let StatementKind::ValDecl(x) = &body.statements[0].kind else {
        panic!("expected x declaration");
    };
    assert!(matches!(x.init, Expr::If(_)));
    let StatementKind::ValDecl(y) = &body.statements[1].kind else {
        panic!("expected y declaration");
    };
    assert!(matches!(y.init, Expr::When(_)));
}

#[test]
fn for_loop_not_supported() {
    let (span, message) = err("fun main() { for (i in xs) {} }");
    assert_eq!(span, Span::new(13, 16));
    assert_eq!(message, "`for` loops are not supported yet (milestone M5)");
}

#[test]
fn string_interpolation_not_supported() {
    let (span, message) = err("fun main() {\n    val s = f\"x {y}\"\n}\n");
    assert_eq!(span, Span::new(25, 26));
    assert_eq!(
        message,
        "string interpolation is not supported yet (milestone M3)"
    );
}

#[test]
fn tuple_element_assignment_not_supported() {
    // Tuple elements are value-type fields; named field assignment
    // (`a.b = 1`) parses since M6 (HIR rejects value-type receivers).
    let (span, message) = err("fun main() {\n    a._1 = 1\n}\n");
    assert_eq!(span, Span::new(22, 23));
    assert_eq!(
        message,
        "field assignment is not supported (value types are immutable)"
    );
}

#[test]
fn var_struct_field_not_supported() {
    let (span, message) = err("struct S(var x: Int)");
    assert_eq!(span, Span::new(9, 12));
    assert_eq!(
        message,
        "`var` struct fields are not supported (value types are immutable)"
    );
}

#[test]
fn field_default_value_not_supported() {
    let (span, message) = err("struct S(val x: Int = 0)");
    assert_eq!(span, Span::new(20, 21));
    assert_eq!(
        message,
        "field default values are not supported yet (milestone M3)"
    );
}

#[test]
fn struct_empty_member_body() {
    // M2–M5 rejected a body after the constructor; M6 adds member
    // functions, so an empty body parses.
    let file = ok("struct S(val x: Int) {}");
    let Decl::Struct(decl) = &file.declarations[0] else {
        panic!("expected a struct declaration");
    };
    assert!(decl.methods.is_empty());
    assert_eq!(decl.span, Span::new(0, 23));
}

#[test]
fn explicit_fieldless_struct_is_preserved() {
    let file = ok("struct S()");
    let Decl::Struct(struct_) = &file.declarations[0] else {
        panic!("expected struct");
    };
    assert!(matches!(
        struct_.fields,
        scoop_ast::StructRepresentationDecl::Declared(ref fields) if fields.is_empty()
    ));
}

// --- generic M2 syntax errors ----------------------------------------------

#[test]
fn if_condition_must_be_parenthesized() {
    let (_, message) = err("fun main() { if x {} }");
    assert_eq!(message, "expected `(`, found `x`");
}

#[test]
fn while_condition_must_be_parenthesized() {
    let (_, message) = err("fun main() { while x {} }");
    assert_eq!(message, "expected `(`, found `x`");
}

#[test]
fn else_if_gets_no_special_treatment() {
    // M2 design section 6: `else` must be followed by a block.
    let (_, message) = err("fun main() { if (a) {} else if (b) {} }");
    assert_eq!(message, "expected `{`, found `if`");
}

#[test]
fn missing_selector_after_dot() {
    let (_, message) = err("fun main() { a. }");
    assert_eq!(message, "expected field name or tuple index, found `}`");
}

#[test]
fn integer_literal_out_of_range() {
    let (span, message) = err("fun main() {\n    val x = 99999999999999999999\n}\n");
    assert_eq!(span, Span::new(25, 45));
    assert_eq!(
        message,
        "integer literal `99999999999999999999` is out of range (Int is i64)"
    );
}

#[test]
fn statement_after_if_on_one_line_needs_a_separator() {
    let (span, message) = err("fun main() { if (a) {} val x = 1 }");
    assert_eq!(span, Span::new(23, 26));
    assert_eq!(
        message,
        "expected `;` or newline after statement, found `val`"
    );
}
