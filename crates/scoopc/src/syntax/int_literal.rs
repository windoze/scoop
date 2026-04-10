/// Parse an integer literal (possibly containing `_` separators) into a `u128`.
///
/// Supported forms:
/// - decimal: `123`, `1_000`
/// - hexadecimal: `0xFF`, `0Xca_fe`
/// - binary: `0b1010`, `0B10_01`
///
/// The parser is intentionally minimal and saturating: callers are expected to run it only on
/// lexer-validated literal text.
pub fn parse_int_literal(text: &str) -> u128 {
    let (radix, digits) = literal_radix_and_digits(text);
    let mut out: u128 = 0;
    for ch in digits.chars() {
        if ch == '_' {
            continue;
        }
        if let Some(d) = ch.to_digit(radix) {
            out = out
                .saturating_mul(u128::from(radix))
                .saturating_add(u128::from(d));
        }
    }
    out
}

fn literal_radix_and_digits(text: &str) -> (u32, &str) {
    if let Some(rest) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        (16, rest)
    } else if let Some(rest) = text.strip_prefix("0b").or_else(|| text.strip_prefix("0B")) {
        (2, rest)
    } else {
        (10, text)
    }
}

#[cfg(test)]
mod tests {
    use super::parse_int_literal;

    #[test]
    fn parse_decimal_literal_with_separators() {
        assert_eq!(parse_int_literal("123_456"), 123_456);
    }

    #[test]
    fn parse_hex_literal() {
        assert_eq!(parse_int_literal("0xFF"), 255);
        assert_eq!(parse_int_literal("0Xca_fe"), 0xCAFE);
    }

    #[test]
    fn parse_binary_literal() {
        assert_eq!(parse_int_literal("0b1010"), 10);
        assert_eq!(parse_int_literal("0B10_01"), 9);
    }
}
