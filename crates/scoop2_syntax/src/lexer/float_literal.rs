//! Float 字面量解析（自 `scoopc_ast::syntax::float_literal` 移植）。

/// 解析后的 Float 字面量元信息。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParsedFloatLiteral {
    pub value: f64,
    pub suffix: FloatLiteralSuffix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatLiteralSuffix {
    Float64,
    Float32,
}

/// Float 字面量解析错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatLiteralParseError {
    InvalidFormat,
    InvalidSeparator,
    MissingExponentDigits,
    Overflow(FloatLiteralSuffix),
}

impl FloatLiteralParseError {
    pub fn reason(self) -> &'static str {
        match self {
            FloatLiteralParseError::InvalidFormat => "格式无效",
            FloatLiteralParseError::InvalidSeparator => "下划线只能出现在数字之间",
            FloatLiteralParseError::MissingExponentDigits => "指数部分缺少数字",
            FloatLiteralParseError::Overflow(FloatLiteralSuffix::Float64) => {
                "超出 Float64 可表示范围"
            }
            FloatLiteralParseError::Overflow(FloatLiteralSuffix::Float32) => {
                "超出 Float32 可表示范围"
            }
        }
    }
}

/// Parse a lexer-validated floating-point literal.
///
/// Supported forms:
/// - decimal fraction: `3.14`, `1_000.5`
/// - scientific notation: `1e3`, `2.5E-4`, `1_2.3_4e5_6`
/// - Float32 suffix: `0.5f`, `1.0f32`
pub fn parse_float_literal(text: &str) -> ParsedFloatLiteral {
    parse_float_literal_checked(text).unwrap_or_else(|err| {
        panic!("validated float literal should parse: text={text:?}, err={err:?}")
    })
}

/// 严格解析 Float 字面量，并在 lexer / 单测中为非法文本提供稳定错误。
pub fn parse_float_literal_checked(
    text: &str,
) -> Result<ParsedFloatLiteral, FloatLiteralParseError> {
    let (body, suffix) = split_float_suffix(text);
    validate_float_body(body)?;

    let normalized: String = body.chars().filter(|&ch| ch != '_').collect();
    let value = normalized
        .parse::<f64>()
        .map_err(|_| FloatLiteralParseError::InvalidFormat)?;
    if !value.is_finite() {
        return Err(FloatLiteralParseError::Overflow(suffix));
    }
    if suffix == FloatLiteralSuffix::Float32 && !(value as f32).is_finite() {
        return Err(FloatLiteralParseError::Overflow(
            FloatLiteralSuffix::Float32,
        ));
    }

    Ok(ParsedFloatLiteral { value, suffix })
}

fn split_float_suffix(text: &str) -> (&str, FloatLiteralSuffix) {
    if let Some(rest) = text.strip_suffix("f32") {
        (rest, FloatLiteralSuffix::Float32)
    } else if let Some(rest) = text.strip_suffix('f') {
        (rest, FloatLiteralSuffix::Float32)
    } else {
        (text, FloatLiteralSuffix::Float64)
    }
}

fn validate_float_body(text: &str) -> Result<(), FloatLiteralParseError> {
    let bytes = text.as_bytes();
    let mut idx = 0;

    consume_decimal_digits(text, bytes, &mut idx)?;

    let mut saw_fraction = false;
    if idx < bytes.len() && bytes[idx] == b'.' {
        saw_fraction = true;
        idx += 1;
        consume_decimal_digits(text, bytes, &mut idx)?;
    }

    let mut saw_exponent = false;
    if idx < bytes.len() && matches!(bytes[idx], b'e' | b'E') {
        saw_exponent = true;
        idx += 1;
        if idx < bytes.len() && matches!(bytes[idx], b'+' | b'-') {
            idx += 1;
        }
        if idx >= bytes.len() {
            return Err(FloatLiteralParseError::MissingExponentDigits);
        }
        consume_decimal_digits(text, bytes, &mut idx).map_err(|err| match err {
            FloatLiteralParseError::InvalidSeparator => FloatLiteralParseError::InvalidSeparator,
            _ => FloatLiteralParseError::MissingExponentDigits,
        })?;
    }

    if !saw_fraction && !saw_exponent {
        return Err(FloatLiteralParseError::InvalidFormat);
    }
    if idx != bytes.len() {
        return Err(FloatLiteralParseError::InvalidFormat);
    }

    Ok(())
}

fn consume_decimal_digits(
    text: &str,
    bytes: &[u8],
    idx: &mut usize,
) -> Result<(), FloatLiteralParseError> {
    if *idx >= bytes.len() {
        return Err(FloatLiteralParseError::InvalidFormat);
    }
    if bytes[*idx] == b'_' {
        return Err(FloatLiteralParseError::InvalidSeparator);
    }
    if !char::from(bytes[*idx]).is_ascii_digit() {
        return Err(FloatLiteralParseError::InvalidFormat);
    }

    let mut prev_was_underscore = false;
    while *idx < bytes.len() {
        let ch = char::from(bytes[*idx]);
        if ch == '_' {
            if prev_was_underscore {
                return Err(FloatLiteralParseError::InvalidSeparator);
            }
            prev_was_underscore = true;
            *idx += 1;
            continue;
        }
        if ch.is_ascii_digit() {
            prev_was_underscore = false;
            *idx += 1;
            continue;
        }
        break;
    }

    if prev_was_underscore {
        return Err(FloatLiteralParseError::InvalidSeparator);
    }
    if text.is_empty() {
        return Err(FloatLiteralParseError::InvalidFormat);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        FloatLiteralParseError, FloatLiteralSuffix, ParsedFloatLiteral, parse_float_literal,
        parse_float_literal_checked,
    };

    #[test]
    fn parse_decimal_float_literal() {
        assert_eq!(
            parse_float_literal("2.75"),
            ParsedFloatLiteral {
                value: 2.75,
                suffix: FloatLiteralSuffix::Float64,
            }
        );
    }

    #[test]
    fn parse_scientific_float_literal_with_separators() {
        assert_eq!(
            parse_float_literal("1_2.5e+2"),
            ParsedFloatLiteral {
                value: 1250.0,
                suffix: FloatLiteralSuffix::Float64,
            }
        );
        assert_eq!(
            parse_float_literal("1e1_0"),
            ParsedFloatLiteral {
                value: 1e10,
                suffix: FloatLiteralSuffix::Float64,
            }
        );
    }

    #[test]
    fn parse_float32_suffix_literal() {
        assert_eq!(
            parse_float_literal("0.5f"),
            ParsedFloatLiteral {
                value: 0.5,
                suffix: FloatLiteralSuffix::Float32,
            }
        );
        assert_eq!(
            parse_float_literal("1.0f32"),
            ParsedFloatLiteral {
                value: 1.0,
                suffix: FloatLiteralSuffix::Float32,
            }
        );
    }

    #[test]
    fn reject_invalid_float_format_and_separator() {
        assert_eq!(
            parse_float_literal_checked("1e"),
            Err(FloatLiteralParseError::MissingExponentDigits)
        );
        assert_eq!(
            parse_float_literal_checked("1e+"),
            Err(FloatLiteralParseError::MissingExponentDigits)
        );
        assert_eq!(
            parse_float_literal_checked("1._2"),
            Err(FloatLiteralParseError::InvalidSeparator)
        );
        assert_eq!(
            parse_float_literal_checked("1.0ff"),
            Err(FloatLiteralParseError::InvalidFormat)
        );
    }

    #[test]
    fn reject_float_overflow() {
        assert_eq!(
            parse_float_literal_checked("1e9999"),
            Err(FloatLiteralParseError::Overflow(
                FloatLiteralSuffix::Float64
            ))
        );
        assert_eq!(
            parse_float_literal_checked("1e39f"),
            Err(FloatLiteralParseError::Overflow(
                FloatLiteralSuffix::Float32
            ))
        );
    }
}
