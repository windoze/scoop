use std::borrow::Cow;

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
