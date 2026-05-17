//! 字符串字面量解析（early stage）。
//!
//! 说明：
//! - lexer/parser 当前阶段只保留字符串字面量的 span（不做解码）；
//! - 但后端/工具链在一些场景需要得到解码后的字节序列，例如：
//!   - LLVM codegen 的字符串常量数据落盘；
//!   - `@Extern("symbol")` 这类需要从字符串字面量提取符号名的场景。
//! - 因此把“最小可用”的字符串字面量解析逻辑放到 `syntax` 层，避免在后端重复实现。

/// 字符串字面量解析错误（最小集合）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringLiteralParseError {
    /// 字面量格式非法（引号不匹配/转义不完整/Unicode 码点非法等）。
    Invalid,
    /// `f"..."` / `f"""..."""` 插值字符串：当前阶段不在此处解码。
    Interpolated,
    /// 解码后的字节不是有效 UTF-8（仅用于 `*_utf8` helper）。
    InvalidUtf8,
}

/// 解析一个 Scoop 字符串字面量（含引号）为 UTF-8 字节序列。
///
/// 支持：
/// - 普通字符串：`"..."`（支持最小转义）
/// - raw 三引号字符串：`""" ... """`（不处理转义）
///
/// 不支持：
/// - f-string（插值字符串）：`f"..."` / `f"""..."""`（返回 `Interpolated`）
pub fn parse_string_literal_bytes(text: &str) -> Result<Vec<u8>, StringLiteralParseError> {
    // f-string：留给 lowering（例如 T0823）；这里避免误把它当作普通字符串。
    if text.starts_with("f\"") || text.starts_with("f\"\"\"") {
        return Err(StringLiteralParseError::Interpolated);
    }

    // raw string：""" ... """
    if let Some(rest) = text.strip_prefix("\"\"\"") {
        let inner = rest
            .strip_suffix("\"\"\"")
            .ok_or(StringLiteralParseError::Invalid)?;
        return Ok(inner.as_bytes().to_vec());
    }

    // normal string：" ... "（支持最小转义）
    let inner = text
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .ok_or(StringLiteralParseError::Invalid)?;

    parse_normal_string_bytes(inner)
}

/// 解析 parser 拆分后的 f-string 文本片段（不含外层引号）。
///
/// 支持：
/// - `{{` / `}}` 消解为字面量大括号；
/// - non-raw f-string 的普通字符串转义；
/// - raw f-string 保留反斜杠，仅消解双大括号。
pub fn parse_f_string_text_bytes(
    raw: bool,
    text: &str,
) -> Result<Vec<u8>, StringLiteralParseError> {
    if raw {
        let undoubled = undouble_braces(text);
        return Ok(undoubled.into_bytes());
    }

    // 非 raw：先在源码层消解双大括号，并避免把 `\u{...}` 的 `{}` 当作候选；
    // 再复用普通字符串的转义解析。
    let undoubled = undouble_braces_preserving_escapes(text);
    parse_normal_string_bytes(&undoubled)
}

/// 解析 f-string 文本片段并要求其内容为有效 UTF-8。
pub fn parse_f_string_text_utf8(raw: bool, text: &str) -> Result<String, StringLiteralParseError> {
    let bytes = parse_f_string_text_bytes(raw, text)?;
    String::from_utf8(bytes).map_err(|_| StringLiteralParseError::InvalidUtf8)
}

fn undouble_braces(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '{' if matches!(chars.peek(), Some('{')) => {
                let _ = chars.next();
                out.push('{');
            }
            '}' if matches!(chars.peek(), Some('}')) => {
                let _ = chars.next();
                out.push('}');
            }
            _ => out.push(ch),
        }
    }
    out
}

fn undouble_braces_preserving_escapes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            // 转义序列中的 `{`/`}` 不参与 `{{`/`}}` 的消解。
            out.push('\\');
            let Some(next) = chars.next() else {
                break;
            };
            out.push(next);

            // `\u{...}`：把整个 `{...}` 视为转义语法的一部分，原样拷贝。
            if next == 'u' && matches!(chars.peek(), Some('{')) {
                out.push(chars.next().expect("peek 已保证存在"));
                for c in chars.by_ref() {
                    out.push(c);
                    if c == '}' {
                        break;
                    }
                }
            }
            continue;
        }

        match ch {
            '{' if matches!(chars.peek(), Some('{')) => {
                let _ = chars.next();
                out.push('{');
            }
            '}' if matches!(chars.peek(), Some('}')) => {
                let _ = chars.next();
                out.push('}');
            }
            _ => out.push(ch),
        }
    }

    out
}

/// 解析普通字符串内容（不包含首尾 `"`）为 UTF-8 字节序列。
///
/// 说明：
/// - 该函数会解析最小转义集合（`\\` `\"` `\n` `\r` `\t` `\0` `\u{...}`）；
/// - 对未知转义采用“保守但可用”的策略：把 `\x` 解析为字面量字符 `x`，
///   以便 early stage 在不阻塞其它功能的前提下跑通更多 fixtures。
pub(crate) fn parse_normal_string_bytes(inner: &str) -> Result<Vec<u8>, StringLiteralParseError> {
    let mut out: Vec<u8> = Vec::with_capacity(inner.len());
    let mut chars = inner.chars();

    while let Some(ch) = chars.next() {
        if ch != '\\' {
            let mut buf = [0u8; 4];
            let s = ch.encode_utf8(&mut buf);
            out.extend_from_slice(s.as_bytes());
            continue;
        }

        let Some(esc) = chars.next() else {
            return Err(StringLiteralParseError::Invalid);
        };

        match esc {
            '\\' => out.push(b'\\'),
            '"' => out.push(b'"'),
            'n' => out.push(b'\n'),
            'r' => out.push(b'\r'),
            't' => out.push(b'\t'),
            '0' => out.push(b'\0'),
            // `\u{...}`（Kotlin-like，early stage）
            'u' => {
                let Some('{') = chars.next() else {
                    return Err(StringLiteralParseError::Invalid);
                };

                let mut hex = String::new();
                let mut closed = false;
                for ch in chars.by_ref() {
                    if ch == '}' {
                        closed = true;
                        break;
                    }
                    hex.push(ch);
                    if hex.len() > 6 {
                        return Err(StringLiteralParseError::Invalid);
                    }
                }

                if !closed || hex.is_empty() {
                    return Err(StringLiteralParseError::Invalid);
                }

                let code =
                    u32::from_str_radix(&hex, 16).map_err(|_| StringLiteralParseError::Invalid)?;
                let Some(ch) = char::from_u32(code) else {
                    return Err(StringLiteralParseError::Invalid);
                };

                let mut buf = [0u8; 4];
                let s = ch.encode_utf8(&mut buf);
                out.extend_from_slice(s.as_bytes());
            }
            // fallback：保守策略——把未知转义当作“转义后字符本身”（便于 early stage 跑通）。
            other => {
                let mut buf = [0u8; 4];
                let s = other.encode_utf8(&mut buf);
                out.extend_from_slice(s.as_bytes());
            }
        }
    }

    Ok(out)
}

/// 解析字符串字面量并要求其内容为有效 UTF-8。
pub fn parse_string_literal_utf8(text: &str) -> Result<String, StringLiteralParseError> {
    let bytes = parse_string_literal_bytes(text)?;
    String::from_utf8(bytes).map_err(|_| StringLiteralParseError::InvalidUtf8)
}
