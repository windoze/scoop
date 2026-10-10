use super::*;

// --- when diagnostics -----------------------------------------------------------

#[test]
fn when_rest_twice_in_positional_pattern() {
    let (span, message) =
        err("fun main() {\n    when (s) {\n        case V(a, .., ..) -> { }\n    }\n}\n");
    assert_eq!(span, Span::new(50, 52));
    assert_eq!(message, "`..` may appear at most once in a pattern");
}

#[test]
fn when_rest_twice_in_tuple_pattern() {
    let (span, message) =
        err("fun main() {\n    when (s) {\n        case (a, .., b, ..) -> { }\n    }\n}\n");
    assert_eq!(span, Span::new(52, 54));
    assert_eq!(message, "`..` may appear at most once in a pattern");
}

#[test]
fn when_rest_must_be_last_in_field_pattern() {
    let (span, message) =
        err("fun main() {\n    when (s) {\n        case S { .., x } -> { }\n    }\n}\n");
    assert_eq!(span, Span::new(49, 50));
    assert_eq!(message, "`..` must be the last element in a field pattern");
}

#[test]
fn when_rest_twice_in_field_pattern() {
    let (_, message) =
        err("fun main() {\n    when (s) {\n        case S { .., .. } -> { }\n    }\n}\n");
    assert_eq!(message, "`..` must be the last element in a field pattern");
}

#[test]
fn when_wildcard_is_not_a_field_name() {
    let (span, message) =
        err("fun main() {\n    when (s) {\n        case S { _ } -> { }\n    }\n}\n");
    assert_eq!(span, Span::new(45, 46));
    assert_eq!(message, "`_` is not allowed in a field pattern");
}

#[test]
fn malformed_recursive_field_rhs_recovers_to_later_declarations() {
    let source = "fun first() {\n    val { child: } = value\n}\n\
                  fun second() {\n    when (value) {\n        case S { child: } -> {}\n    }\n}\n\
                  fun main() {}\n";
    let diagnostics = crate::parse(source).expect_err("both malformed subpatterns must fail");
    assert_eq!(diagnostics.len(), 2);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.message == "expected pattern, found `}`")
    );
    let expected_starts = source
        .match_indices("child: }")
        .map(|(start, _)| u32::try_from(start + "child: ".len()).expect("test source fits u32"))
        .collect::<Vec<_>>();
    let actual_starts = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.span.expect("parser diagnostic span").start)
        .collect::<Vec<_>>();
    assert_eq!(actual_starts, expected_starts);
}

#[test]
fn when_arm_needs_an_arrow() {
    let (span, message) = err("fun main() {\n    when (s) {\n        case Red\n    }\n}\n");
    assert_eq!(span, Span::new(49, 50));
    assert_eq!(message, "expected `->`, found `}`");
}

#[test]
fn when_else_must_be_the_last_arm() {
    let (span, message) =
        err("fun main() {\n    when (s) {\n        else -> { }\n        Red -> { }\n    }\n}\n");
    assert_eq!(span, Span::new(56, 59));
    assert_eq!(message, "expected `}`, found `Red`");
}

#[test]
fn when_guard_must_be_parenthesized() {
    let (span, message) =
        err("fun main() {\n    when (s) {\n        case Red if x -> { }\n    }\n}\n");
    assert_eq!(span, Span::new(48, 49));
    assert_eq!(message, "expected `(`, found `x`");
}

#[test]
fn when_bare_rest_is_not_a_pattern() {
    let (span, message) = err("fun main() {\n    when (s) {\n        case .. -> { }\n    }\n}\n");
    assert_eq!(span, Span::new(41, 43));
    assert_eq!(message, "expected pattern, found `..`");
}

#[test]
fn when_range_is_not_a_pattern() {
    // The `..` in the pattern position is the rest marker; a range
    // expression therefore cannot appear there (spec 4.6 disambiguation).
    let (span, message) = err("fun main() {\n    when (s) {\n        case 1..4 -> { }\n    }\n}\n");
    assert_eq!(span, Span::new(42, 44));
    assert_eq!(message, "expected `->`, found `..`");
}
