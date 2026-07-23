//! 整数字面量解析（自 `scoopc_ast::syntax::int_literal` 移植）。

/// 整数字面量解析错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntLiteralParseError {
    MissingDigits,
    InvalidDigit,
    InvalidSeparator,
    Overflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntLiteralSuffix {
    None,
    UInt,
    Long,
    ULong,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntLiteralParts<'a> {
    pub digits: &'a str,
    pub suffix: IntLiteralSuffix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntLiteralTarget {
    pub bits: u32,
    pub signed: bool,
}

impl IntLiteralTarget {
    pub const fn new(bits: u32, signed: bool) -> Self {
        Self { bits, signed }
    }
}

impl IntLiteralParseError {
    pub fn reason(self) -> &'static str {
        match self {
            IntLiteralParseError::MissingDigits => "前缀后缺少数字",
            IntLiteralParseError::InvalidDigit => "包含无效数字",
            IntLiteralParseError::InvalidSeparator => "下划线只能出现在数字之间",
            IntLiteralParseError::Overflow => "超出编译器支持的整数字面量范围",
        }
    }
}

/// Parse an integer literal (possibly containing `_` separators) into a `u128`.
///
/// Supported forms:
/// - decimal: `123`, `1_000`
/// - hexadecimal: `0xFF`, `0Xca_fe`
/// - binary: `0b1010`, `0B10_01`
///
/// 调用方应只在“词法已验证”的文本上使用该入口；若需要面向用户的错误，请调用
/// [`parse_int_literal_checked`]。
pub fn parse_int_literal(text: &str) -> u128 {
    parse_int_literal_checked(text).unwrap_or_else(|err| {
        panic!("validated integer literal should parse: text={text:?}, err={err:?}")
    })
}

/// 严格解析整数字面量。
///
/// 与 `parse_int_literal` 不同，该入口会校验：
/// - 前缀后是否至少有一个数字；
/// - 下划线是否只出现在数字之间；
/// - 数字是否与 radix 匹配；
/// - 数值是否超出 `u128`。
pub fn parse_int_literal_checked(text: &str) -> Result<u128, IntLiteralParseError> {
    let parts = split_int_literal_suffix(text);
    let (radix, digits) = literal_radix_and_digits(parts.digits);
    if digits.is_empty() {
        return Err(IntLiteralParseError::MissingDigits);
    }

    let mut out: u128 = 0;
    let mut prev_was_underscore = false;
    let mut saw_digit = false;

    for ch in digits.chars() {
        if ch == '_' {
            if !saw_digit || prev_was_underscore {
                return Err(IntLiteralParseError::InvalidSeparator);
            }
            prev_was_underscore = true;
            continue;
        }

        let Some(digit) = ch.to_digit(radix) else {
            return Err(IntLiteralParseError::InvalidDigit);
        };
        out = out
            .checked_mul(u128::from(radix))
            .and_then(|value| value.checked_add(u128::from(digit)))
            .ok_or(IntLiteralParseError::Overflow)?;
        saw_digit = true;
        prev_was_underscore = false;
    }

    if prev_was_underscore {
        return Err(IntLiteralParseError::InvalidSeparator);
    }

    Ok(out)
}

pub fn parse_int_literal_suffix(text: &str) -> IntLiteralSuffix {
    split_int_literal_suffix(text).suffix
}

pub fn checked_positive_int_literal_bits(value: u128, target: IntLiteralTarget) -> Option<u128> {
    let max = if target.signed {
        signed_int_max(target.bits)
    } else {
        unsigned_int_max(target.bits)
    };
    (value <= max).then_some(value)
}

pub fn checked_negated_int_literal_bits(value: u128, target: IntLiteralTarget) -> Option<u128> {
    if !target.signed {
        return None;
    }
    let min_abs = signed_int_min_abs(target.bits);
    (value <= min_abs).then_some(mask_to_bits(0u128.wrapping_sub(value), target.bits))
}

fn split_int_literal_suffix(text: &str) -> IntLiteralParts<'_> {
    for (suffix_text, suffix) in [
        ("uL", IntLiteralSuffix::ULong),
        ("UL", IntLiteralSuffix::ULong),
        ("ul", IntLiteralSuffix::ULong),
        ("Ul", IntLiteralSuffix::ULong),
        ("u", IntLiteralSuffix::UInt),
        ("U", IntLiteralSuffix::UInt),
        ("L", IntLiteralSuffix::Long),
        ("l", IntLiteralSuffix::Long),
    ] {
        if let Some(digits) = text.strip_suffix(suffix_text) {
            return IntLiteralParts { digits, suffix };
        }
    }

    IntLiteralParts {
        digits: text,
        suffix: IntLiteralSuffix::None,
    }
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

fn mask_to_bits(value: u128, bits: u32) -> u128 {
    if bits == 0 {
        return 0;
    }
    if bits >= 128 {
        return value;
    }
    value & ((1u128 << bits) - 1)
}

fn unsigned_int_max(bits: u32) -> u128 {
    if bits == 0 {
        return 0;
    }
    if bits >= 128 {
        return u128::MAX;
    }
    (1u128 << bits) - 1
}

fn signed_int_max(bits: u32) -> u128 {
    if bits <= 1 {
        return 0;
    }
    if bits >= 128 {
        return i128::MAX as u128;
    }
    (1u128 << (bits - 1)) - 1
}

fn signed_int_min_abs(bits: u32) -> u128 {
    if bits == 0 {
        return 0;
    }
    if bits >= 128 {
        return 1u128 << 127;
    }
    1u128 << (bits - 1)
}

#[cfg(test)]
mod tests {
    use super::{
        IntLiteralParseError, IntLiteralSuffix, parse_int_literal, parse_int_literal_checked,
        parse_int_literal_suffix,
    };

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

    #[test]
    fn parse_integer_suffixes() {
        assert_eq!(parse_int_literal("42L"), 42);
        assert_eq!(parse_int_literal_suffix("42L"), IntLiteralSuffix::Long);
        assert_eq!(parse_int_literal("42u"), 42);
        assert_eq!(parse_int_literal_suffix("42u"), IntLiteralSuffix::UInt);
        assert_eq!(parse_int_literal("42UL"), 42);
        assert_eq!(parse_int_literal_suffix("42UL"), IntLiteralSuffix::ULong);
        assert_eq!(parse_int_literal("0xFFu"), 255);
    }

    #[test]
    fn reject_invalid_separator_and_digit() {
        assert_eq!(
            parse_int_literal_checked("1__2"),
            Err(IntLiteralParseError::InvalidSeparator)
        );
        assert_eq!(
            parse_int_literal_checked("0x_1"),
            Err(IntLiteralParseError::InvalidSeparator)
        );
        assert_eq!(
            parse_int_literal_checked("0b102"),
            Err(IntLiteralParseError::InvalidDigit)
        );
    }

    #[test]
    fn reject_missing_digits_and_overflow() {
        assert_eq!(
            parse_int_literal_checked("0x"),
            Err(IntLiteralParseError::MissingDigits)
        );
        assert_eq!(
            parse_int_literal_checked("0b"),
            Err(IntLiteralParseError::MissingDigits)
        );
        assert_eq!(
            parse_int_literal_checked("340282366920938463463374607431768211456"),
            Err(IntLiteralParseError::Overflow)
        );
    }

    #[test]
    fn checks_target_integer_ranges() {
        let int8 = super::IntLiteralTarget::new(8, true);
        let uint8 = super::IntLiteralTarget::new(8, false);

        assert_eq!(
            super::checked_positive_int_literal_bits(127, int8),
            Some(127)
        );
        assert_eq!(super::checked_positive_int_literal_bits(128, int8), None);
        assert_eq!(
            super::checked_negated_int_literal_bits(128, int8),
            Some(128)
        );
        assert_eq!(super::checked_negated_int_literal_bits(129, int8), None);
        assert_eq!(
            super::checked_positive_int_literal_bits(255, uint8),
            Some(255)
        );
        assert_eq!(super::checked_positive_int_literal_bits(256, uint8), None);
        assert_eq!(super::checked_negated_int_literal_bits(1, uint8), None);
    }
}
