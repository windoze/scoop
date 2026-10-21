use scoop_ast::Diagnostic;
use scoop_syntax::{ExpectedSyntax, SyntaxKind};

use crate::lexer::TokenKind;

use super::{DelimiterBalance, Parser};

#[derive(Debug, Clone, Copy)]
pub(crate) enum ListKind {
    FunctionParameters,
    CallArguments,
    TypeArguments,
    TypeParameters,
}

impl ListKind {
    fn close(self) -> TokenKind {
        match self {
            Self::FunctionParameters | Self::CallArguments => TokenKind::RParen,
            Self::TypeArguments | Self::TypeParameters => TokenKind::Greater,
        }
    }

    fn close_label(self) -> &'static str {
        match self {
            Self::FunctionParameters | Self::CallArguments => "`)`",
            Self::TypeArguments | Self::TypeParameters => "`>`",
        }
    }

    fn is_type_list(self) -> bool {
        matches!(self, Self::TypeArguments | Self::TypeParameters)
    }

    fn starts_item(self, parser: &Parser) -> bool {
        let token = &parser.peek().kind;
        if matches!(token, TokenKind::Ident(_) | TokenKind::EscapedIdent(_)) {
            return true;
        }
        match self {
            Self::FunctionParameters => matches!(token, TokenKind::At | TokenKind::Vararg),
            Self::TypeParameters => matches!(token, TokenKind::In),
            Self::TypeArguments => matches!(
                token,
                TokenKind::At
                    | TokenKind::LParen
                    | TokenKind::Suspend
                    | TokenKind::In
                    | TokenKind::Star
            ),
            Self::CallArguments => matches!(
                token,
                TokenKind::Str(_)
                    | TokenKind::Char(_)
                    | TokenKind::Int(_)
                    | TokenKind::Float(_)
                    | TokenKind::FStringStart
                    | TokenKind::LParen
                    | TokenKind::LBracket
                    | TokenKind::LBrace
                    | TokenKind::This
                    | TokenKind::True
                    | TokenKind::False
                    | TokenKind::Fun
                    | TokenKind::Suspend
                    | TokenKind::If
                    | TokenKind::When
                    | TokenKind::Return
                    | TokenKind::Break
                    | TokenKind::Continue
                    | TokenKind::Plus
                    | TokenKind::Minus
                    | TokenKind::Bang
                    | TokenKind::Star
                    | TokenKind::DoubleColon
            ),
        }
    }

    fn at_follow(self, parser: &Parser) -> bool {
        let token = &parser.peek().kind;
        if matches!(
            token,
            TokenKind::Eof | TokenKind::RBrace | TokenKind::Semicolon
        ) {
            return true;
        }
        match self {
            Self::FunctionParameters => {
                matches!(
                    token,
                    TokenKind::LBrace
                        | TokenKind::Equal
                        | TokenKind::Colon
                        | TokenKind::Fun
                        | TokenKind::Class
                        | TokenKind::Struct
                        | TokenKind::Interface
                ) || matches!(token, TokenKind::Ident(name) if name == "where")
            }
            Self::CallArguments => matches!(
                token,
                TokenKind::RBracket
                    | TokenKind::Val
                    | TokenKind::Var
                    | TokenKind::Class
                    | TokenKind::Struct
                    | TokenKind::Interface
            ),
            Self::TypeArguments => matches!(
                token,
                TokenKind::RParen
                    | TokenKind::RBracket
                    | TokenKind::Colon
                    | TokenKind::Equal
                    | TokenKind::Arrow
                    | TokenKind::LBrace
            ),
            Self::TypeParameters => {
                matches!(
                    token,
                    TokenKind::LParen
                        | TokenKind::RParen
                        | TokenKind::Colon
                        | TokenKind::Equal
                        | TokenKind::LBrace
                ) || matches!(token, TokenKind::Ident(_) | TokenKind::EscapedIdent(_))
                    && matches!(
                        parser.tokens.get(parser.pos + 1).map(|token| &token.kind),
                        Some(TokenKind::LParen)
                    )
            }
        }
    }

    fn at_recovery_follow(self, parser: &Parser) -> bool {
        self.at_follow(parser)
            && !(matches!(self, Self::FunctionParameters)
                && matches!(parser.peek().kind, TokenKind::Colon | TokenKind::Equal))
    }
}

impl Parser {
    /// The caller consumed the opening delimiter. Each item uses its normal
    /// grammar; recovery retains the following sibling without inventing one.
    pub(crate) fn parse_delimited_items<T>(
        &mut self,
        kind: ListKind,
        mut item: impl FnMut(&mut Self) -> Result<T, Diagnostic>,
    ) -> (Vec<T>, u32) {
        let close = kind.close();
        let mut items = Vec::new();
        let mut required = kind.is_type_list();
        loop {
            if !required && (self.peek().kind == close || kind.at_recovery_follow(self)) {
                break;
            }
            let start = self.pos;
            match item(self) {
                Ok(value) => items.push(value),
                Err(diagnostic) => {
                    self.diagnostics.push(diagnostic);
                    self.synchronize_list_item(kind, start);
                }
            }
            if matches!(self.peek().kind, TokenKind::Comma) {
                self.bump();
                required = true;
                continue;
            }
            if self.peek().kind == close || kind.at_follow(self) {
                break;
            }
            if kind.starts_item(self) {
                let diagnostic = self.missing_diagnostic(
                    ExpectedSyntax::Token(SyntaxKind::Comma),
                    "`,` between list items",
                );
                self.diagnostics.push(diagnostic);
                required = true;
                continue;
            }
            self.diagnostics.push(Diagnostic::at(
                self.peek().span,
                format!(
                    "expected {} after list item, found {}",
                    kind.close_label(),
                    self.peek().describe()
                ),
            ));
            self.synchronize_list_item(kind, self.pos);
            if matches!(self.peek().kind, TokenKind::Comma) {
                self.bump();
                required = true;
                continue;
            }
            break;
        }
        let end = if self.peek().kind == close {
            self.bump().span.end
        } else {
            let diagnostic = self.missing_diagnostic(
                ExpectedSyntax::Token(close.syntax_kind()),
                kind.close_label(),
            );
            self.diagnostics.push(diagnostic);
            self.peek().span.start
        };
        (items, end)
    }

    fn synchronize_list_item(&mut self, kind: ListKind, start: usize) {
        self.with_node(SyntaxKind::Error, |parser| {
            let mut balance = DelimiterBalance::default();
            let mut angles = 0usize;
            for token in &parser.tokens[start..parser.pos] {
                balance.observe(&token.kind);
                observe_angle(&token.kind, &mut angles);
            }
            while !parser.at_eof() {
                if balance.is_balanced()
                    && (!kind.is_type_list() || angles == 0)
                    && (matches!(parser.peek().kind, TokenKind::Comma)
                        || parser.peek().kind == kind.close()
                        || kind.at_recovery_follow(parser)
                        || parser.pos > start
                            && parser.peek().newline_before
                            && kind.starts_item(parser))
                {
                    return;
                }
                let token = parser.bump();
                balance.observe(&token.kind);
                observe_angle(&token.kind, &mut angles);
            }
        });
    }
}

fn observe_angle(token: &TokenKind, depth: &mut usize) {
    match token {
        TokenKind::Less => *depth += 1,
        TokenKind::Greater => *depth = depth.saturating_sub(1),
        _ => {}
    }
}
