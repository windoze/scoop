//! Char 字面量解析（自 `scoopc_ast::syntax::char_literal` 移植）。
//!
//! 说明：
//! - Char 字面量在前端阶段就需要做严格校验，以便把空字面量、多字符、非法转义、
//!   非法 Unicode 码点等错误尽早定位到词法阶段；
//! - 这里的 helper 负责解析“完整字面量文本（含单引号）”为单个 Unicode scalar value。

/// Char 字面量解析错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharLiteralParseError {
    InvalidFormat,
    Empty,
    TooManyChars,
    InvalidEscape,
    InvalidUnicode,
}

impl CharLiteralParseError {
    pub fn reason(self) -> &'static str {
        match self {
            CharLiteralParseError::InvalidFormat => "引号或格式无效",
            CharLiteralParseError::Empty => "不能为空",
            CharLiteralParseError::TooManyChars => "必须恰好包含一个字符",
            CharLiteralParseError::InvalidEscape => "包含无效转义",
            CharLiteralParseError::InvalidUnicode => "包含无效 Unicode 码点",
        }
    }
}

/// 解析一个完整的 Char 字面量（例如 `'a'`、`'\n'`、`'\u0041'`）。
pub fn parse_char_literal(text: &str) -> Result<char, CharLiteralParseError> {
    let inner = text
        .strip_prefix('\'')
        .and_then(|s| s.strip_suffix('\''))
        .ok_or(CharLiteralParseError::InvalidFormat)?;

    if inner.is_empty() {
        return Err(CharLiteralParseError::Empty);
    }

    let mut chars = inner.chars();
    let ch = match chars.next() {
        Some('\\') => parse_escape(&mut chars)?,
        Some(ch) => ch,
        None => return Err(CharLiteralParseError::Empty),
    };

    if chars.next().is_some() {
        return Err(CharLiteralParseError::TooManyChars);
    }

    Ok(ch)
}

fn parse_escape(chars: &mut std::str::Chars<'_>) -> Result<char, CharLiteralParseError> {
    let Some(esc) = chars.next() else {
        return Err(CharLiteralParseError::InvalidEscape);
    };

    match esc {
        'n' => Ok('\n'),
        't' => Ok('\t'),
        'r' => Ok('\r'),
        '\\' => Ok('\\'),
        '\'' => Ok('\''),
        '0' => Ok('\0'),
        'u' => parse_unicode_escape(chars),
        _ => Err(CharLiteralParseError::InvalidEscape),
    }
}

fn parse_unicode_escape(chars: &mut std::str::Chars<'_>) -> Result<char, CharLiteralParseError> {
    let mut hex = String::with_capacity(4);
    for _ in 0..4 {
        let Some(ch) = chars.next() else {
            return Err(CharLiteralParseError::InvalidUnicode);
        };
        if !ch.is_ascii_hexdigit() {
            return Err(CharLiteralParseError::InvalidUnicode);
        }
        hex.push(ch);
    }

    let code = u32::from_str_radix(&hex, 16).map_err(|_| CharLiteralParseError::InvalidUnicode)?;
    char::from_u32(code).ok_or(CharLiteralParseError::InvalidUnicode)
}

#[cfg(test)]
mod tests {
    use super::{CharLiteralParseError, parse_char_literal};

    #[test]
    fn parse_plain_char_literals() {
        assert_eq!(parse_char_literal("'a'"), Ok('a'));
        assert_eq!(parse_char_literal("'中'"), Ok('中'));
    }

    #[test]
    fn parse_escaped_char_literals() {
        assert_eq!(parse_char_literal(r"'\n'"), Ok('\n'));
        assert_eq!(parse_char_literal(r"'\t'"), Ok('\t'));
        assert_eq!(parse_char_literal(r"'\\'"), Ok('\\'));
        assert_eq!(parse_char_literal(r"'\''"), Ok('\''));
        assert_eq!(parse_char_literal(r"'\0'"), Ok('\0'));
    }

    #[test]
    fn parse_unicode_escape() {
        assert_eq!(parse_char_literal(r"'\u0041'"), Ok('A'));
        assert_eq!(parse_char_literal(r"'\u4E2D'"), Ok('中'));
    }

    #[test]
    fn reject_empty_and_multi_char_literals() {
        assert_eq!(parse_char_literal("''"), Err(CharLiteralParseError::Empty));
        assert_eq!(
            parse_char_literal("'ab'"),
            Err(CharLiteralParseError::TooManyChars)
        );
    }

    #[test]
    fn reject_invalid_escape_and_unicode() {
        assert_eq!(
            parse_char_literal(r"'\q'"),
            Err(CharLiteralParseError::InvalidEscape)
        );
        assert_eq!(
            parse_char_literal(r"'\uD800'"),
            Err(CharLiteralParseError::InvalidUnicode)
        );
    }
}
