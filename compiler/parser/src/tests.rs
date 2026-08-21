//! Unit tests: valid programs of every shape, spans, escapes, comments,
//! optional semicolons, and every diagnostic the M1 parser can produce.

use scoop_ast::{Expr, Span, StatementKind};

use crate::parse;

fn ok(source: &str) -> scoop_ast::SourceFile {
    parse(source).unwrap_or_else(|diagnostics| panic!("should parse: {diagnostics:?}"))
}

/// Asserts fail-fast (exactly one diagnostic) and returns its span + message.
fn err(source: &str) -> (Span, String) {
    let diagnostics = parse(source).expect_err("should fail");
    assert_eq!(diagnostics.len(), 1, "M1 is fail-fast");
    let diagnostic = diagnostics.into_iter().next().unwrap();
    (
        diagnostic.span.expect("parser diagnostics carry a span"),
        diagnostic.message,
    )
}

fn only_stmt(file: &scoop_ast::SourceFile) -> &scoop_ast::Statement {
    assert_eq!(file.functions.len(), 1);
    let statements = &file.functions[0].body.statements;
    assert_eq!(statements.len(), 1);
    &statements[0]
}

// --- valid programs -------------------------------------------------------

#[test]
fn empty_file() {
    let file = ok("");
    assert!(file.functions.is_empty());
    assert_eq!(file.span, Span::new(0, 0));
}

#[test]
fn comments_only_file() {
    let file = ok("// nothing here\n/* and\nnothing\nhere either */\n");
    assert!(file.functions.is_empty());
}

#[test]
fn empty_function() {
    let file = ok("fun main() {}");
    assert_eq!(file.functions.len(), 1);
    let function = &file.functions[0];
    assert_eq!(function.name.text, "main");
    assert_eq!(function.name.span, Span::new(4, 8));
    assert_eq!(function.span, Span::new(0, 13));
    assert_eq!(function.body.span, Span::new(11, 13));
    assert!(function.body.statements.is_empty());
}

#[test]
fn hello_world() {
    let file = ok("fun main() {\n    print(\"hello, world\")\n    println(\"!\")\n}\n");
    assert_eq!(
        scoop_ast::dump(&file),
        "SourceFile\n  fun main\n    Call print\n      StringLiteral \"hello, world\"\n    Call println\n      StringLiteral \"!\"\n"
    );
}

#[test]
fn multiple_functions_and_calls() {
    let file = ok("fun greet() {\n    println(\"hi\")\n}\n\nfun main() {\n    greet()\n}\n");
    assert_eq!(file.functions.len(), 2);
    assert_eq!(file.functions[0].name.text, "greet");
    assert_eq!(file.functions[1].name.text, "main");
    let stmt = &file.functions[1].body.statements[0];
    let StatementKind::Expr(Expr::Call(call)) = &stmt.kind else {
        panic!("expected a call statement");
    };
    assert_eq!(call.callee.text, "greet");
    assert!(call.args.is_empty());
}

#[test]
fn string_escapes() {
    let file = ok("fun main() {\n    print(\"a\\nb\\tc\\\\d\\\"e\")\n}\n");
    let stmt = only_stmt(&file);
    let StatementKind::Expr(Expr::Call(call)) = &stmt.kind else {
        panic!("expected a call statement");
    };
    let [Expr::StringLiteral { value, .. }] = &call.args[..] else {
        panic!("expected one string argument");
    };
    assert_eq!(value, "a\nb\tc\\d\"e");
}

#[test]
fn unicode_string_contents() {
    let file = ok("fun main() {\n    print(\"héllo，世界\")\n}\n");
    let stmt = only_stmt(&file);
    let StatementKind::Expr(Expr::Call(call)) = &stmt.kind else {
        panic!("expected a call statement");
    };
    let Expr::StringLiteral { value, .. } = &call.args[0] else {
        panic!("expected a string argument");
    };
    assert_eq!(value, "héllo，世界");
}

#[test]
fn line_and_block_comments() {
    let file = ok(
        "// leading\nfun main() { /* inline */ print(\"a\") // trailing\n    /* multi\nline */ print(\"b\")\n}\n",
    );
    let statements = &file.functions[0].body.statements;
    assert_eq!(statements.len(), 2);
}

#[test]
fn optional_semicolons() {
    let file = ok("fun main() {\n    print(\"a\");\n    print(\"b\"); print(\"c\")\n}\n");
    let statements = &file.functions[0].body.statements;
    assert_eq!(statements.len(), 3);
}

#[test]
fn last_statement_before_closing_brace_needs_no_separator() {
    let file = ok("fun main() { print(\"a\") }");
    assert_eq!(file.functions[0].body.statements.len(), 1);
}

#[test]
fn spans_are_byte_offsets() {
    let file = ok("fun main() {\n    print(\"hi\")\n}\n");
    let function = &file.functions[0];
    assert_eq!(function.span, Span::new(0, 30));
    assert_eq!(function.body.span, Span::new(11, 30));
    let stmt = &function.body.statements[0];
    assert_eq!(stmt.span, Span::new(17, 28));
    let StatementKind::Expr(Expr::Call(call)) = &stmt.kind else {
        panic!("expected a call statement");
    };
    assert_eq!(call.callee.span, Span::new(17, 22));
    assert_eq!(call.span, Span::new(17, 28));
    let Expr::StringLiteral { span, .. } = &call.args[0] else {
        panic!("expected a string argument");
    };
    assert_eq!(*span, Span::new(23, 27));
}

// --- diagnostics ----------------------------------------------------------

#[test]
fn unexpected_character() {
    let (span, message) = err("fun main() {\n    @\n}\n");
    assert_eq!(span, Span::new(17, 18));
    assert_eq!(message, "unexpected character `@`");
}

#[test]
fn unexpected_non_ascii_character() {
    let (_, message) = err("fun main() { λ }");
    assert_eq!(message, "unexpected character `λ`");
}

#[test]
fn unterminated_string_literal() {
    let (span, message) = err("fun main() {\n    print(\"abc\n}\n");
    assert_eq!(span, Span::new(23, 27));
    assert_eq!(message, "unterminated string literal");
}

#[test]
fn unsupported_escape_sequence() {
    let (span, message) = err("fun main() { print(\"a\\q\") }");
    assert_eq!(span, Span::new(21, 23));
    assert_eq!(message, "unsupported escape sequence `\\q`");
}

#[test]
fn unterminated_block_comment() {
    let (span, message) = err("fun main() { /* oops");
    assert_eq!(span, Span::new(13, 20));
    assert_eq!(message, "unterminated block comment");
}

#[test]
fn parameters_not_supported() {
    let (span, message) = err("fun f(x: Int) {}");
    assert_eq!(span, Span::new(6, 7));
    assert_eq!(message, "parameters are not supported yet (milestone M1)");
}

#[test]
fn return_type_annotation_not_supported() {
    let (span, message) = err("fun f(): Int {}");
    assert_eq!(span, Span::new(7, 8));
    assert_eq!(
        message,
        "return type annotations are not supported yet (milestone M1)"
    );
}

#[test]
fn val_not_supported() {
    let (span, message) = err("fun main() {\n    val x\n}\n");
    assert_eq!(span, Span::new(17, 20));
    assert_eq!(
        message,
        "variable declarations are not supported yet (milestone M1)"
    );
}

#[test]
fn var_not_supported() {
    let (_, message) = err("fun main() { var y }");
    assert_eq!(
        message,
        "variable declarations are not supported yet (milestone M1)"
    );
}

#[test]
fn top_level_must_be_fun() {
    let (span, message) = err("main() {}");
    assert_eq!(span, Span::new(0, 4));
    assert_eq!(message, "expected `fun`, found `main`");
}

#[test]
fn missing_function_name() {
    let (span, message) = err("fun () {}");
    assert_eq!(span, Span::new(4, 5));
    assert_eq!(message, "expected function name, found `(`");
}

#[test]
fn missing_parameter_list() {
    let (_, message) = err("fun main {}");
    assert_eq!(message, "expected `(`, found `{`");
}

#[test]
fn missing_body() {
    let (_, message) = err("fun main() x");
    assert_eq!(message, "expected `{`, found `x`");
}

#[test]
fn unclosed_body() {
    let (span, message) = err("fun main() {\n    print(\"a\")\n");
    assert_eq!(span, Span::new(28, 28));
    assert_eq!(message, "expected `}`, found end of file");
}

#[test]
fn bare_identifier_is_not_a_statement() {
    let (_, message) = err("fun main() { foo }");
    assert_eq!(message, "expected `(`, found `}`");
}

#[test]
fn keyword_in_expression_position() {
    let (_, message) = err("fun main() { fun }");
    assert_eq!(message, "expected expression, found `fun`");
}

#[test]
fn missing_comma_between_args() {
    let (_, message) = err("fun main() { print(\"a\" \"b\") }");
    assert_eq!(message, "expected `)`, found string literal");
}

#[test]
fn unclosed_call_at_eof() {
    let (_, message) = err("fun main() { print(\"a\"");
    assert_eq!(message, "expected `)`, found end of file");
}

#[test]
fn trailing_comma_is_an_error() {
    let (_, message) = err("fun main() { print(\"a\",) }");
    assert_eq!(message, "expected expression, found `)`");
}

#[test]
fn statements_on_one_line_need_semicolon() {
    let (span, message) = err("fun main() { print(\"a\") print(\"b\") }");
    assert_eq!(span, Span::new(24, 29));
    assert_eq!(
        message,
        "expected `;` or newline after statement, found `print`"
    );
}
