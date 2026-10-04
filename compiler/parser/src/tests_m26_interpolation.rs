use scoop_ast::{Expr, FunctionBody, Span, StringPart};

use crate::lexer::{TokenKind, lex};
use crate::tests::{ok, only_function};

#[test]
fn interpolation_parts_keep_original_unicode_spans_and_decoded_dollars() {
    let source = r#"fun text() = f"雪\n${"x"}\u0024{ignored} \$name \${also}""#;
    let parsed = ok(source);
    let Expr::InterpolatedString { parts, span } = expression(&parsed) else {
        panic!("expected interpolation");
    };
    assert_eq!(
        &source[span.start as usize..span.end as usize],
        &source[13..]
    );
    let [
        StringPart::Text { value: first, .. },
        StringPart::Expression { value, span },
        StringPart::Text { value: last, .. },
    ] = parts.as_slice()
    else {
        panic!("expected ordered text, expression, text");
    };
    assert_eq!(first, "雪\n");
    assert_eq!(&source[span.start as usize..span.end as usize], "${\"x\"}");
    let expression = value.span();
    assert_eq!(
        &source[expression.start as usize..expression.end as usize],
        "\"x\""
    );
    assert_eq!(last, "${ignored} $name ${also}");
}

#[test]
fn nested_interpolation_uses_one_callable_identity_sequence() {
    let source = r#"fun text() = f"${({ -> 1 })()} ${f"${({ -> 2 })()}"}""#;
    let dump = scoop_ast::dump(&ok(source));
    assert!(dump.contains("Lambda 0 suspend=false"));
    assert!(dump.contains("Lambda 1 suspend=false"));
    assert_eq!(dump.matches("Lambda ").count(), 2);
}

#[test]
fn expression_comments_and_string_braces_do_not_close_interpolation() {
    let source = "f\"${if (true) { \"}\" /* } */ } else { // }\n \"x\" }}\"";
    let (tokens, diagnostics) = lex(source);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(
        tokens
            .iter()
            .filter(|token| token.kind == TokenKind::InterpolationEnd)
            .count(),
        1
    );
    assert_eq!(
        tokens
            .iter()
            .filter(|token| token.kind == TokenKind::RBrace)
            .count(),
        2
    );
}

#[test]
fn raw_strings_keep_newlines_backslashes_and_final_extra_quotes() {
    let source = "fun text() = \"\"\"雪\\n${value}\nend\"\"\"\"";
    let parsed = ok(source);
    let Expr::StringLiteral { value, .. } = expression(&parsed) else {
        panic!("expected raw string literal");
    };
    assert_eq!(value, "雪\\n${value}\nend\"");
    let source = "fun text() = f\"\"\"雪\\n${1}\nend\"\"\"\"";
    let parsed = ok(source);
    let Expr::InterpolatedString { parts, .. } = expression(&parsed) else {
        panic!("expected raw interpolation");
    };
    assert!(matches!(&parts[0], StringPart::Text { value, .. } if value == "雪\\n"));
    assert!(matches!(&parts[2], StringPart::Text { value, .. } if value == "\nend\""));
}

#[test]
fn malformed_interpolation_reports_the_delimiter_in_the_original_file() {
    let source = "fun text() = f\"雪${}\"";
    let errors = crate::parse(source).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "expected expression, found interpolation closing `}`"
    );
    let at = source.find('}').unwrap() as u32;
    assert_eq!(errors[0].span, Some(Span::new(at, at + 1)));
}

#[test]
fn unfinished_string_modes_have_finite_spanned_diagnostics() {
    for (source, message) in [
        ("f\"unfinished\n", "unterminated f-string literal"),
        ("f\"\"\"unfinished", "unterminated f-string literal"),
        ("f\"${1", "unterminated string interpolation"),
        ("\"\"\"unfinished", "unterminated raw string literal"),
    ] {
        let (tokens, errors) = lex(source);
        assert!(matches!(tokens.last().unwrap().kind, TokenKind::Eof));
        assert_eq!(errors.len(), 1, "{source}: {errors:?}");
        assert_eq!(errors[0].message, message);
        assert!(errors[0].span.is_some());
    }
}

#[test]
fn ordinary_strings_never_tokenize_interpolation() {
    let (tokens, errors) = lex("\"$name ${unknown}\"");
    assert!(errors.is_empty());
    assert!(matches!(&tokens[0].kind, TokenKind::Str(value) if value == "$name ${unknown}"));
}

fn expression(parsed: &scoop_ast::SourceFile) -> &Expr {
    let FunctionBody::Expr(value) = &only_function(parsed).body else {
        panic!("expected expression body");
    };
    value
}
