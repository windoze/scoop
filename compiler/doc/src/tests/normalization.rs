use super::*;

#[test]
fn adjacent_lines_and_separate_fragments_keep_markdown_paragraphs() {
    let source =
        "/// One\r\n  /// Two\r\n\r\n/// Three\r\n/* ordinary */\r\n/** Four */\r\nstruct S()";
    let result = prepare(source);
    assert!(result.warnings.is_empty());
    assert_eq!(result.documents.len(), 1);
    assert_eq!(
        result.documents[0].body.text.text,
        "One\nTwo\n\nThree\n\nFour"
    );
    let text = &result.documents[0].body.text;
    for word in ["One", "Two", "Three", "Four"] {
        let at = text.text.find(word).unwrap() as u32;
        assert_eq!(
            original(
                source,
                text.source_span(Span::new(at, at + word.len() as u32))
            ),
            word
        );
    }
}

#[test]
fn decorated_blocks_preserve_code_indentation_and_unicode_byte_positions() {
    let source = "/**\r\n * 中文 😀.\r\n *\r\n *     @param code [Ignored]\r\n *\r\n * @param 数字 The value.\r\n */\r\nfun f(数字: Int) {}";
    let result = prepare(source);
    assert!(result.warnings.is_empty());
    let document = &result.documents[0];
    assert_eq!(
        document.body.text.text,
        "中文 😀.\n\n    @param code [Ignored]"
    );
    assert!(document.body.references.is_empty());
    assert_eq!(document.tags.len(), 1);
    let DocTagKind::Param(name) = &document.tags[0].kind else {
        panic!("the source parameter name survives normalization")
    };
    assert_eq!(name.text, "数字");
    assert_eq!(original(source, name.span), "数字");
    assert_eq!(original(source, document.tags[0].span), "@param");
    let mapped = document.body.text.source_span(Span::new(0, 11));
    assert_eq!(original(source, mapped), "中文 😀");
    let newline = document.body.text.source_span(Span::new(12, 13));
    assert_eq!(original(source, newline), "\r\n");
}

#[test]
fn undecorated_blocks_remove_common_indent_without_flattening_code() {
    let source = "/**\n    Text.\n\n        val example = 1\n\n    More.\n*/\nstruct S()";
    let result = prepare(source);
    assert!(result.warnings.is_empty());
    assert_eq!(
        result.documents[0].body.text.text,
        "Text.\n\n    val example = 1\n\nMore."
    );
}

#[test]
fn a_document_starting_with_indented_code_stays_code() {
    let source = "///     @param example [Ignored]\n///     let x = 1\nfun f() {}";
    let result = prepare(source);
    assert!(result.warnings.is_empty());
    assert!(result.documents[0].tags.is_empty());
    assert!(result.documents[0].body.references.is_empty());
    assert!(result.documents[0].body.text.text.starts_with("    @param"));
}

#[test]
fn empty_comments_and_ordinary_comments_keep_their_distinct_meanings() {
    let source =
        "/***/\n//// ordinary\n//! ordinary\n/**/\nstruct First()\n/**\n *\n */\nstruct Second()";
    let result = prepare(source);
    assert!(result.warnings.is_empty());
    assert_eq!(result.documents.len(), 2);
    assert!(
        result
            .documents
            .iter()
            .all(|doc| doc.body.text.text.is_empty())
    );
}

#[test]
fn unattached_comments_report_the_original_fragment() {
    let source = "/// file header\nimport Option\n/** Attached. */\nstruct S()\n/// end of file";
    let result = prepare(source);
    assert_eq!(result.documents.len(), 1);
    assert_eq!(result.warnings.len(), 2);
    for (warning, expected) in result
        .warnings
        .iter()
        .zip(["/// file header", "/// end of file"])
    {
        assert_eq!(original(source, warning.span.unwrap()), expected);
        assert_eq!(
            warning.message,
            "documentation comment is not attached to a declaration"
        );
    }
}

#[test]
fn slicing_normalized_text_keeps_source_positions_across_removed_prefixes() {
    let source = "/// first\n  /// 中文\n  /// last\nstruct S()";
    let result = prepare(source);
    let text = &result.documents[0].body.text;
    let slice = text.slice(Span::new(6, 17));
    assert_eq!(slice.text, "中文\nlast");
    assert_eq!(original(source, slice.source_span(Span::new(0, 6))), "中文");
    assert_eq!(
        original(source, slice.source_span(Span::new(7, 11))),
        "last"
    );
}
