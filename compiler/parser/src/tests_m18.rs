//! M18 callable-surface parser coverage.

use scoop_ast::{
    AssignmentOp, CompoundAssignOp, Decl, Expr, Navigation, PlaceExpr, StatementKind,
    UpdateNotation, UpdateOp,
};

use crate::parse;
use crate::tests::{block_body, ok, only_function};
use crate::tests_m2::{init_expr, stmt_dump};

#[test]
fn callable_operator_precedence_is_fully_ordered() {
    assert_eq!(
        stmt_dump("val x = a || b && c == d < e in f ?: g link h..i + j * (-k as Int)"),
        "val x\n\
         \x20 Binary Or\n\
         \x20   Var a\n\
         \x20   Binary And\n\
         \x20     Var b\n\
         \x20     Binary Eq\n\
         \x20       Var c\n\
         \x20       Binary Lt\n\
         \x20         Var d\n\
         \x20         Binary Contains\n\
         \x20           Var e\n\
         \x20           Elvis\n\
         \x20             Var f\n\
         \x20             InfixCall link\n\
         \x20               Var g\n\
         \x20               Binary RangeTo\n\
         \x20                 Var h\n\
         \x20                 Binary Add\n\
         \x20                   Var i\n\
         \x20                   Binary Mul\n\
         \x20                     Var j\n\
         \x20                     Cast Int optional=false\n\
         \x20                       Unary Neg\n\
         \x20                         Var k\n"
    );
}

#[test]
fn all_new_operator_tokens_keep_their_closed_ast_roles() {
    let source = "fun main() {\n\
        +a;\n-a;\n!a;\n++a;\na--;\na % b\na..b\na..<b\na in b\na !in b\n\
        a += b\na -= b\na *= b\na /= b\na %= b\n}\n";
    let file = ok(source);
    let statements = &block_body(only_function(&file)).statements;
    assert_eq!(statements.len(), 15, "{}", scoop_ast::dump(&file));
    assert!(matches!(
        statements[0].kind,
        StatementKind::Expr(Expr::Unary {
            op: scoop_ast::UnOp::Plus,
            ..
        })
    ));
    assert!(matches!(
        statements[3].kind,
        StatementKind::Expr(Expr::Update {
            op: UpdateOp::Increment,
            notation: UpdateNotation::Prefix,
            ..
        })
    ));
    assert!(matches!(
        statements[4].kind,
        StatementKind::Expr(Expr::Update {
            op: UpdateOp::Decrement,
            notation: UpdateNotation::Postfix,
            ..
        })
    ));
    assert!(matches!(
        statements[5].kind,
        StatementKind::Expr(Expr::Binary {
            op: scoop_ast::BinOp::Rem,
            ..
        })
    ));
    assert!(matches!(
        statements[6].kind,
        StatementKind::Expr(Expr::Binary {
            op: scoop_ast::BinOp::RangeTo,
            ..
        })
    ));
    assert!(matches!(
        statements[7].kind,
        StatementKind::Expr(Expr::Binary {
            op: scoop_ast::BinOp::RangeUntil,
            ..
        })
    ));
    assert!(matches!(
        statements[8].kind,
        StatementKind::Expr(Expr::Binary {
            op: scoop_ast::BinOp::Contains,
            ..
        })
    ));
    assert!(matches!(
        statements[9].kind,
        StatementKind::Expr(Expr::Binary {
            op: scoop_ast::BinOp::NotContains,
            ..
        })
    ));
    let expected = [
        CompoundAssignOp::Add,
        CompoundAssignOp::Sub,
        CompoundAssignOp::Mul,
        CompoundAssignOp::Div,
        CompoundAssignOp::Rem,
    ];
    for (statement, expected) in statements[10..].iter().zip(expected) {
        let StatementKind::Assign(assign) = &statement.kind else {
            panic!("expected a compound assignment");
        };
        assert_eq!(assign.op, AssignmentOp::Compound(expected));
        assert!(matches!(assign.target, PlaceExpr::Name(_)));
    }
}

#[test]
fn modifiers_safe_methods_invocation_and_multi_index_are_preserved() {
    let file = ok("infix fun Box.union(other: Box): Box = this\n\
         struct Handler {\n\
             operator infix fun invoke(message: String): String = message\n\
         }\n\
         fun main() {\n\
             maybe?.send<String>(request)\n\
             (handler)<String>(message)\n\
             matrix[i, j]\n\
             matrix[i, j] = value\n\
         }\n");
    let Decl::Function(extension) = &file.declarations[0] else {
        panic!("expected extension function");
    };
    assert!(extension.infix.is_some());
    assert!(extension.receiver_ty.is_some());
    let Decl::Struct(handler) = &file.declarations[1] else {
        panic!("expected handler struct");
    };
    let invoke = handler.functions().next().expect("handler method");
    assert!(invoke.operator.is_some());
    assert!(invoke.infix.is_some());
    let Decl::Function(main) = &file.declarations[2] else {
        panic!("expected main function");
    };
    let statements = &block_body(main).statements;
    assert!(matches!(
        statements[0].kind,
        StatementKind::Expr(Expr::MethodCall {
            navigation: Navigation::Safe,
            ..
        })
    ));
    assert!(matches!(
        statements[1].kind,
        StatementKind::Expr(Expr::Invoke { ref type_args, .. }) if type_args.len() == 1
    ));
    assert!(matches!(
        statements[2].kind,
        StatementKind::Expr(Expr::Index { ref indices, .. }) if indices.len() == 2
    ));
    assert!(matches!(
        statements[3].kind,
        StatementKind::Assign(scoop_ast::Assign {
            target: PlaceExpr::Index { ref indices, .. },
            ..
        }) if indices.len() == 2
    ));
}

#[test]
fn infix_is_left_associative_and_stops_at_newline() {
    assert_eq!(
        stmt_dump("val x = a merge b merge c"),
        "val x\n  InfixCall merge\n    InfixCall merge\n      Var a\n      Var b\n    Var c\n"
    );
    let file = ok("fun main() {\n    a\n    b\n}\n");
    assert_eq!(block_body(only_function(&file)).statements.len(), 2);
    assert_eq!(
        stmt_dump("val result = command \"build\""),
        "val result\n  InfixCall <invoke>\n    Var command\n    StringLiteral \"build\"\n"
    );
}

#[test]
fn malformed_m18_forms_recover_to_later_statements() {
    let diagnostics = parse(
        "fun main() {\n\
         \x20   ++call()\n\
         \x20   matrix[i,] = value\n\
         \x20   left merge\n\
         \x20   val after = 1\n\
         }\n",
    )
    .expect_err("the malformed forms must be diagnosed");
    let messages = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        messages,
        [
            "update operand must be an assignable place",
            "expected index expression after `,`, found `]`",
            "expected expression, found `val`",
        ]
    );
}

#[test]
fn update_rejects_non_place_without_leaking_an_ast() {
    let diagnostics = parse("fun main() { (a + b)++ }").expect_err("update must reject a value");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].message,
        "update operand must be an assignable place"
    );
    let Expr::InfixCall { .. } = init_expr("left merge right") else {
        panic!("valid infix input still produces a complete AST");
    };
}
