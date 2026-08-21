//! Unit tests for the M2 syntax added on top of M1: struct declarations,
//! `val` / `var`, assignment, `if` / `while` / nested blocks, the full
//! expression grammar (precedence, postfix access, unit/tuple/paren
//! disambiguation), and every M2 "not supported" diagnostic.

use scoop_ast::{BinOp, Decl, Expr, FieldSelector, Span, StatementKind, TypeRefKind, UnOp};

use crate::tests::{err, ok, only_function};

/// Parses `expr` as the initializer of `val x = <expr>` and returns it.
fn init_expr(expr: &str) -> Expr {
    let file = ok(&format!("fun main() {{\n    val x = {expr}\n}}\n"));
    let function = only_function(&file);
    let StatementKind::ValDecl(decl) = &function.body.statements[0].kind else {
        panic!("expected a val declaration");
    };
    decl.init.clone()
}

/// Dumps `fun main() { <statement> }`, stripping the wrapper and the
/// function body's base indentation.
fn stmt_dump(statement: &str) -> String {
    let file = ok(&format!("fun main() {{\n    {statement}\n}}\n"));
    let dump = scoop_ast::dump(&file);
    let body = dump
        .strip_prefix("SourceFile\n  fun main\n")
        .expect("dump starts with the function header");
    let mut out = String::new();
    for line in body.lines() {
        out.push_str(line.strip_prefix("    ").unwrap_or(line));
        out.push('\n');
    }
    out
}

// --- declarations ---------------------------------------------------------

#[test]
fn struct_decl() {
    let file = ok("struct Point(val x: Int, val y: Int)");
    let Decl::Struct(decl) = &file.declarations[0] else {
        panic!("expected a struct declaration");
    };
    assert_eq!(decl.name.text, "Point");
    assert_eq!(decl.name.span, Span::new(7, 12));
    assert_eq!(decl.span, Span::new(0, 36));
    assert_eq!(decl.fields.len(), 2);
    assert_eq!(decl.fields[0].name.text, "x");
    assert_eq!(decl.fields[0].span, Span::new(13, 23));
    assert_eq!(decl.fields[1].name.text, "y");
    assert_eq!(
        scoop_ast::dump(&file),
        "SourceFile\n  struct Point\n    field x: Int\n    field y: Int\n"
    );
}

#[test]
fn struct_field_type_forms() {
    let file =
        ok("struct S(val a: (Int, String), val b: (Int,), val c: (), val d: Unit, val e: (Int))");
    let Decl::Struct(decl) = &file.declarations[0] else {
        panic!("expected a struct declaration");
    };
    let kinds: Vec<&TypeRefKind> = decl.fields.iter().map(|f| &f.ty.kind).collect();
    assert!(
        matches!(kinds[0], TypeRefKind::Tuple(elements) if elements.len() == 2),
        "tuple type"
    );
    assert!(
        matches!(kinds[1], TypeRefKind::Tuple(elements) if elements.len() == 1),
        "1-tuple type needs a trailing comma"
    );
    assert!(
        matches!(kinds[2], TypeRefKind::Unit),
        "`()` is the Unit type"
    );
    assert!(
        matches!(kinds[3], TypeRefKind::Unit),
        "`Unit` is the Unit type"
    );
    assert!(
        matches!(kinds[4], TypeRefKind::Named(name) if name.text == "Int"),
        "`(T)` is just `T` in parentheses"
    );
}

#[test]
fn mixed_declarations() {
    let file = ok("struct P(val x: Int)\n\nfun main() {\n}\n");
    assert_eq!(file.declarations.len(), 2);
    assert!(matches!(file.declarations[0], Decl::Struct(_)));
    assert!(matches!(file.declarations[1], Decl::Function(_)));
}

// --- statements -----------------------------------------------------------

#[test]
fn val_decl_with_type_annotation() {
    let file = ok("fun main() {\n    val x: Int = 42\n}\n");
    let function = only_function(&file);
    let stmt = &function.body.statements[0];
    assert_eq!(stmt.span, Span::new(17, 32));
    let StatementKind::ValDecl(decl) = &stmt.kind else {
        panic!("expected a val declaration");
    };
    assert!(!decl.mutable);
    assert_eq!(decl.name.text, "x");
    assert_eq!(decl.name.span, Span::new(21, 22));
    let ty = decl.ty.as_ref().expect("type annotation present");
    assert!(matches!(&ty.kind, TypeRefKind::Named(name) if name.text == "Int"));
    assert_eq!(decl.span, Span::new(17, 32));
    assert_eq!(
        stmt_dump("val x: Int = 42"),
        "val x: Int\n  IntLiteral 42\n"
    );
}

#[test]
fn var_decl_without_type_annotation() {
    let file = ok("fun main() {\n    var n = 0\n}\n");
    let function = only_function(&file);
    let StatementKind::ValDecl(decl) = &function.body.statements[0].kind else {
        panic!("expected a var declaration");
    };
    assert!(decl.mutable);
    assert!(decl.ty.is_none());
    assert_eq!(stmt_dump("var n = 0"), "var n\n  IntLiteral 0\n");
}

#[test]
fn assign_statement() {
    let file = ok("fun main() {\n    n = n + 1\n}\n");
    let function = only_function(&file);
    let stmt = &function.body.statements[0];
    assert_eq!(stmt.span, Span::new(17, 26));
    let StatementKind::Assign(assign) = &stmt.kind else {
        panic!("expected an assignment");
    };
    assert_eq!(assign.target.text, "n");
    assert_eq!(assign.target.span, Span::new(17, 18));
    assert_eq!(assign.span, Span::new(17, 26));
    assert_eq!(
        stmt_dump("n = n + 1"),
        "assign n\n  Binary Add\n    Var n\n    IntLiteral 1\n"
    );
}

#[test]
fn assign_vs_equality() {
    // `=` at statement start is assignment; `==` is an expression.
    let assign = ok("fun main() { a = 1 }");
    let StatementKind::Assign(_) = &only_function(&assign).body.statements[0].kind else {
        panic!("expected an assignment");
    };
    let equality = ok("fun main() { a == 1 }");
    let StatementKind::Expr(Expr::Binary { op: BinOp::Eq, .. }) =
        &only_function(&equality).body.statements[0].kind
    else {
        panic!("expected an equality expression");
    };
}

#[test]
fn if_else_statement() {
    let dump = stmt_dump("if (a) {\n        b()\n    } else {\n        c()\n    }");
    assert_eq!(dump, "if\n  Var a\n  Call b\nelse\n  Call c\n");
}

#[test]
fn if_without_else() {
    let file = ok("fun main() { if (a) {} }");
    let StatementKind::If(if_) = &only_function(&file).body.statements[0].kind else {
        panic!("expected an if statement");
    };
    assert!(if_.else_block.is_none());
}

#[test]
fn else_may_start_on_the_next_line() {
    let file = ok("fun main() { if (a) {\n} else {\n} }");
    let StatementKind::If(if_) = &only_function(&file).body.statements[0].kind else {
        panic!("expected an if statement");
    };
    assert!(if_.else_block.is_some());
}

#[test]
fn while_statement() {
    assert_eq!(
        stmt_dump("while (n < 3) {\n        n = n + 1\n    }"),
        "while\n  Binary Lt\n    Var n\n    IntLiteral 3\n  assign n\n    Binary Add\n      Var n\n      IntLiteral 1\n"
    );
}

#[test]
fn nested_block_statement() {
    assert_eq!(
        stmt_dump("{\n        val x = 1\n    }"),
        "block\n  val x\n    IntLiteral 1\n"
    );
}

// --- expressions ----------------------------------------------------------

#[test]
fn int_and_bool_literals() {
    assert!(
        matches!(init_expr("42"), Expr::IntLiteral { value: 42, span } if span == Span::new(25, 27))
    );
    assert!(matches!(
        init_expr("true"),
        Expr::BoolLiteral { value: true, .. }
    ));
    assert!(matches!(
        init_expr("false"),
        Expr::BoolLiteral { value: false, .. }
    ));
}

#[test]
fn multiplicative_binds_tighter_than_additive() {
    assert_eq!(
        stmt_dump("val x = 1 + 2 * 3"),
        "val x\n  Binary Add\n    IntLiteral 1\n    Binary Mul\n      IntLiteral 2\n      IntLiteral 3\n"
    );
}

#[test]
fn binary_operators_are_left_associative() {
    assert_eq!(
        stmt_dump("val x = 1 - 2 - 3"),
        "val x\n  Binary Sub\n    Binary Sub\n      IntLiteral 1\n      IntLiteral 2\n    IntLiteral 3\n"
    );
}

#[test]
fn and_binds_tighter_than_or() {
    assert_eq!(
        stmt_dump("val x = a || b && c"),
        "val x\n  Binary Or\n    Var a\n    Binary And\n      Var b\n      Var c\n"
    );
}

#[test]
fn comparison_binds_tighter_than_equality() {
    assert_eq!(
        stmt_dump("val x = 1 < 2 == true"),
        "val x\n  Binary Eq\n    Binary Lt\n      IntLiteral 1\n      IntLiteral 2\n    BoolLiteral true\n"
    );
}

#[test]
fn unary_binds_tighter_than_equality() {
    assert_eq!(
        stmt_dump("val x = !a == b"),
        "val x\n  Binary Eq\n    Unary Not\n      Var a\n    Var b\n"
    );
}

#[test]
fn postfix_binds_tighter_than_unary() {
    let expr = init_expr("-p.x");
    let Expr::Unary {
        op: UnOp::Neg,
        operand,
        span,
    } = &expr
    else {
        panic!("expected a unary expression");
    };
    assert!(matches!(**operand, Expr::FieldAccess(_)));
    assert_eq!(*span, Span::new(25, 29));
    assert_eq!(
        stmt_dump("val x = !!flag"),
        "val x\n  Unary Not\n    Unary Not\n      Var flag\n"
    );
}

#[test]
fn field_access_by_name_and_index() {
    assert_eq!(
        stmt_dump("val y = t._1._2"),
        "val y\n  FieldAccess _2\n    FieldAccess _1\n      Var t\n"
    );
    let expr = init_expr("p.x");
    let Expr::FieldAccess(access) = &expr else {
        panic!("expected a field access");
    };
    assert_eq!(access.span, Span::new(25, 28));
    let FieldSelector::Name(name) = &access.selector else {
        panic!("expected a named selector");
    };
    assert_eq!(name.text, "x");
    let expr = init_expr("q._12");
    let Expr::FieldAccess(access) = &expr else {
        panic!("expected a field access");
    };
    assert!(matches!(access.selector, FieldSelector::Index(12, _)));
}

#[test]
fn field_access_on_call_result() {
    // Struct construction is plain call syntax in M2 (parsed as `Call`).
    assert_eq!(
        stmt_dump("val x = Point(1, 2).x"),
        "val x\n  FieldAccess x\n    Call Point\n      IntLiteral 1\n      IntLiteral 2\n"
    );
}

#[test]
fn division_operator_coexists_with_comments() {
    assert_eq!(
        stmt_dump("val x = 6 / 2 // half"),
        "val x\n  Binary Div\n    IntLiteral 6\n    IntLiteral 2\n"
    );
    assert_eq!(
        stmt_dump("val x = a /* not a comment error */ / b"),
        "val x\n  Binary Div\n    Var a\n    Var b\n"
    );
}

// --- unit / tuple / paren disambiguation (spec 4.3) ------------------------

#[test]
fn empty_parens_are_the_unit_literal() {
    let Expr::UnitLiteral { span } = init_expr("()") else {
        panic!("expected a unit literal");
    };
    assert_eq!(span, Span::new(25, 27));
}

#[test]
fn unit_identifier_is_the_unit_literal() {
    let Expr::UnitLiteral { span } = init_expr("Unit") else {
        panic!("expected a unit literal");
    };
    assert_eq!(span, Span::new(25, 29));
}

#[test]
fn parens_around_one_expr_produce_no_node() {
    let Expr::Var(ident) = init_expr("(y)") else {
        panic!("expected the parenthesized expression itself");
    };
    assert_eq!(ident.text, "y");
    // Nested parens dissolve the same way.
    assert!(matches!(
        init_expr("((1))"),
        Expr::IntLiteral { value: 1, .. }
    ));
}

#[test]
fn trailing_comma_makes_a_one_tuple() {
    let Expr::TupleLiteral { elements, span } = init_expr("(y,)") else {
        panic!("expected a 1-tuple literal");
    };
    assert_eq!(elements.len(), 1);
    assert_eq!(span, Span::new(25, 29));
}

#[test]
fn comma_makes_a_tuple() {
    let Expr::TupleLiteral { elements, .. } = init_expr("(y, z)") else {
        panic!("expected a tuple literal");
    };
    assert_eq!(elements.len(), 2);
    // A trailing comma is allowed on multi-element tuples too.
    let Expr::TupleLiteral { elements, .. } = init_expr("(y, z,)") else {
        panic!("expected a tuple literal");
    };
    assert_eq!(elements.len(), 2);
}

// --- the M2 design example end to end --------------------------------------

#[test]
fn design_example() {
    let source = "struct Point(val x: Int, val y: Int)\n\
\n\
fun main() {\n\
\u{20}   val p = Point(1, 2)\n\
\u{20}   var q = (1, \"hello\")\n\
\u{20}   val x = p.x\n\
\u{20}   var n = 0\n\
\u{20}   while (n < 3) {\n\
\u{20}       n = n + 1\n\
\u{20}   }\n\
\u{20}   if (p == Point(1, 2) && x > 0) {\n\
\u{20}       println(\"ok\")\n\
\u{20}   } else {\n\
\u{20}       println(\"ng\")\n\
\u{20}   }\n\
\u{20}   val u = ()\n\
\u{20}   val s = (42,)\n\
}\n";
    let file = ok(source);
    assert_eq!(
        scoop_ast::dump(&file),
        "SourceFile\n  struct Point\n    field x: Int\n    field y: Int\n  fun main\n    val p\n      Call Point\n        IntLiteral 1\n        IntLiteral 2\n    var q\n      TupleLiteral\n        IntLiteral 1\n        StringLiteral \"hello\"\n    val x\n      FieldAccess x\n        Var p\n    var n\n      IntLiteral 0\n    while\n      Binary Lt\n        Var n\n        IntLiteral 3\n      assign n\n        Binary Add\n          Var n\n          IntLiteral 1\n    if\n      Binary And\n        Binary Eq\n          Var p\n          Call Point\n            IntLiteral 1\n            IntLiteral 2\n        Binary Gt\n          Var x\n          IntLiteral 0\n      Call println\n        StringLiteral \"ok\"\n    else\n      Call println\n        StringLiteral \"ng\"\n    val u\n      UnitLiteral\n    val s\n      TupleLiteral\n        IntLiteral 42\n"
    );
}

// --- M2 "not supported" diagnostics ----------------------------------------

#[test]
fn enum_not_supported() {
    let (span, message) = err("enum Color { RED }");
    assert_eq!(span, Span::new(0, 4));
    assert_eq!(message, "enums are not supported yet (milestone M2)");
}

#[test]
fn when_statement_not_supported() {
    let (span, message) = err("fun main() {\n    when (x) {\n    }\n}\n");
    assert_eq!(span, Span::new(17, 21));
    assert_eq!(
        message,
        "`when` expressions are not supported yet (milestone M2)"
    );
}

#[test]
fn when_expression_not_supported() {
    let (_, message) = err("fun main() { val x = when (y) {} }");
    assert_eq!(
        message,
        "`when` expressions are not supported yet (milestone M2)"
    );
}

#[test]
fn for_loop_not_supported() {
    let (span, message) = err("fun main() { for (i in xs) {} }");
    assert_eq!(span, Span::new(13, 16));
    assert_eq!(message, "`for` loops are not supported yet (milestone M2)");
}

#[test]
fn string_interpolation_not_supported() {
    let (span, message) = err("fun main() {\n    val s = f\"x {y}\"\n}\n");
    assert_eq!(span, Span::new(25, 26));
    assert_eq!(
        message,
        "string interpolation is not supported yet (milestone M2)"
    );
}

#[test]
fn field_assignment_not_supported() {
    let (span, message) = err("fun main() {\n    a.b = 1\n}\n");
    assert_eq!(span, Span::new(21, 22));
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
        "field default values are not supported yet (milestone M2)"
    );
}

#[test]
fn struct_member_body_not_supported() {
    let (span, message) = err("struct S(val x: Int) {}");
    assert_eq!(span, Span::new(21, 22));
    assert_eq!(
        message,
        "struct member declarations are not supported yet (milestone M2)"
    );
}

#[test]
fn fieldless_struct_not_supported() {
    let (span, message) = err("struct S()");
    assert_eq!(span, Span::new(9, 10));
    assert_eq!(message, "expected field declaration, found `)`");
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
