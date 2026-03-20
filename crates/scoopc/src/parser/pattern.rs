//! Pattern（模式）解析（最小子集）。
//!
//! 当前仅用于 `val` 解构绑定（T0244），因此只实现：
//! - `_` wildcard
//! - 标识符 bind
//! - tuple pattern：`(p1, p2, ...)`
//! - struct pattern：`TypeName { field, field: pat, ... }`

use crate::ast;
use crate::span::Span;
use crate::syntax::token::{Symbol, TokenKind};

use super::{ParseError, Parser};

impl<'a> Parser<'a> {
    pub(super) fn parse_pattern(&mut self) -> Result<ast::Pattern, ParseError> {
        self.parse_pattern_atom()
    }

    pub(super) fn looks_like_struct_pattern_ahead(&self) -> bool {
        if !self.peek_kind(TokenKind::Ident) {
            return false;
        }

        // 识别 `Ident(.Ident)* {` 的最小形式。
        let mut n = 1usize; // 已消费第一个 ident
        while self.peek_n(n).kind == TokenKind::Symbol(Symbol::Dot)
            && self.peek_n(n + 1).kind == TokenKind::Ident
        {
            n += 2;
        }

        self.peek_n(n).kind == TokenKind::Symbol(Symbol::LBrace)
    }

    fn parse_pattern_atom(&mut self) -> Result<ast::Pattern, ParseError> {
        if self.peek_symbol(Symbol::LParen) {
            return self.parse_tuple_pattern();
        }

        if self.peek_kind(TokenKind::Ident) {
            if self.looks_like_struct_pattern_ahead() {
                return self.parse_struct_pattern();
            }

            let tok = self.bump();
            let ident = ast::Ident { span: tok.span };
            if self.is_wildcard_ident(ident) {
                return Ok(ast::Pattern {
                    span: tok.span,
                    kind: ast::PatternKind::Wildcard,
                });
            }

            return Ok(ast::Pattern {
                span: tok.span,
                kind: ast::PatternKind::Bind(ident),
            });
        }

        let tok = *self.peek();
        Err(ParseError::Expected {
            expected: "pattern（`_` / 标识符 / tuple `(...)` / struct `Type { ... }`）",
            found: tok.kind,
            span: tok.span.into(),
        })
    }

    fn is_wildcard_ident(&self, ident: ast::Ident) -> bool {
        matches!(
            self.source_text.get(ident.span.start..ident.span.end),
            Some("_")
        )
    }

    fn parse_tuple_pattern(&mut self) -> Result<ast::Pattern, ParseError> {
        let open = self.expect_symbol(Symbol::LParen)?;
        let start = open.span.start;

        let mut elements = Vec::new();
        let mut rest_span: Option<Span> = None;
        if self.peek_symbol(Symbol::RParen) {
            let close = self.bump();
            return Ok(ast::Pattern {
                span: Span::new(start, close.span.end),
                kind: ast::PatternKind::Tuple(elements),
            });
        }

        loop {
            // `..` rest：仅允许出现一次，并且必须是最后一个元素。
            if rest_span.is_some() {
                let tok = *self.peek();
                if self.peek_symbol(Symbol::Dot)
                    && self.peek_n(1).kind == TokenKind::Symbol(Symbol::Dot)
                {
                    let err = ParseError::Expected {
                        expected: "tuple pattern：`..` 只能出现一次",
                        found: tok.kind,
                        span: tok.span.into(),
                    };
                    let _ = self.consume_balanced_after_open(Symbol::LParen, Symbol::RParen, start);
                    return Err(err);
                }
                let err = ParseError::Expected {
                    expected: "`)`（`..` 必须是最后一个元素）",
                    found: tok.kind,
                    span: tok.span.into(),
                };
                let _ = self.consume_balanced_after_open(Symbol::LParen, Symbol::RParen, start);
                return Err(err);
            }

            if self.peek_symbol(Symbol::Dot) && self.peek_n(1).kind == TokenKind::Symbol(Symbol::Dot)
            {
                let dot1 = self.bump();
                let dot2 = self.bump();
                let span = Span::new(dot1.span.start, dot2.span.end);
                rest_span = Some(span);
                elements.push(ast::Pattern {
                    span,
                    kind: ast::PatternKind::Rest,
                });
            } else {
                elements.push(self.parse_pattern()?);
            }

            if self.eat_symbol(Symbol::Comma) {
                // allow trailing comma
                if self.peek_symbol(Symbol::RParen) {
                    break;
                }
                continue;
            }
            break;
        }

        let close = self.expect_symbol(Symbol::RParen)?;
        Ok(ast::Pattern {
            span: Span::new(start, close.span.end),
            kind: ast::PatternKind::Tuple(elements),
        })
    }

    fn parse_struct_pattern(&mut self) -> Result<ast::Pattern, ParseError> {
        let path = self.parse_pattern_type_path()?;
        let open = self.expect_symbol(Symbol::LBrace)?;

        let mut fields = Vec::new();
        let mut rest_span: Option<Span> = None;
        if self.peek_symbol(Symbol::RBrace) {
            let close = self.bump();
            return Ok(ast::Pattern {
                span: Span::new(path.span.start, close.span.end),
                kind: ast::PatternKind::Struct {
                    path,
                    fields,
                    rest: rest_span,
                },
            });
        }

        loop {
            // `..` rest：仅允许出现一次，并且必须是最后一个字段。
            if rest_span.is_some() {
                let tok = *self.peek();
                if self.peek_symbol(Symbol::Dot)
                    && self.peek_n(1).kind == TokenKind::Symbol(Symbol::Dot)
                {
                    let err = ParseError::Expected {
                        expected: "struct pattern：`..` 只能出现一次",
                        found: tok.kind,
                        span: tok.span.into(),
                    };
                    let _ =
                        self.consume_balanced_after_open(Symbol::LBrace, Symbol::RBrace, open.span.start);
                    return Err(err);
                }
                let err = ParseError::Expected {
                    expected: "`}`（`..` 必须是最后一个字段）",
                    found: tok.kind,
                    span: tok.span.into(),
                };
                let _ =
                    self.consume_balanced_after_open(Symbol::LBrace, Symbol::RBrace, open.span.start);
                return Err(err);
            }

            if self.peek_symbol(Symbol::Dot) && self.peek_n(1).kind == TokenKind::Symbol(Symbol::Dot)
            {
                let dot1 = self.bump();
                let dot2 = self.bump();
                rest_span = Some(Span::new(dot1.span.start, dot2.span.end));

                if self.eat_symbol(Symbol::Comma) {
                    // allow trailing comma
                    if self.peek_symbol(Symbol::RBrace) {
                        break;
                    }
                    continue;
                }
                break;
            }

            let name_tok = self.expect_kind(TokenKind::Ident, "字段名（标识符）")?;
            let name = ast::Ident { span: name_tok.span };

            let value = if self.eat_symbol(Symbol::Colon) {
                Some(Box::new(self.parse_pattern()?))
            } else {
                None
            };

            let end = value
                .as_ref()
                .map(|p| p.span.end)
                .unwrap_or(name_tok.span.end);
            fields.push(ast::StructPatternField {
                span: Span::new(name_tok.span.start, end),
                name,
                value,
            });

            if self.eat_symbol(Symbol::Comma) {
                // allow trailing comma
                if self.peek_symbol(Symbol::RBrace) {
                    break;
                }
                continue;
            }
            break;
        }

        if self.peek_kind(TokenKind::Eof) {
            return Err(ParseError::UnterminatedGroup {
                close: Symbol::RBrace,
                span: Span::new(open.span.start, self.peek().span.end).into(),
            });
        }

        let close = self.expect_symbol(Symbol::RBrace)?;
        Ok(ast::Pattern {
            span: Span::new(path.span.start, close.span.end),
            kind: ast::PatternKind::Struct {
                path,
                fields,
                rest: rest_span,
            },
        })
    }

    fn parse_pattern_type_path(&mut self) -> Result<ast::TypePath, ParseError> {
        let first = self.expect_kind(TokenKind::Ident, "类型名（标识符）")?;
        let start = first.span.start;
        let mut segments = vec![ast::Ident { span: first.span }];

        while self.peek_symbol(Symbol::Dot) && self.peek_n(1).kind == TokenKind::Ident {
            self.bump(); // `.`
            let seg = self.bump(); // ident
            segments.push(ast::Ident { span: seg.span });
        }

        let end = segments.last().unwrap().span.end;
        Ok(ast::TypePath {
            span: Span::new(start, end),
            segments,
            args: Vec::new(),
        })
    }
}
