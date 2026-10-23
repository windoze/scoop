use scoop_parser::{parse_recovering, parse_type_recovering};
use scoop_syntax::{ExpectedSyntax, SyntaxKind};

#[test]
fn parameter_errors_keep_following_parameters_and_body() {
    let source =
        "fun f(first: , second: Int, broken Int, last: String) { return second }\nfun after() = 1";
    let parsed = parse_recovering(source).unwrap();
    assert!(parsed.ast().is_none());
    assert_eq!(parsed.diagnostics().len(), 2);
    let syntax = parsed.syntax();
    assert_eq!(syntax.text().to_string(), source);
    assert_eq!(
        syntax
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::Parameter)
            .count(),
        4
    );
    assert_eq!(
        syntax
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::FunctionDecl)
            .count(),
        2
    );
    assert!(
        syntax
            .descendants()
            .any(|node| node.kind() == SyntaxKind::Block
                && node.text().to_string().contains("return second"))
    );
}

#[test]
fn arguments_keep_named_siblings_after_multiple_missing_expressions() {
    let source = "fun f() { consume(first = , good = 2, broken = , last = 4)\n return 0 }";
    let parsed = parse_recovering(source).unwrap();
    assert!(parsed.ast().is_none());
    assert_eq!(parsed.diagnostics().len(), 2);
    let syntax = parsed.syntax();
    assert_eq!(syntax.text().to_string(), source);
    let arguments: Vec<_> = syntax
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::Argument)
        .collect();
    assert_eq!(arguments.len(), 4);
    assert_eq!(arguments[3].text().to_string(), "last = 4");
    assert!(
        parsed
            .missing()
            .iter()
            .all(|item| item.expected == ExpectedSyntax::Expression)
    );
}

#[test]
fn type_lists_keep_siblings_after_missing_bounds_or_arguments() {
    let source = "fun <T:, U, V:> f(x: Pair<, Int, , String>): Unit {}";
    let parsed = parse_recovering(source).unwrap();
    assert!(parsed.ast().is_none());
    assert_eq!(parsed.diagnostics().len(), 4);
    let syntax = parsed.syntax();
    assert_eq!(syntax.text().to_string(), source);
    assert_eq!(
        syntax
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::TypeParameter)
            .count(),
        3
    );
    assert!(
        syntax
            .descendants()
            .any(|node| node.kind() == SyntaxKind::Type && node.text() == "String")
    );
}

#[test]
fn inserts_only_missing_separators_at_the_current_list_boundary() {
    let source = "fun f(x: Int y: String { return x }";
    let parsed = parse_recovering(source).unwrap();
    assert!(parsed.ast().is_none());
    assert_eq!(parsed.diagnostics().len(), 2);
    assert_eq!(parsed.missing().len(), 2);
    assert_eq!(
        parsed.missing()[0].expected,
        ExpectedSyntax::Token(SyntaxKind::Comma)
    );
    assert_eq!(
        parsed.missing()[0].offset,
        source.find("y:").unwrap() as u32
    );
    assert_eq!(
        parsed.missing()[1].expected,
        ExpectedSyntax::Token(SyntaxKind::RParen)
    );
    assert_eq!(parsed.missing()[1].offset, source.find('{').unwrap() as u32);
    assert!(
        parsed
            .syntax()
            .descendants()
            .any(|node| node.kind() == SyntaxKind::Block)
    );
    assert_eq!(parsed.syntax().text().to_string(), source);
    let ty = parse_type_recovering("Pair<Int String>").unwrap();
    assert!(ty.ast().is_none());
    assert_eq!(
        ty.missing()[0].expected,
        ExpectedSyntax::Token(SyntaxKind::Comma)
    );
}

#[test]
fn recovery_does_not_relax_empty_items_trailing_commas_or_variance() {
    for source in [
        "fun f(x: Int,) {}",
        "fun f() { call(1,) }",
        "fun <> f() {}",
        "fun <T,> f() {}",
        "fun <out T> f() {}",
        "fun f(x: Pair<Int,>) {}",
        "fun f(x: List<*>) {}",
        "fun f(x: List<out Int>) {}",
        "fun f(x: List<in Int>) {}",
    ] {
        let parsed = parse_recovering(source).unwrap();
        assert!(parsed.ast().is_none(), "{source}");
        assert!(!parsed.diagnostics().is_empty(), "{source}");
        assert_eq!(parsed.syntax().text().to_string(), source);
    }
}

#[test]
fn recovered_type_probes_do_not_change_valid_comparisons() {
    for source in [
        "fun f() { consume(a < b, c > d) }",
        "fun f(p: Bool = a < b, next: Int = 1) {}",
        "fun f() { val x = a < b && c > d }",
        "fun f() { consume(f<Int>(1), g<Map<Int, String>>(2)) }",
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
fn malformed_nested_lists_and_eof_always_finish() {
    for source in [
        "fun f(",
        "fun f(x:",
        "fun f() { call(",
        "fun <T:",
        "fun f() { call(bad = (1 + ), good = 2) }",
        "fun f(x: List<Bad<(, Int>, y: String) {}",
        "fun f() { call(bad = [1, +], good = 2) }",
    ] {
        let parsed = parse_recovering(source).unwrap();
        assert!(parsed.ast().is_none(), "{source}");
        assert_eq!(parsed.syntax().text().to_string(), source);
    }
}
