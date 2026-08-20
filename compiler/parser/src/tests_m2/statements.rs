use super::*;

// --- statements -----------------------------------------------------------

#[test]
fn val_decl_with_type_annotation() {
    let file = ok("fun main() {\n    val x: Int = 42\n}\n");
    let function = only_function(&file);
    let stmt = &block_body(function).statements[0];
    assert_eq!(stmt.span, Span::new(17, 32));
    let StatementKind::ValDecl(decl) = &stmt.kind else {
        panic!("expected a val declaration");
    };
    assert!(!decl.mutable);
    let Pattern::Binding(name) = &decl.target else {
        panic!("expected a binding pattern");
    };
    assert_eq!(name.text, "x");
    assert_eq!(name.span, Span::new(21, 22));
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
    let StatementKind::ValDecl(decl) = &block_body(function).statements[0].kind else {
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
    let stmt = &block_body(function).statements[0];
    assert_eq!(stmt.span, Span::new(17, 26));
    let StatementKind::Assign(assign) = &stmt.kind else {
        panic!("expected an assignment");
    };
    let AssignTarget::Name(target) = &assign.target else {
        panic!("expected a local assignment target");
    };
    assert_eq!(target.text, "n");
    assert_eq!(target.span, Span::new(17, 18));
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
    let StatementKind::Assign(_) = &block_body(only_function(&assign)).statements[0].kind else {
        panic!("expected an assignment");
    };
    let equality = ok("fun main() { a == 1 }");
    let StatementKind::Expr(Expr::Binary { op: BinOp::Eq, .. }) =
        &block_body(only_function(&equality)).statements[0].kind
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
    let StatementKind::If(if_) = &block_body(only_function(&file)).statements[0].kind else {
        panic!("expected an if statement");
    };
    assert!(if_.else_block.is_none());
}

#[test]
fn else_may_start_on_the_next_line() {
    let file = ok("fun main() { if (a) {\n} else {\n} }");
    let StatementKind::If(if_) = &block_body(only_function(&file)).statements[0].kind else {
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
