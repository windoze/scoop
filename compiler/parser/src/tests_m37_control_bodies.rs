use scoop_ast::{Decl, Expr, FunctionBody, StatementKind};

use crate::tests::ok;

#[test]
fn dangling_else_binds_to_the_inner_if() {
    let file = ok("fun main() { if (a) if (b) left() else right() }");
    let Decl::Function(function) = &file.declarations[0] else {
        panic!("expected function");
    };
    let FunctionBody::Block(body) = &function.body else {
        panic!("expected block");
    };
    let StatementKind::If(outer) = &body.statements[0].kind else {
        panic!("expected outer if");
    };
    let StatementKind::If(inner) = &outer.then_block.statements[0].kind else {
        panic!("expected inner if");
    };
    assert!(outer.else_block.is_none());
    assert!(inner.else_block.is_some());
}

#[test]
fn expression_if_branches_keep_values_and_jump_boundaries() {
    let file = ok("fun main() { val v = if (a) 1 else if (b) 2 else 3 }");
    let Decl::Function(function) = &file.declarations[0] else {
        panic!("expected function");
    };
    let FunctionBody::Block(body) = &function.body else {
        panic!("expected block");
    };
    let StatementKind::ValDecl(binding) = &body.statements[0].kind else {
        panic!("expected local binding");
    };
    let Expr::If(if_) = &binding.init else {
        panic!("expected value if");
    };
    assert!(matches!(
        if_.then_block.statements[0].kind,
        StatementKind::Expr(_)
    ));
    assert!(matches!(
        if_.else_block.as_ref().unwrap().statements[0].kind,
        StatementKind::If(_)
    ));
    ok("fun main() { if (a) return else print(1) }");
    ok("fun main() { if (a) print(1); else print(2) }");
}

#[test]
fn loops_and_nested_calls_keep_their_own_bodies() {
    ok("fun main() {\n\
           while (hasNext()) consume { 42 }\n\
           for (value in items()) if (ready(value)) consume(value) else continue\n\
           if (check { true }) consume { 42 } else consume { 0 }\n\
           if (ready) val temporary = 42\n\
         }");
}
