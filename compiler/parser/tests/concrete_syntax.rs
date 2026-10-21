use scoop_parser::{parse_function_body_recovering, parse_recovering, parse_type_recovering};
use scoop_syntax::{ExpectedSyntax, Parse, SyntaxKind, TextSize};

fn assert_original<T>(parsed: &Parse<T>, source: &str) {
    let syntax = parsed.syntax();
    assert_eq!(syntax.text().to_string(), source);
    let tokens: String = syntax
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .map(|token| token.text().to_owned())
        .collect();
    assert_eq!(tokens, source);
    assert_eq!(u32::from(syntax.text_range().end()) as usize, source.len());
    for missing in parsed.missing() {
        assert!(source.is_char_boundary(missing.offset as usize));
    }
}

#[test]
fn preserves_all_trivia_literals_and_erroneous_text() {
    for source in [
        "",
        "\r\n\r\n",
        "#! /usr/bin/env scoop\n/// 中文\r\nfun main() {}\n",
        "/* block */ fun `中文`() = \"é 😀\" // last\r",
        "fun strings() = f\"a ${f\"b ${value}\"} c\"",
        "fun bad() { val value = \"unterminated\n}\nfun next() = 2",
        "fun bad() = f\"${value",
        "fun bad() = f\"text",
        "/* unterminated 中文",
        "fun f() {}\0 fun next() {}",
        "@💥\nfun next() = 2",
        "fun bad() { val = 1\n val = 2\n }\nfun next() {}",
        "fun f() { return (1 + ) }",
        "fun f() { object : Base { fun g() {} } }",
    ] {
        let parsed = parse_recovering(source).unwrap();
        assert_eq!(parsed.syntax().kind(), SyntaxKind::SourceFile);
        assert_original(&parsed, source);
        assert_eq!(parsed.ast().is_some(), parsed.diagnostics().is_empty());
    }
}

#[test]
fn fragments_have_their_own_roots_even_when_invalid() {
    for source in [
        "{}",
        "/* before */ { return 1 } // after",
        "{ val = 1 } fun extra() {}",
        "",
    ] {
        let parsed = parse_function_body_recovering(source).unwrap();
        assert_eq!(parsed.syntax().kind(), SyntaxKind::FunctionBody);
        assert_original(&parsed, source);
    }
    for source in [
        "List<Int>",
        "(@Ann Int, String?)",
        "List<Int",
        "Int More",
        "",
    ] {
        let parsed = parse_type_recovering(source).unwrap();
        assert_eq!(parsed.syntax().kind(), SyntaxKind::TypeFragment);
        assert_original(&parsed, source);
    }
}

#[test]
fn records_missing_items_and_skipped_tokens_without_fake_ast() {
    let source = "fun (x: Int) {}\nfun good() = 2";
    let parsed = parse_recovering(source).unwrap();
    assert_original(&parsed, source);
    assert!(parsed.ast().is_none());
    assert!(
        parsed
            .missing()
            .iter()
            .any(|item| item.offset == 4 && item.expected == ExpectedSyntax::Name)
    );
    let syntax = parsed.syntax();
    assert!(
        syntax
            .descendants()
            .any(|node| node.kind() == SyntaxKind::Error)
    );
    assert!(
        syntax
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::Missing)
            .all(|node| node.text_range().is_empty())
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
fn valid_probes_leave_no_missing_or_duplicate_nodes() {
    for source in [
        "fun main() { val f = { value }\n val g = { x: Int -> x }\n val b = left < right }",
        "class Empty\n/// next\nfun good() = 1",
        "fun <T> List<T>.first(): T = this[0]",
        "val x = 1\n@Ann fun next() {}",
        "fun main() { fun local() = 1\n val f = fun(x: Int): Int { return x }\n val c = f<Int>(value) }",
    ] {
        let parsed = parse_recovering(source).unwrap();
        assert_original(&parsed, source);
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
    }
}

#[test]
fn token_boundaries_and_ancestors_remain_available_at_eof() {
    let source = "fun f(x: Int) { return x }";
    let syntax = parse_recovering(source).unwrap().syntax();
    let offset = TextSize::from(source.rfind('x').unwrap() as u32);
    let token = syntax.token_at_offset(offset).right_biased().unwrap();
    assert_eq!(token.text(), "x");
    assert!(
        token
            .parent_ancestors()
            .any(|node| node.kind() == SyntaxKind::Block)
    );
    assert!(
        token
            .parent_ancestors()
            .any(|node| node.kind() == SyntaxKind::FunctionDecl)
    );
    let last = syntax
        .token_at_offset(TextSize::from(source.len() as u32))
        .left_biased()
        .unwrap();
    assert_eq!(last.text(), "}");
}

#[test]
fn editing_sequences_keep_every_byte_and_finish_recovery() {
    let source = "/// 中文\nfun <T> f(x: List<T>): T { return x[0] }\nfun g() = f<Int>(values)";
    for (offset, ch) in source.char_indices() {
        let mut removed = source.to_owned();
        removed.replace_range(offset..offset + ch.len_utf8(), "");
        assert_original(&parse_recovering(&removed).unwrap(), &removed);
        for insertion in [")", ".", "\"", ","] {
            let mut inserted = source.to_owned();
            inserted.insert_str(offset, insertion);
            assert_original(&parse_recovering(&inserted).unwrap(), &inserted);
        }
    }
}
