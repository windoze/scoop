use scoop_parser::parse_recovering;
use scoop_syntax::SyntaxKind;

#[test]
fn pattern_lists_keep_bindings_after_a_bad_item() {
    for source in [
        "fun f(x: Any) = when(x) { case Pair(, good) -> good\nelse -> 0 }",
        "fun f(x: Any) = when(x) { case User {bad: , good} -> good\nelse -> 0 }",
        "fun f() { val (first, , good) = value\nreturn good }",
    ] {
        let parsed = parse_recovering(source).unwrap();
        assert!(parsed.ast().is_none(), "{source}");
        let syntax = parsed.syntax();
        assert_eq!(syntax.text().to_string(), source);
        let list = syntax
            .descendants()
            .find(|node| node.kind() == SyntaxKind::PatternList)
            .unwrap();
        assert!(
            list.descendants().any(|node| matches!(
                node.kind(),
                SyntaxKind::Pattern | SyntaxKind::Name
            ) && node.text() == "good"),
            "{source}: {list:#?}"
        );
    }
}

#[test]
fn rest_markers_and_parenthesized_patterns_keep_their_meaning() {
    for pattern in [
        "()",
        "(value)",
        "(value,)",
        "(..)",
        "(first, .., last)",
        "Pair(.., x)",
        "User {Unit, field, ..}",
    ] {
        let source = format!("fun f(x: Any) = when(x) {{ case {pattern} -> 1\nelse -> 0 }}");
        let parsed = parse_recovering(&source).unwrap();
        assert!(
            parsed.diagnostics().is_empty(),
            "{pattern}: {:?}",
            parsed.diagnostics()
        );
        assert!(parsed.ast().is_some());
    }
    for pattern in ["(.., ..)", "Pair(.., ..)", "User {.., field}", "User {_}"] {
        let source = format!("fun f(x: Any) = when(x) {{ case {pattern} -> 1\nelse -> 0 }}");
        let parsed = parse_recovering(&source).unwrap();
        assert!(parsed.ast().is_none(), "{pattern}");
        assert_eq!(parsed.syntax().text().to_string(), source);
    }
}

#[test]
fn incomplete_nested_patterns_leave_following_arms_in_the_when() {
    let source = "fun f(x: Any) = when(x) {\ncase Pair(first, -> 1\ncase User {bad: , good} -> 2\nelse -> 0\n}";
    let parsed = parse_recovering(source).unwrap();
    assert!(parsed.ast().is_none());
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
        3
    );
}
