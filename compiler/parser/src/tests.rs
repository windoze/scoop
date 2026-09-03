//! Unit tests carried over from M1: valid programs of every M1 shape,
//! spans, escapes, comments, optional semicolons, and the diagnostics
//! whose behavior M2 keeps. M2-only syntax and diagnostics live in
//! `tests_m2.rs`.

use scoop_ast::{Decl, Expr, FunctionBody, FunctionDecl, Span, StatementKind};

use crate::parse;

pub(crate) fn ok(source: &str) -> scoop_ast::SourceFile {
    parse(source).unwrap_or_else(|diagnostics| panic!("should parse: {diagnostics:?}"))
}

/// Asserts that this focused negative case produces exactly one diagnostic
/// and returns its span + message.
pub(crate) fn err(source: &str) -> (Span, String) {
    let diagnostics = parse(source).expect_err("should fail");
    assert_eq!(
        diagnostics.len(),
        1,
        "expected one focused diagnostic: {diagnostics:?}"
    );
    let diagnostic = diagnostics.into_iter().next().unwrap();
    (
        diagnostic.span.expect("parser diagnostics carry a span"),
        diagnostic.message,
    )
}

pub(crate) fn only_function(file: &scoop_ast::SourceFile) -> &scoop_ast::FunctionDecl {
    assert_eq!(file.declarations.len(), 1);
    let Decl::Function(function) = &file.declarations[0] else {
        panic!("expected a function declaration");
    };
    function
}

pub(crate) fn block_body(function: &FunctionDecl) -> &scoop_ast::Block {
    match &function.body {
        FunctionBody::Block(block) => block,
        FunctionBody::Expr(_) | FunctionBody::None => panic!("expected a block body"),
    }
}

fn only_stmt(file: &scoop_ast::SourceFile) -> &scoop_ast::Statement {
    let statements = &block_body(only_function(file)).statements;
    assert_eq!(statements.len(), 1);
    &statements[0]
}

// --- valid programs -------------------------------------------------------

#[test]
fn empty_file() {
    let file = ok("");
    assert!(file.declarations.is_empty());
    assert_eq!(file.span, Span::new(0, 0));
}

#[test]
fn comments_only_file() {
    let file = ok("// nothing here\n/* and\nnothing\nhere either */\n");
    assert!(file.declarations.is_empty());
}

#[test]
fn parser_recovers_across_statements_and_top_level_declarations() {
    let diagnostics = parse(
        "fun first() {\n    val = 1\n    println(\"ok\")\n    val x =\n}\n\
         fun broken(: Int) {}\n\
         fun main() {\n    val = 2\n}\n",
    )
    .expect_err("four independent syntax errors must be collected");
    let messages: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect();
    assert_eq!(
        messages,
        [
            "expected pattern, found `=`",
            "expected expression, found `}`",
            "expected parameter name, found `:`",
            "expected pattern, found `=`",
        ]
    );
    assert!(diagnostics.windows(2).all(|pair| {
        pair[0].span.expect("spanned").start < pair[1].span.expect("spanned").start
    }));
}

#[test]
fn parser_recovers_between_type_members() {
    let diagnostics = parse(
        "class C {\n\
             val x = 1\n\
             fun broken(: Int) {}\n\
             object Nested\n\
             fun ok() {}\n\
         }\n\
         fun main() {}\n",
    )
    .expect_err("independent member errors must be collected");
    let messages: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect();
    assert_eq!(
        messages,
        [
            "class stored properties require an explicit type",
            "expected parameter name, found `:`",
            "`object` declarations are not supported yet (milestone M21)",
        ]
    );
}

#[test]
fn lexer_collects_multiple_bad_characters() {
    let diagnostics =
        parse("§\n$\nfun main() {}\n").expect_err("independent lexical errors must be collected");
    let messages: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect();
    assert_eq!(
        messages,
        ["unexpected character `§`", "unexpected character `$`"]
    );
}

#[test]
fn empty_function() {
    let file = ok("fun main() {}");
    assert_eq!(file.declarations.len(), 1);
    let function = only_function(&file);
    assert_eq!(function.name.text, "main");
    assert_eq!(function.name.span, Span::new(4, 8));
    assert_eq!(function.span, Span::new(0, 13));
    assert!(function.type_params.is_empty());
    assert!(function.params.is_empty());
    assert!(function.return_ty.is_none());
    let body = block_body(function);
    assert_eq!(body.span, Span::new(11, 13));
    assert!(body.statements.is_empty());
}

#[test]
fn hello_world() {
    let file = ok("fun main() {\n    print(\"hello, world\")\n    println(\"!\")\n}\n");
    assert_eq!(
        scoop_ast::dump(&file),
        "SourceFile\n  fun main()\n    Call print\n      StringLiteral \"hello, world\"\n    Call println\n      StringLiteral \"!\"\n"
    );
}

#[test]
fn multiple_functions_and_calls() {
    let file = ok("fun greet() {\n    println(\"hi\")\n}\n\nfun main() {\n    greet()\n}\n");
    assert_eq!(file.declarations.len(), 2);
    let Decl::Function(greet) = &file.declarations[0] else {
        panic!("expected a function declaration");
    };
    assert_eq!(greet.name.text, "greet");
    let main = &file.declarations[1];
    let Decl::Function(main) = main else {
        panic!("expected a function declaration");
    };
    assert_eq!(main.name.text, "main");
    let stmt = &block_body(main).statements[0];
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
    let [argument] = &call.args[..] else {
        panic!("expected one string argument");
    };
    let Expr::StringLiteral { value, .. } = &argument.expression else {
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
    let Expr::StringLiteral { value, .. } = &call.args[0].expression else {
        panic!("expected a string argument");
    };
    assert_eq!(value, "héllo，世界");
}

#[test]
fn line_and_block_comments() {
    let file = ok(
        "// leading\nfun main() { /* inline */ print(\"a\") // trailing\n    /* multi\nline */ print(\"b\")\n}\n",
    );
    let statements = &block_body(only_function(&file)).statements;
    assert_eq!(statements.len(), 2);
}

#[test]
fn optional_semicolons() {
    let file = ok("fun main() {\n    print(\"a\");\n    print(\"b\"); print(\"c\")\n}\n");
    let statements = &block_body(only_function(&file)).statements;
    assert_eq!(statements.len(), 3);
}

#[test]
fn last_statement_before_closing_brace_needs_no_separator() {
    let file = ok("fun main() { print(\"a\") }");
    assert_eq!(block_body(only_function(&file)).statements.len(), 1);
}

#[test]
fn bare_identifier_is_a_var_expression() {
    // M1 rejected this; in M2 a bare identifier is a `Var` expression.
    let file = ok("fun main() { foo }");
    let stmt = only_stmt(&file);
    let StatementKind::Expr(Expr::Var(ident)) = &stmt.kind else {
        panic!("expected a variable expression");
    };
    assert_eq!(ident.text, "foo");
}

#[test]
fn spans_are_byte_offsets() {
    let file = ok("fun main() {\n    print(\"hi\")\n}\n");
    let function = only_function(&file);
    assert_eq!(function.span, Span::new(0, 30));
    let body = block_body(function);
    assert_eq!(body.span, Span::new(11, 30));
    let stmt = &body.statements[0];
    assert_eq!(stmt.span, Span::new(17, 28));
    let StatementKind::Expr(Expr::Call(call)) = &stmt.kind else {
        panic!("expected a call statement");
    };
    assert_eq!(call.callee.span, Span::new(17, 22));
    assert_eq!(call.span, Span::new(17, 28));
    let Expr::StringLiteral { span, .. } = &call.args[0].expression else {
        panic!("expected a string argument");
    };
    assert_eq!(*span, Span::new(23, 27));
}

// --- diagnostics ----------------------------------------------------------

#[test]
fn incomplete_safety_annotation_in_statement_position() {
    let (_, message) = err("fun main() {\n    @\n}\n");
    assert_eq!(message, "expected annotation name, found `}`");
}

#[test]
fn unexpected_non_ascii_character() {
    let (_, message) = err("fun main() { λ }");
    assert_eq!(message, "unexpected character `λ`");
}

#[test]
fn single_ampersand_is_unexpected() {
    let (_, message) = err("fun main() { a & b }");
    assert_eq!(message, "unexpected character `&`");
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
fn val_without_initializer() {
    let (_, message) = err("fun main() {\n    val x\n}\n");
    assert_eq!(message, "expected `=`, found `}`");
}

#[test]
fn top_level_must_be_a_declaration() {
    let (span, message) = err("main() {}");
    assert_eq!(span, Span::new(0, 4));
    assert_eq!(
        message,
        "expected `fun`, `val`, `var`, `struct`, `enum`, `class` or `interface`, found `main`"
    );
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
    assert_eq!(
        message,
        "expected `fun`, `val`, `var`, `struct`, `enum`, `class` or `interface`, found `x`"
    );
}

#[test]
fn unclosed_body() {
    let (span, message) = err("fun main() {\n    print(\"a\")\n");
    assert_eq!(span, Span::new(28, 28));
    assert_eq!(message, "expected `}`, found end of file");
}

#[test]
fn local_function_missing_name() {
    let (_, message) = err("fun main() { fun }");
    assert_eq!(message, "expected function name, found `}`");
}

#[test]
fn adjacent_arguments_form_one_property_like_infix_expression() {
    let file = ok("fun main() { print(command \"build\") }");
    let StatementKind::Expr(Expr::Call(call)) = &only_stmt(&file).kind else {
        panic!("expected outer call");
    };
    assert!(matches!(
        call.args[0].expression,
        Expr::InfixCall {
            target: scoop_ast::InfixTarget::Invoke,
            ..
        }
    ));
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
    let (span, message) = err("fun main() { print(\"a\") val x = 1 }");
    assert_eq!(span, Span::new(24, 27));
    assert_eq!(
        message,
        "expected `;` or newline after statement, found `val`"
    );
}
