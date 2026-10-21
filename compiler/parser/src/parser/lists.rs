use scoop_ast::Diagnostic;
use scoop_syntax::{ExpectedSyntax, SyntaxKind};

use crate::lexer::TokenKind;

use super::{DelimiterBalance, Parser};

mod kind;
pub(crate) use kind::ListKind;

pub(crate) struct DelimitedItems<T> {
    pub(crate) items: Vec<T>,
    pub(crate) end: u32,
    pub(crate) had_comma: bool,
}

impl<T> DelimitedItems<T> {
    pub(crate) fn into_parts(self) -> (Vec<T>, u32) {
        (self.items, self.end)
    }
}

impl Parser {
    /// The caller consumed the opening delimiter. Each item uses its normal
    /// grammar; recovery retains the following sibling without inventing one.
    pub(crate) fn parse_delimited_items<T>(
        &mut self,
        kind: ListKind,
        mut item: impl FnMut(&mut Self) -> Result<T, Diagnostic>,
    ) -> DelimitedItems<T> {
        let close = kind.close();
        let mut items = Vec::new();
        let mut required = kind.requires_item();
        let mut had_comma = false;
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
                had_comma = true;
                if kind.allows_trailing_comma() && self.peek().kind == close {
                    break;
                }
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
                had_comma = true;
                if kind.allows_trailing_comma() && self.peek().kind == close {
                    break;
                }
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
        DelimitedItems {
            items,
            end,
            had_comma,
        }
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
                    && (!kind.tracks_angles() || angles == 0)
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
