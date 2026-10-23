use std::borrow::Cow;

/// Read exactly one source identifier using the ordinary lexer.
pub fn identifier_name(spelling: &str) -> Option<String> {
    scoop_source::source_span(spelling.len()).ok()?;
    let lexed = crate::lexer::lex(spelling);
    let [token, end] = lexed.tokens.as_slice() else {
        return None;
    };
    if !lexed.diagnostics.is_empty()
        || token.span.start != 0
        || token.span.end as usize != spelling.len()
        || !matches!(end.kind, crate::lexer::TokenKind::Eof)
    {
        return None;
    }
    match &token.kind {
        crate::lexer::TokenKind::Ident(name) | crate::lexer::TokenKind::EscapedIdent(name)
            if !name.is_empty() =>
        {
            Some(name.clone())
        }
        _ => None,
    }
}

/// Spell an existing semantic identifier using the ordinary lexer rules.
pub fn identifier_spelling(name: &str) -> Cow<'_, str> {
    let lexed = crate::lexer::lex(name);
    if lexed.diagnostics.is_empty()
        && matches!(lexed.tokens.as_slice(), [token, end]
            if matches!(&token.kind, crate::lexer::TokenKind::Ident(text) if text == name)
                && matches!(end.kind, crate::lexer::TokenKind::Eof))
    {
        Cow::Borrowed(name)
    } else {
        Cow::Owned(format!("`{name}`"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifier_names_require_one_complete_identifier_token() {
        for (spelling, name) in [
            ("名字", "名字"),
            ("open", "open"),
            ("`fun`", "fun"),
            ("`has space`", "has space"),
        ] {
            assert_eq!(identifier_name(spelling).as_deref(), Some(name));
        }
        for spelling in [
            "",
            "``",
            "fun",
            " name",
            "name ",
            "a.b",
            "name()",
            "`unterminated",
            "a/* comment */",
        ] {
            assert_eq!(identifier_name(spelling), None, "{spelling}");
        }
    }

    #[test]
    fn spelling_preserves_unicode_contextual_names_and_escapes_hard_keywords() {
        for (name, expected) in [
            ("方法", "方法"),
            ("open", "open"),
            ("fun", "`fun`"),
            ("has space", "`has space`"),
        ] {
            assert_eq!(identifier_spelling(name), expected);
        }
    }
}
