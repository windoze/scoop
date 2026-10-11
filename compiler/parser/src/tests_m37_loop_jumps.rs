use scoop_ast::{Expr, LoopJumpKind, StatementKind};

use crate::tests::{block_body, err, ok, only_function};
use crate::tests_m2::stmt_dump;

#[test]
fn labels_keep_actual_names_and_source_spans() {
    let source = "fun main() { `外.层`@ while (true) break@`外.层` }";
    let file = ok(source);
    let statement = &block_body(only_function(&file)).statements[0];
    let StatementKind::While(loop_) = &statement.kind else {
        panic!("while")
    };
    let label = loop_.label.as_ref().unwrap();
    assert_eq!(label.text, "外.层");
    assert_eq!(
        &source[label.span.start as usize..label.span.end as usize],
        "`外.层`"
    );
    assert_eq!(statement.span.start, label.span.start);
    let StatementKind::Break {
        label: Some(target),
    } = &loop_.body.statements[0].kind
    else {
        panic!("labelled break")
    };
    assert_eq!(target.text, label.text);
}

#[test]
fn labelled_for_and_jump_dumps_preserve_targets() {
    assert_eq!(
        stmt_dump("outer@ for (x in items) continue@outer"),
        "for@outer x\n  Var items\n  continue@outer\n"
    );
}

#[test]
fn loop_jumps_parse_in_expression_positions() {
    let file = ok("fun main() { val a = other ?: break@outer; call(continue); val b = (break) }");
    let statements = &block_body(only_function(&file)).statements;
    let StatementKind::ValDecl(decl) = &statements[0].kind else {
        panic!("val")
    };
    let Expr::Elvis { rhs, .. } = &decl.init else {
        panic!("elvis")
    };
    assert!(
        matches!(rhs.as_ref(), Expr::LoopJump { kind: LoopJumpKind::Break, label: Some(label), .. } if label.text == "outer")
    );
    let StatementKind::Expr(Expr::Call(call)) = &statements[1].kind else {
        panic!("call")
    };
    assert!(matches!(
        call.args[0].expression,
        Expr::LoopJump {
            kind: LoopJumpKind::Continue,
            label: None,
            ..
        }
    ));
}

#[test]
fn jumps_do_not_take_values_or_cross_a_newline_to_find_a_label() {
    for source in [
        "fun f() { break 1 }",
        "fun f() { val x = left break right }",
        "fun f() { continue@\nouter }",
    ] {
        err(source);
    }
    let file = ok("fun f() { while (true) { break\nouter@ while (false) {} } }");
    let StatementKind::While(loop_) = &block_body(only_function(&file)).statements[0].kind else {
        panic!("while")
    };
    assert!(matches!(
        loop_.body.statements[0].kind,
        StatementKind::Break { label: None }
    ));
}
