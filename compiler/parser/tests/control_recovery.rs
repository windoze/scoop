use scoop_parser::parse_recovering;
use scoop_syntax::{ExpectedSyntax, SyntaxKind};

#[test]
fn malformed_lambda_parameters_keep_typed_siblings_and_body() {
    let source = "fun f() = { bad: , good: Int -> good }";
    let parsed = parse_recovering(source).unwrap();
    assert!(parsed.ast().is_none());
    assert_eq!(parsed.diagnostics().len(), 1);
    let syntax = parsed.syntax();
    assert_eq!(syntax.text().to_string(), source);
    assert!(
        syntax
            .descendants()
            .any(|node| node.kind() == SyntaxKind::Parameter && node.text() == "good: Int")
    );
    assert!(
        syntax
            .descendants()
            .any(|node| node.kind() == SyntaxKind::Statement && node.text() == "good")
    );
}

#[test]
fn lambda_probe_requires_its_real_arrow_and_rolls_back_otherwise() {
    for source in [
        "fun f() = { value }",
        "fun f() = { call(1)\ncall(2) }",
        "fun f() = { val x = 1\n x }",
        "fun f() = { return 1 }",
        "fun f() = { if (ready) call(1) else call(2) }",
        "fun f() = { mapper: (Int) -> Int, x: Int -> mapper(x) }",
        "fun f() = { (x, y) -> x }",
        "fun f() = { -> 1 }",
    ] {
        let parsed = parse_recovering(source).unwrap();
        assert!(
            parsed.diagnostics().is_empty(),
            "{source}: {:?}",
            parsed.diagnostics()
        );
        assert!(
            parsed.missing().is_empty(),
            "{source}: {:?}",
            parsed.missing()
        );
        assert_eq!(parsed.syntax().text().to_string(), source);
    }
}

#[test]
fn when_recovers_between_arms_without_leaking_them_out_of_the_block() {
    let source = "fun f(value: Int) { when (value) {\ncase -> 1\n2 if -> 2\n3 -> 3\nelse -> 0\n}\nval kept = 4\n}\nfun after() = 5";
    let parsed = parse_recovering(source).unwrap();
    assert!(parsed.ast().is_none());
    assert_eq!(parsed.diagnostics().len(), 2);
    let syntax = parsed.syntax();
    assert_eq!(syntax.text().to_string(), source);
    let when = syntax
        .descendants()
        .find(|node| node.kind() == SyntaxKind::WhenExpr)
        .unwrap();
    assert_eq!(
        when.descendants()
            .filter(|node| node.kind() == SyntaxKind::WhenArm)
            .count(),
        4
    );
    assert!(
        when.descendants()
            .any(|node| node.kind() == SyntaxKind::WhenArm && node.text() == "3 -> 3")
    );
    assert_eq!(
        syntax
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::FunctionDecl)
            .count(),
        2
    );
}

#[test]
fn missing_selectors_and_operands_keep_their_receivers_and_later_statements() {
    for source in [
        "fun f(user: User) { user.\nval kept = 1\n}",
        "fun f(user: User) { user?.\nval kept = 1\n}",
        "fun f() { val x = 1 +\nval kept = 1\n}",
    ] {
        let parsed = parse_recovering(source).unwrap();
        assert!(parsed.ast().is_none());
        assert_eq!(parsed.diagnostics().len(), 1);
        let expected = if source.contains("user") {
            ExpectedSyntax::Name
        } else {
            ExpectedSyntax::Expression
        };
        assert!(
            parsed
                .missing()
                .iter()
                .any(|missing| missing.expected == expected)
        );
        let syntax = parsed.syntax();
        assert_eq!(syntax.text().to_string(), source);
        assert!(
            syntax
                .descendants()
                .any(|node| node.kind() == SyntaxKind::Statement && node.text() == "val kept = 1")
        );
    }
}

#[test]
fn when_else_order_and_guard_rules_remain_errors() {
    for source in [
        "fun f(x: Int) = when(x) { else -> 0\n1 -> 1 }",
        "fun f(x: Int) = when(x) { case 1, 2 -> 1 }",
        "fun f() = when { true if ready -> 1 }",
    ] {
        let parsed = parse_recovering(source).unwrap();
        assert!(parsed.ast().is_none());
        assert_eq!(parsed.syntax().text().to_string(), source);
    }
}
