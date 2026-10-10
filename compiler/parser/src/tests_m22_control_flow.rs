use scoop_ast::{BinOp, Expr, InfixTarget, Pattern, Span, StatementKind};

use crate::parse;
use crate::tests::{block_body, err, ok, only_function};
use crate::tests_m2::stmt_dump;

#[test]
fn for_preserves_pattern_iterable_body_and_spans() {
    let source = "fun main() {\n    for ((left, { value, .. }) in makeItems()) {\n        consume(left)\n    }\n}\n";
    let file = ok(source);
    let statement = &block_body(only_function(&file)).statements[0];
    let StatementKind::For(for_) = &statement.kind else {
        panic!("expected a for statement");
    };

    let start = source.find("for").unwrap() as u32;
    let end = source.find("\n    }\n}").unwrap() as u32 + "\n    }".len() as u32;
    assert_eq!(statement.span, Span::new(start, end));
    assert_eq!(for_.span, statement.span);
    assert!(matches!(
        &for_.pattern,
        Pattern::Tuple { elements, .. } if elements.len() == 2
    ));
    assert!(matches!(
        &for_.iterable,
        Expr::Call(call) if call.callee.text == "makeItems"
    ));
    assert_eq!(for_.body.statements.len(), 1);
    assert_eq!(for_.body.span.end, statement.span.end);
}

#[test]
fn m22_range_for_header_and_infix_chains_keep_ast_shape() {
    let file = ok("fun main() {\n\
         \x20   for (value in 1..<8 step 2) {}\n\
         \x20   val descending = 8 downTo 1 step 3\n\
         \x20   val member = 1 in 0 until 2\n\
         }\n");
    let statements = &block_body(only_function(&file)).statements;

    let StatementKind::For(for_) = &statements[0].kind else {
        panic!("expected a for statement");
    };
    let Expr::InfixCall {
        lhs: for_range,
        target: InfixTarget::Named(step),
        rhs: for_step,
        ..
    } = &for_.iterable
    else {
        panic!("for iterable must keep the outer step infix call");
    };
    assert_eq!(step.text, "step");
    assert!(matches!(
        for_range.as_ref(),
        Expr::Binary {
            op: BinOp::RangeUntil,
            ..
        }
    ));
    assert!(matches!(for_step.as_ref(), Expr::IntLiteral(_)));

    let StatementKind::ValDecl(descending) = &statements[1].kind else {
        panic!("expected descending range binding");
    };
    let Expr::InfixCall {
        lhs: descending_range,
        target: InfixTarget::Named(step),
        ..
    } = &descending.init
    else {
        panic!("descending range must keep the outer step call");
    };
    assert_eq!(step.text, "step");
    assert!(matches!(
        descending_range.as_ref(),
        Expr::InfixCall {
            target: InfixTarget::Named(target),
            ..
        } if target.text == "downTo"
    ));

    let StatementKind::ValDecl(member) = &statements[2].kind else {
        panic!("expected membership binding");
    };
    assert!(matches!(
        &member.init,
        Expr::Binary {
            op: BinOp::Contains,
            rhs,
            ..
        } if matches!(
            rhs.as_ref(),
            Expr::InfixCall {
                target: InfixTarget::Named(target),
                ..
            } if target.text == "until"
        )
    ));
}

#[test]
fn for_and_loop_jumps_have_stable_dump_shape() {
    assert_eq!(
        stmt_dump("for ((left, right) in items()) {\n        if (left) break else continue\n    }"),
        "for (left, right)\n  Call items\n  if\n    Var left\n    break\n  else\n    continue\n"
    );
}

#[test]
fn block_loop_jumps_keep_keyword_spans() {
    let source = "fun main() {\n    while (ready) {\n        break\n        continue\n    }\n}\n";
    let file = ok(source);
    let StatementKind::While(while_) = &block_body(only_function(&file)).statements[0].kind else {
        panic!("expected while");
    };
    let break_statement = &while_.body.statements[0];
    let continue_statement = &while_.body.statements[1];
    assert!(matches!(&break_statement.kind, StatementKind::Break));
    assert!(matches!(&continue_statement.kind, StatementKind::Continue));
    let break_start = source.find("break").unwrap() as u32;
    let continue_start = source.find("continue").unwrap() as u32;
    assert_eq!(
        break_statement.span,
        Span::new(break_start, break_start + "break".len() as u32)
    );
    assert_eq!(
        continue_statement.span,
        Span::new(continue_start, continue_start + "continue".len() as u32)
    );
}

#[test]
fn unbraced_if_branches_accept_only_loop_jumps() {
    let file = ok(
        "fun main() {\n    while (ready) {\n        if (done) break\n        else continue\n    }\n}\n",
    );
    let StatementKind::While(while_) = &block_body(only_function(&file)).statements[0].kind else {
        panic!("expected while");
    };
    let StatementKind::If(if_) = &while_.body.statements[0].kind else {
        panic!("expected if");
    };
    assert!(matches!(
        if_.then_block.statements.as_slice(),
        [statement] if matches!(&statement.kind, StatementKind::Break)
    ));
    assert!(matches!(
        if_.else_block.as_ref().unwrap().statements.as_slice(),
        [statement] if matches!(&statement.kind, StatementKind::Continue)
    ));

    let (_, message) = err("fun main() { if (ready) work() else rest() }");
    assert_eq!(message, "expected `{`, found `work`");
}

#[test]
fn value_if_may_use_a_direct_jump_as_its_terminating_branch() {
    let file = ok("fun main() { val value = if (done) break else { 1 } }");
    let StatementKind::ValDecl(value) = &block_body(only_function(&file)).statements[0].kind else {
        panic!("expected val");
    };
    let Expr::If(if_) = &value.init else {
        panic!("expected if expression");
    };
    assert!(matches!(
        if_.then_block.statements.as_slice(),
        [statement] if matches!(&statement.kind, StatementKind::Break)
    ));
}

#[test]
fn when_single_statement_bodies_accept_loop_jumps() {
    let file = ok(
        "fun main() {\n    while (ready) {\n        when (state) {\n            case Done -> break\n            else -> continue;\n        }\n    }\n}\n",
    );
    let StatementKind::While(while_) = &block_body(only_function(&file)).statements[0].kind else {
        panic!("expected while");
    };
    let StatementKind::When(when) = &while_.body.statements[0].kind else {
        panic!("expected when");
    };
    assert!(matches!(
        when.arms[0].body.statements.as_slice(),
        [statement] if matches!(&statement.kind, StatementKind::Break)
    ));
    assert!(matches!(
        when.else_body.as_ref().unwrap().statements.as_slice(),
        [statement] if matches!(&statement.kind, StatementKind::Continue)
    ));
}

#[test]
fn malformed_for_headers_report_the_exact_missing_delimiter() {
    let cases = [
        (
            "fun main() { for item in items) {} }",
            "item",
            "expected `(`, found `item`",
        ),
        (
            "fun main() { for (item items) {} }",
            "items",
            "expected `in`, found `items`",
        ),
        (
            "fun main() { for (item in items\n{} }",
            "{",
            "expected `)`, found `{`",
        ),
        (
            "fun main() { for (item in items) consume(item) }",
            "consume",
            "expected `{`, found `consume`",
        ),
    ];

    for (source, marker, expected) in cases {
        let (span, message) = err(source);
        let start = (if marker == "{" {
            source.rfind(marker).unwrap()
        } else {
            source.find(marker).unwrap()
        }) as u32;
        assert_eq!(
            span,
            Span::new(start, start + marker.len() as u32),
            "{source}"
        );
        assert_eq!(message, expected, "{source}");
    }
}

#[test]
fn malformed_for_recovery_reaches_later_statements_and_declarations() {
    let source = "fun main() {\n    for item in xs) {}\n    keep()\n    for (x xs) {}\n    keepAgain()\n}\nfun next(: Int) {}\n";
    let diagnostics = parse(source).expect_err("three independent errors must be collected");
    let messages = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        messages,
        [
            "expected `(`, found `item`",
            "expected `in`, found `xs`",
            "expected parameter name, found `:`",
        ]
    );
    assert!(diagnostics.windows(2).all(|pair| {
        pair[0].span.expect("spanned").start < pair[1].span.expect("spanned").start
    }));
}

#[test]
fn labeled_jumps_and_loops_remain_unsupported() {
    for (source, expected_span, expected) in [
        (
            "fun main() { break@outer }",
            "break@outer",
            "`break` labels are not supported in M22",
        ),
        (
            "fun main() { continue@outer }",
            "continue@outer",
            "`continue` labels are not supported in M22",
        ),
        (
            "fun main() { return@outer }",
            "return@outer",
            "`return` labels are not supported in M22",
        ),
        (
            "fun main() { outer@ while (ready) {} }",
            "outer@",
            "loop labels are not supported in M22",
        ),
        (
            "fun main() { outer@ for (item in items) {} }",
            "outer@",
            "loop labels are not supported in M22",
        ),
    ] {
        let (span, message) = err(source);
        let start = source.find(expected_span).unwrap() as u32;
        assert_eq!(
            span,
            Span::new(start, start + expected_span.len() as u32),
            "{source}"
        );
        assert_eq!(message, expected, "{source}");
    }
}

#[test]
fn general_jump_expressions_remain_unsupported() {
    for source in [
        "fun main() { val value = other ?: break }",
        "fun main() { call(continue) }",
        "fun main() { val value = (break) }",
        "fun main() { val value = left break right }",
        "fun main() { break + 1 }",
        "fun main() { continue.member }",
        "fun main() { break() }",
        "fun main() { if (ready) break + 1 }",
        "fun main() { when (state) { else -> continue.member } }",
    ] {
        let (span, message) = err(source);
        let (jump, expected) = if source.contains("continue") {
            (
                "continue",
                "`continue` jump expressions are not supported in M22",
            )
        } else {
            ("break", "`break` jump expressions are not supported in M22")
        };
        let start = source.find(jump).unwrap() as u32;
        assert_eq!(span, Span::new(start, start + jump.len() as u32));
        assert_eq!(message, expected);
    }
}

#[test]
fn do_while_remains_unsupported() {
    let source = "fun main() { do { work() } while (ready) }";
    let (span, message) = err(source);
    let start = source.find("do").unwrap() as u32;
    assert_eq!(span, Span::new(start, start + 2));
    assert_eq!(message, "`do-while` loops are not supported in M22");
}
