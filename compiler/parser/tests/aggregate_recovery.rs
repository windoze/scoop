use scoop_parser::{parse_recovering, parse_type_recovering};
use scoop_syntax::SyntaxKind;

#[test]
fn constructors_and_anonymous_functions_keep_later_parameters() {
    for source in [
        "class C(first: , val second: Int) { fun good() = second }",
        "class C { constructor(first: , second: Int) {}\nfun good() = 1 }",
        "fun outer() { val f = fun(first: , second: Int): Int { return second } }",
        "context(first: , second: Int) fun f() = second",
    ] {
        let parsed = parse_recovering(source).unwrap();
        assert!(parsed.ast().is_none(), "{source}");
        let syntax = parsed.syntax();
        assert_eq!(syntax.text().to_string(), source);
        assert!(
            syntax
                .descendants()
                .any(|node| node.kind() == SyntaxKind::Parameter
                    && node.text().to_string().contains("second: Int")),
            "{source}: {syntax:#?}"
        );
        assert!(
            syntax
                .descendants()
                .any(|node| node.kind() == SyntaxKind::FunctionDecl)
        );
    }
}

#[test]
fn annotations_and_nested_arrays_retain_later_values_and_declarations() {
    let source = "@Ann(values = [1, , 3], extra = 4) fun good() = 1";
    let parsed = parse_recovering(source).unwrap();
    assert!(parsed.ast().is_none());
    let syntax = parsed.syntax();
    assert_eq!(syntax.text().to_string(), source);
    assert!(
        syntax
            .descendants()
            .any(|node| node.kind() == SyntaxKind::Argument && node.text() == "extra = 4")
    );
    assert!(
        syntax
            .descendants()
            .any(|node| node.kind() == SyntaxKind::AnnotationArray
                && node.text().to_string().contains('3'))
    );
    assert!(
        syntax
            .descendants()
            .any(|node| node.kind() == SyntaxKind::FunctionDecl)
    );
}

#[test]
fn tuple_and_array_errors_do_not_discard_later_elements() {
    for source in ["fun f() = (1, , 3)", "fun f() = [1, , 3]"] {
        let parsed = parse_recovering(source).unwrap();
        assert!(parsed.ast().is_none());
        let syntax = parsed.syntax();
        assert_eq!(syntax.text().to_string(), source);
        assert!(
            syntax
                .descendants()
                .any(|node| node.kind() == SyntaxKind::Expression && node.text() == "3")
        );
    }
    for source in ["(Int, , String)", "(Int, , String) -> Bool"] {
        let parsed = parse_type_recovering(source).unwrap();
        assert!(parsed.ast().is_none());
        let syntax = parsed.syntax();
        assert_eq!(syntax.text().to_string(), source);
        assert!(
            syntax
                .descendants()
                .any(|node| node.kind() == SyntaxKind::Type && node.text() == "String")
        );
    }
}

#[test]
fn aggregate_trailing_comma_rules_stay_distinct() {
    for source in [
        "@Ann(1,) fun f() = (1,)",
        "@Ann(values = [1, 2,]) fun f() = ()",
        "fun f(x: (Int,)) = (1, 2,)",
        "class C(val x: Int) {}",
    ] {
        let parsed = parse_recovering(source).unwrap();
        assert!(
            parsed.diagnostics().is_empty(),
            "{source}: {:?}",
            parsed.diagnostics()
        );
        assert!(parsed.ast().is_some());
    }
    for source in [
        "fun f() = [1,]",
        "class C(val x: Int,) {}",
        "fun f() = fun(x: Int,) {}",
        "context(x: Int,) fun f() {}",
        "context(x: Int = 1) fun f() {}",
    ] {
        let parsed = parse_recovering(source).unwrap();
        assert!(parsed.ast().is_none(), "{source}");
    }
}

#[test]
fn unfinished_annotations_and_every_prefix_have_a_syntax_result() {
    for source in [
        "@",
        "@Ann(",
        "@Ann(values = [",
        "@field:",
        "@Ann(values = [1, 2]) class C(val x: Int) { fun f() = (1, 2) }",
        "fun outer() { val f = fun(x: Int): Int { return x } }",
        "context(value: Int) fun <T> f(x: (T, T)): T = x._0",
    ] {
        for end in source
            .char_indices()
            .map(|(index, _)| index)
            .chain(std::iter::once(source.len()))
        {
            let prefix = &source[..end];
            let parsed = parse_recovering(prefix).unwrap();
            assert_eq!(parsed.syntax().text().to_string(), prefix);
        }
    }
}
