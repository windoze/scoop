use scoop_parser::parse_recovering;
use scoop_syntax::SyntaxKind;

#[test]
fn broken_fields_retain_the_following_field_and_member() {
    for source in [
        "struct S(val bad: , val good: Int) { fun method() = good }",
        "enum E { V(val bad: , val good: Int), Next\nfun method() = 1 }",
        "enum E { V(Int, , String), Next\nfun method() = 1 }",
        "enum E { V {bad: , good: Int}, Next\nfun method() = 1 }",
        "enum E { V {bad: Int = 1\ngood: Int}, Next\nfun method() = 1 }",
    ] {
        let parsed = parse_recovering(source).unwrap();
        assert!(parsed.ast().is_none(), "{source}");
        let syntax = parsed.syntax();
        assert_eq!(syntax.text().to_string(), source);
        assert!(
            syntax
                .descendants()
                .any(|node| node.kind() == SyntaxKind::FieldDecl
                    && (node.text().to_string().contains("good: Int") || node.text() == "String")),
            "{source}: {syntax:#?}"
        );
        assert!(
            syntax
                .descendants()
                .any(|node| node.kind() == SyntaxKind::FunctionDecl
                    && node.text().to_string().contains("method")),
            "{syntax:#?}"
        );
        if source.starts_with("enum") {
            assert!(
                syntax
                    .descendants()
                    .any(|node| node.kind() == SyntaxKind::Variant && node.text() == "Next"),
                "{syntax:#?}"
            );
        }
    }
}

#[test]
fn annotation_parameters_recover_without_losing_the_next_declaration() {
    let source = "annotation class A(val bad: , val good: Int)\nfun next() = 1";
    let parsed = parse_recovering(source).unwrap();
    assert!(parsed.ast().is_none());
    let syntax = parsed.syntax();
    assert_eq!(syntax.text().to_string(), source);
    assert!(
        syntax
            .descendants()
            .any(|node| node.kind() == SyntaxKind::Parameter && node.text() == "val good: Int")
    );
    assert!(
        syntax
            .descendants()
            .any(|node| node.kind() == SyntaxKind::FunctionDecl)
    );
}

#[test]
fn field_list_rules_still_distinguish_their_source_forms() {
    for source in [
        "struct S()",
        "struct S(val x: Int = 1)",
        "enum E { V(Int, String), C(val x: Int = 1), N {a: Int\nb: String,} }",
        "annotation class A(val x: Int = 1,)",
    ] {
        let parsed = parse_recovering(source).unwrap();
        assert!(
            parsed.ast().is_some(),
            "{source}: {:?}",
            parsed.diagnostics()
        );
    }
    for source in [
        "struct S(val x: Int,)",
        "struct S(var x: Int)",
        "enum E { V() }",
        "enum E { V(Int,) }",
        "enum E { V(val x: Int,) }",
        "enum E { V(var x: Int) }",
        "enum E { V {} }",
        "enum E { V {x: Int = 1} }",
        "annotation class A(var x: Int)",
    ] {
        let parsed = parse_recovering(source).unwrap();
        assert!(parsed.ast().is_none(), "{source}");
        assert_eq!(parsed.syntax().text().to_string(), source);
    }
}
