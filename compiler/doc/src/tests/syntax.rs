use super::*;
use scoop_ast::DocSeeTarget;

#[test]
fn prepares_all_tags_and_preserves_quoted_names() {
    let source = r#"/**
 * Summary with [Record].
 * @param `value name` The argument.
 * @property count A counter.
 * @constructor Creates the record.
 * @receiver The receiver.
 * @return The result.
 * @throws errors.Failure An error.
 * @see Record.method Related API.
 * @see https://example.org/guide External guide.
 * @since 1.2
 */
fun f() {}
"#;
    let result = prepare(source);
    assert!(result.warnings.is_empty(), "{:?}", result.warnings);
    let document = &result.documents[0];
    assert_eq!(document.tags.len(), 9);
    assert_eq!(document.body.references.len(), 1);
    let DocTagKind::Param(name) = &document.tags[0].kind else {
        panic!()
    };
    assert_eq!(name.text, "value name");
    assert_eq!(original(source, name.span), "`value name`");
    assert_eq!(document.tags[0].body.text.text, "The argument.");
    assert!(matches!(document.tags[1].kind, DocTagKind::Property(_)));
    assert!(matches!(document.tags[2].kind, DocTagKind::Constructor));
    assert!(matches!(document.tags[3].kind, DocTagKind::Receiver));
    assert!(matches!(document.tags[4].kind, DocTagKind::Return));
    let DocTagKind::Throws(path) = &document.tags[5].kind else {
        panic!()
    };
    assert_eq!(path.to_string(), "errors.Failure");
    assert_eq!(original(source, path.span), "errors.Failure");
    assert!(matches!(
        document.tags[6].kind,
        DocTagKind::See(DocSeeTarget::Symbol(_))
    ));
    assert!(matches!(
        document.tags[7].kind,
        DocTagKind::See(DocSeeTarget::Url { .. })
    ));
    assert_eq!(document.tags[8].body.text.text, "1.2");
}

#[test]
fn markdown_code_and_html_do_not_create_tags_or_symbol_candidates() {
    let source = r#"/**
 * ```scoop
 * @param fake [NotAReference]
 * ```
 *
 * A multiline `inline
 * @return [StillCode]` example.
 *
 * <div>
 * @throws NotATag
 * </div>
 *
 * @return [Result] outside code.
 */
fun f() {}
"#;
    let result = prepare(source);
    assert!(result.warnings.is_empty(), "{:?}", result.warnings);
    let document = &result.documents[0];
    assert_eq!(document.tags.len(), 1);
    assert!(document.body.references.is_empty());
    assert_eq!(document.tags[0].body.references.len(), 1);
    assert_eq!(
        document.tags[0].body.references[0].path.to_string(),
        "Result"
    );
}

#[test]
fn explicit_reference_definitions_apply_across_tag_sections() {
    let source = r#"/**
 * [Thing] and [the guide][thing].
 * @return [THING] and [Other].
 *
 * [Thing]: https://example.org/reference "A guide"
 */
fun f() {}
"#;
    let result = prepare(source);
    assert!(result.warnings.is_empty(), "{:?}", result.warnings);
    let document = &result.documents[0];
    assert!(document.body.references.is_empty());
    assert_eq!(document.body.definitions.len(), 1);
    assert_eq!(document.tags[0].body.references.len(), 1);
    assert_eq!(
        document.tags[0].body.references[0].path.to_string(),
        "Other"
    );
}

#[test]
fn only_undefined_text_links_become_symbol_candidates() {
    let source = r#"/**
 * [Record] [read][Record.method] [Record][] [`with.dot`.`a b`].
 * ![Record] ` [Ignored] ` [web](https://example.org/) <https://example.org/>.
 */
struct S()
"#;
    let result = prepare(source);
    assert!(result.warnings.is_empty(), "{:?}", result.warnings);
    let body = &result.documents[0].body;
    assert_eq!(body.references.len(), 4);
    let path = &body.references[3].path;
    assert_eq!(path.first.text, "with.dot");
    assert_eq!(path.rest.len(), 1);
    assert_eq!(path.rest[0].identifier.text, "a b");
    assert_eq!(original(source, path.first.span), "`with.dot`");
    assert_eq!(original(source, path.rest[0].identifier.span), "`a b`");
    assert_eq!(original(source, path.rest[0].dot_span), ".");
}

#[test]
fn malformed_tags_report_exact_original_spans() {
    let source = "/**\r\n * @param\r\n * Not a parameter name on the next line.\r\n * @property\r\n * @throws\r\n * @see\r\n * @since\r\n * @unknown text\r\n */\r\nfun f() {}";
    let result = prepare(source);
    assert!(result.documents[0].tags.is_empty());
    assert_eq!(result.warnings.len(), 6, "{:?}", result.warnings);
    for (warning, spelling) in result.warnings.iter().zip([
        "@param",
        "@property",
        "@throws",
        "@see",
        "@since",
        "@unknown",
    ]) {
        assert_eq!(original(source, warning.span.unwrap()), spelling);
    }
}

#[test]
fn invalid_symbol_and_subject_spellings_have_document_warnings() {
    let source = "/**\n * [not a symbol]\n * @param a.b Invalid parameter.\n * @throws Broken<Type> Invalid type path.\n */\nfun f() {}";
    let result = prepare(source);
    assert_eq!(result.warnings.len(), 3, "{:?}", result.warnings);
    assert!(result.documents[0].tags.is_empty());
    for (warning, expected) in result
        .warnings
        .iter()
        .zip(["[not a symbol]", "a.b", "Broken<Type>"])
    {
        assert_eq!(original(source, warning.span.unwrap()), expected);
    }
}

#[test]
fn escaped_markdown_and_indented_tag_continuations_keep_their_text() {
    let source = r#"/**
 * \@param is prose, and \[NotALink] is literal.
 * @return Example:
 *
 *     @param stillCode [Ignored]
 *
 * More text with [Result].
 */
fun f() {}
"#;
    let result = prepare(source);
    assert!(result.warnings.is_empty(), "{:?}", result.warnings);
    assert_eq!(result.documents[0].tags.len(), 1);
    assert!(result.documents[0].body.references.is_empty());
    let body = &result.documents[0].tags[0].body;
    assert_eq!(body.references.len(), 1);
    assert!(body.text.text.contains("    @param stillCode [Ignored]"));
}
