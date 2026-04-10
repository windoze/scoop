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

/// Parse a lexer-validated floating-point literal.
///
/// Supported forms:
/// - decimal fraction: `3.14`, `1_000.5`
/// - scientific notation: `1e3`, `2.5E-4`, `1_2.3_4e5_6`
/// - Float32 suffix: `0.5f`, `1.0f32`
pub fn parse_float_literal(text: &str) -> ParsedFloatLiteral {
    let (body, suffix) = split_float_suffix(text);
    let normalized: String = body.chars().filter(|&ch| ch != '_').collect();
    let value = normalized.parse::<f64>().unwrap_or_else(|err| {
        panic!(
            "lexer validated Float literal before parse_float_literal: text={text:?}, normalized={normalized:?}, err={err:?}"
        )
    });
    ParsedFloatLiteral { value, suffix }
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

#[cfg(test)]
mod tests {
    use super::{FloatLiteralSuffix, ParsedFloatLiteral, parse_float_literal};

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
}
