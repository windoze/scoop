//! M22 integer-literal syntax: radix, separators, suffixes and unsigned
//! magnitude retention before HIR commits an exact integer type.

use scoop_ast::{
    AnnotationLiteral, Decl, Expr, FunctionBody, IntegerLiteralSyntax, IntegerRadix, IntegerSuffix,
    Pattern, StatementKind, UnOp,
};

use crate::lexer::{IntegerLiteralLexeme, TokenKind, lex};
use crate::tests::{ok, only_function};
use crate::tests_m2::{init_expr, stmt_dump};

fn expression_literal(source: &str) -> IntegerLiteralSyntax {
    let Expr::IntLiteral(literal) = init_expr(source) else {
        panic!("expected an integer literal");
    };
    literal
}

fn lexeme(source: &str) -> IntegerLiteralLexeme {
    let (tokens, diagnostics) = lex(source);
    assert!(
        diagnostics.is_empty(),
        "unexpected diagnostics: {diagnostics:?}"
    );
    assert_eq!(tokens.len(), 2, "expected one token and EOF: {tokens:?}");
    let TokenKind::Int(literal) = tokens[0].kind else {
        panic!("expected an integer token: {tokens:?}");
    };
    assert_eq!(tokens[0].span.start, 0);
    assert_eq!(tokens[0].span.end as usize, source.len());
    literal
}

#[test]
fn retains_radix_magnitude_suffix_and_full_span() {
    for (source, magnitude, radix, suffix) in [
        ("12_345", 12_345, IntegerRadix::Decimal, IntegerSuffix::None),
        (
            "0B1010_0110U",
            0b1010_0110,
            IntegerRadix::Binary,
            IntegerSuffix::Unsigned,
        ),
        (
            "0XdeAD_beEFL",
            0xdead_beef,
            IntegerRadix::Hexadecimal,
            IntegerSuffix::Long,
        ),
        (
            "0xffUL",
            255,
            IntegerRadix::Hexadecimal,
            IntegerSuffix::UnsignedLong,
        ),
    ] {
        let literal = expression_literal(source);
        assert_eq!(literal.magnitude, magnitude);
        assert_eq!(literal.radix, radix);
        assert_eq!(literal.suffix, suffix);
        assert_eq!(literal.span.start, 25);
        assert_eq!(literal.span.end as usize, 25 + source.len());
        assert_eq!(literal.span, init_expr(source).span());
    }
}

#[test]
fn accepts_every_frozen_suffix_spelling() {
    for (source, suffix) in [
        ("1", IntegerSuffix::None),
        ("1u", IntegerSuffix::Unsigned),
        ("1U", IntegerSuffix::Unsigned),
        ("1l", IntegerSuffix::Long),
        ("1L", IntegerSuffix::Long),
        ("1ul", IntegerSuffix::UnsignedLong),
        ("1uL", IntegerSuffix::UnsignedLong),
        ("1Ul", IntegerSuffix::UnsignedLong),
        ("1UL", IntegerSuffix::UnsignedLong),
    ] {
        assert_eq!(lexeme(source).suffix, suffix, "source: {source}");
    }
}

#[test]
fn accepts_u64_maximum_in_every_radix() {
    let binary = format!("0b{}", "1".repeat(64));
    for source in [
        "18446744073709551615".to_string(),
        binary,
        "0xffff_ffff_ffff_ffff".to_string(),
    ] {
        assert_eq!(lexeme(&source).magnitude, u64::MAX, "source: {source}");
    }
}

#[test]
fn leading_zeroes_are_decimal_not_octal() {
    let literal = lexeme("0012");
    assert_eq!(literal.magnitude, 12);
    assert_eq!(literal.radix, IntegerRadix::Decimal);
}

#[test]
fn minus_remains_a_unary_node_outside_unsigned_magnitude() {
    let expr = init_expr("-9223372036854775808");
    let Expr::Unary {
        op: UnOp::Neg,
        operand,
        span,
    } = expr
    else {
        panic!("expected unary minus");
    };
    let Expr::IntLiteral(literal) = *operand else {
        panic!("expected an unsigned-magnitude operand");
    };
    assert_eq!(literal.magnitude, 9_223_372_036_854_775_808);
    assert_eq!(literal.span.start, 26);
    assert_eq!(span.start, 25);
    assert_eq!(span.end, literal.span.end);
}

#[test]
fn expression_annotation_and_pattern_share_the_syntax_model() {
    let expression = expression_literal("0b10u");

    let file = ok("@A(0b10u) fun f() {}\n");
    let AnnotationLiteral::Int(annotation) = only_function(&file).annotations[0].args[0].value
    else {
        panic!("expected an integer annotation argument");
    };
    assert_eq!(annotation.magnitude, expression.magnitude);
    assert_eq!(annotation.radix, expression.radix);
    assert_eq!(annotation.suffix, expression.suffix);

    let file = ok("fun f(value: UInt) { when (value) { case 0b10u -> {} } }");
    let FunctionBody::Block(body) = &only_function(&file).body else {
        panic!("expected a block body");
    };
    let StatementKind::When(when) = &body.statements[0].kind else {
        panic!("expected a when statement");
    };
    let Pattern::Literal { expr, span } = &when.arms[0].pattern else {
        panic!("expected an integer literal pattern");
    };
    let Expr::IntLiteral(pattern) = &**expr else {
        panic!("expected shared integer syntax in the pattern");
    };
    assert_eq!(pattern.magnitude, expression.magnitude);
    assert_eq!(pattern.radix, expression.radix);
    assert_eq!(pattern.suffix, expression.suffix);
    assert_eq!(*span, pattern.span);
}

#[test]
fn patterns_accept_unary_minus_over_parenthesized_integer_literals() {
    let source = "fun f(value: UInt) { when (value) { case -1u -> {}; case -((1u)) -> {} } }";
    let file = ok(source);
    let FunctionBody::Block(body) = &only_function(&file).body else {
        panic!("expected a block body");
    };
    let StatementKind::When(when) = &body.statements[0].kind else {
        panic!("expected a when statement");
    };
    assert_eq!(when.arms.len(), 2);
    for arm in &when.arms {
        let Pattern::Literal { expr, span } = &arm.pattern else {
            panic!("expected a prefixed integer literal pattern");
        };
        let Expr::Unary {
            op,
            operand,
            span: expr_span,
        } = &**expr
        else {
            panic!("the sign remains explicit in AST");
        };
        assert_eq!(*op, UnOp::Neg);
        assert_eq!(*span, *expr_span);
        assert!(matches!(
            &**operand,
            Expr::IntLiteral(IntegerLiteralSyntax {
                magnitude: 1,
                suffix: IntegerSuffix::Unsigned,
                ..
            })
        ));
    }
    assert!(scoop_ast::dump(&file).contains("arm -1u"));

    for (invalid, expected_diagnostic) in [
        ("-value", "expected numeric literal after unary minus"),
        ("-(value)", "expected numeric literal after unary minus"),
        ("-+1", "expected numeric literal after unary minus"),
        ("+1", "expected pattern"),
        ("--1", "expected pattern"),
    ] {
        let source = format!("fun f(value: Int) {{ when (value) {{ case {invalid} -> {{}} }} }}");
        let diagnostics =
            crate::parse(&source).expect_err("only a direct integer token may follow");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.starts_with(expected_diagnostic)),
            "source `{invalid}` produced {diagnostics:?}",
        );
    }
}

#[test]
fn dumps_canonical_radix_and_suffix_without_separator_noise() {
    assert_eq!(
        stmt_dump("val a = 0B1010_0110U"),
        "val a\n  IntLiteral 0b10100110u\n"
    );
    assert_eq!(
        stmt_dump("val b = 0XdeAD_beEFL"),
        "val b\n  IntLiteral 0xdeadbeefL\n"
    );
    assert_eq!(stmt_dump("val c = 0xffUL"), "val c\n  IntLiteral 0xffuL\n");

    let file = ok("@A(value = 0XffUL) fun f() {}");
    assert_eq!(
        scoop_ast::dump(&file),
        "SourceFile\n  RootPackage\n    @A(value = 0xffuL)\n  fun f()\n"
    );
}

#[test]
fn overflows_report_the_complete_literal_span() {
    for source in [
        "18446744073709551616uL".to_string(),
        format!("0b1{}", "0".repeat(64)),
        "0x1_0000_0000_0000_0000".to_string(),
    ] {
        let (_, diagnostics) = lex(&source);
        assert_eq!(diagnostics.len(), 1, "source: {source}");
        let diagnostic = &diagnostics[0];
        let span = diagnostic.span.expect("lexical diagnostics have spans");
        assert_eq!(span.start, 0, "source: {source}");
        assert_eq!(span.end as usize, source.len(), "source: {source}");
        assert_eq!(
            diagnostic.message,
            format!("integer literal magnitude `{source}` is out of range for u64")
        );
    }
}

#[test]
fn rejects_missing_digits_invalid_digits_and_bad_separators() {
    for (source, span, message) in [
        (
            "0b",
            (0, 2),
            "binary integer prefix must be followed by a valid digit",
        ),
        (
            "0Xg",
            (0, 3),
            "hexadecimal integer prefix must be followed by a valid digit",
        ),
        (
            "0b102",
            (4, 5),
            "invalid digit in base-2 integer literal `0b102`",
        ),
        (
            "0x_1",
            (2, 4),
            "integer separators must appear between two valid digits",
        ),
        (
            "1__2",
            (1, 4),
            "integer separators must appear between two valid digits",
        ),
        (
            "1_u",
            (1, 3),
            "integer separators must appear between two valid digits",
        ),
        (
            "1_",
            (1, 2),
            "integer separators must appear between two valid digits",
        ),
    ] {
        let (_, diagnostics) = lex(source);
        assert_eq!(diagnostics.len(), 1, "source: {source}");
        let diagnostic = &diagnostics[0];
        let actual = diagnostic.span.expect("lexical diagnostics have spans");
        assert_eq!((actual.start, actual.end), span, "source: {source}");
        assert_eq!(diagnostic.message, message, "source: {source}");
    }
}

#[test]
fn rejects_long_before_unsigned_and_unknown_suffixes_as_one_lexeme() {
    for suffix in ["lu", "lU", "Lu", "LU", "uu", "ll", "ulx", "word"] {
        let source = format!("1{suffix}");
        let (tokens, diagnostics) = lex(&source);
        assert_eq!(diagnostics.len(), 1, "source: {source}");
        assert_eq!(tokens.len(), 2, "bad suffix retains one recovery token");
        assert!(matches!(tokens[0].kind, TokenKind::Error));
        assert!(matches!(tokens[1].kind, TokenKind::Eof));
        let diagnostic = &diagnostics[0];
        let span = diagnostic.span.expect("lexical diagnostics have spans");
        assert_eq!((span.start, span.end as usize), (1, source.len()));
        assert_eq!(
            diagnostic.message,
            format!("invalid integer literal suffix `{suffix}`")
        );
    }
}

#[test]
fn malformed_integer_recovery_preserves_the_next_declaration_boundary() {
    for malformed in ["18446744073709551616uL", "0b102", "1__2", "1lu"] {
        let source = format!("val broken: ULong = {malformed}\nfun main() {{}}\n");
        let diagnostics =
            crate::parse(&source).expect_err("the malformed literal must reject the source");
        assert_eq!(
            diagnostics.len(),
            1,
            "source `{malformed}`: {diagnostics:?}"
        );
    }
}

#[test]
fn lexer_recovers_multiple_malformed_integer_lexemes_in_source_order() {
    let (_, diagnostics) = lex("0x 1__2 4lu");
    assert_eq!(diagnostics.len(), 3);
    let spans: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.span.expect("spanned"))
        .collect();
    assert_eq!(
        spans,
        [
            scoop_ast::Span::new(0, 2),
            scoop_ast::Span::new(4, 7),
            scoop_ast::Span::new(9, 11),
        ]
    );
}

#[test]
fn integer_syntax_is_preserved_in_expression_bodies() {
    let file = ok("fun answer(): ULong = 0xffff_ffff_ffff_ffffUL");
    let Decl::Function(function) = &file.declarations[0] else {
        panic!("expected a function");
    };
    let FunctionBody::Expr(body) = &function.body else {
        panic!("expected an expression body");
    };
    let Expr::IntLiteral(literal) = &**body else {
        panic!("expected an integer literal");
    };
    assert_eq!(literal.magnitude, u64::MAX);
    assert_eq!(literal.radix, IntegerRadix::Hexadecimal);
    assert_eq!(literal.suffix, IntegerSuffix::UnsignedLong);
}
