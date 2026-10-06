//! Floating syntax retains decimal precision until an exact type is selected.

use scoop_ast::{Expr, FloatSuffix};

use crate::lexer::{TokenKind, lex};
use crate::tests_m2::init_expr;

#[test]
fn retains_unrounded_decimal_suffix_and_span() {
    for (source, decimal, suffix) in [
        ("1f", "1", FloatSuffix::Float),
        (".5F", ".5", FloatSuffix::Float),
        ("1_234.5_6e-7_8", "1234.56e-78", FloatSuffix::None),
        (
            "16777217.000000001",
            "16777217.000000001",
            FloatSuffix::None,
        ),
        ("1E+2", "1E+2", FloatSuffix::None),
    ] {
        let Expr::FloatLiteral(literal) = init_expr(source) else {
            panic!("expected a float literal for {source}");
        };
        assert_eq!(literal.decimal, decimal);
        assert_eq!(literal.suffix, suffix);
        assert_eq!(literal.span.end - literal.span.start, source.len() as u32);
    }
}

#[test]
fn rejects_malformed_fractions_exponents_and_suffixes() {
    for source in ["1.0e", "1e+", "1e_2", "1.0_", "1__0.0", "1.0d", ".5ff"] {
        let (_, diagnostics) = lex(source);
        assert!(!diagnostics.is_empty(), "expected rejection for {source}");
    }
}

#[test]
fn keeps_integer_ranges_radices_and_member_access_separate() {
    for source in ["1..2", "1..<2", "1.toString()", "0xff", "0b101"] {
        let (tokens, diagnostics) = lex(source);
        assert!(diagnostics.is_empty(), "{source}: {diagnostics:?}");
        assert!(matches!(tokens[0].kind, TokenKind::Int(_)));
        assert!(
            !tokens
                .iter()
                .any(|token| matches!(token.kind, TokenKind::Float(_)))
        );
    }
}
