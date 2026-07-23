//! 模式解析（grammar §9）：`val` 解构（§9.1）与 `when` 分支模式（§9.2）。

use scoop2_base::Span;

use crate::ast::pattern::*;
use crate::ast::{CharLit, IntLit, StringLit, TypePath};
use crate::lexer::{char_literal, int_literal, string_literal};
use crate::token::{Keyword, Symbol, Token, TokenKind};

use super::{PResult, Parser};

impl<'a> Parser<'a> {
    // --------------------------------------------------------------
    // lookahead
    // --------------------------------------------------------------

    /// `Ident(.Ident)* {` 形态（struct pattern）。
    pub(crate) fn looks_like_struct_pattern_ahead(&self) -> bool {
        if !self.at_kind(TokenKind::Ident) {
            return false;
        }
        let mut n = 1usize;
        while self.peek_n(n).kind == TokenKind::Symbol(Symbol::Dot)
            && self.peek_n(n + 1).kind == TokenKind::Ident
        {
            n += 2;
        }
        self.peek_n(n).kind == TokenKind::Symbol(Symbol::LBrace)
    }

    /// `Ident(.Ident)* (` 形态（variant pattern）。
    pub(crate) fn looks_like_variant_pattern_ahead(&self) -> bool {
        if !self.at_kind(TokenKind::Ident) {
            return false;
        }
        let mut n = 1usize;
        while self.peek_n(n).kind == TokenKind::Symbol(Symbol::Dot)
            && self.peek_n(n + 1).kind == TokenKind::Ident
        {
            n += 2;
        }
        self.peek_n(n).kind == TokenKind::Symbol(Symbol::LParen)
    }

    // --------------------------------------------------------------
    // §9.1 解构模式（val 绑定）
    // --------------------------------------------------------------

    pub(crate) fn parse_pattern(&mut self) -> PResult<Pattern> {
        if self.at_sym(Symbol::LParen) {
            return self.parse_tuple_pattern();
        }

        if self.at_kind(TokenKind::Ident) {
            if self.looks_like_struct_pattern_ahead() {
                return self.parse_struct_pattern();
            }
            if self.looks_like_variant_pattern_ahead() {
                return self.parse_variant_pattern();
            }

            let tok = self.bump();
            let ident = self.ident(tok);
            if self.token_text(tok) == "_" {
                return Ok(Pattern {
                    id: self.nid(),
                    span: tok.span,
                    kind: PatternKind::Wildcard,
                });
            }
            return Ok(Pattern {
                id: self.nid(),
                span: tok.span,
                kind: PatternKind::Bind(ident),
            });
        }

        let tok = self.peek();
        Err(self.err_expected_pattern(tok))
    }

    /// `..` rest 检测（也可能 lex 成两个 `.`）。
    fn at_rest(&self) -> bool {
        self.at_sym(Symbol::DotDot) || (self.at_sym(Symbol::Dot) && self.at_sym_n(1, Symbol::Dot))
    }

    fn bump_rest(&mut self) -> Span {
        if self.at_sym(Symbol::DotDot) {
            self.bump().span
        } else {
            let dot1 = self.bump();
            let dot2 = self.bump();
            Span::new(dot1.span.start, dot2.span.end)
        }
    }

    fn parse_tuple_pattern(&mut self) -> PResult<Pattern> {
        let open = self.expect_sym(Symbol::LParen)?;
        let start = open.span.start;

        let mut elements = Vec::new();
        let mut rest_span: Option<Span> = None;
        if self.at_sym(Symbol::RParen) {
            let close = self.bump();
            return Ok(Pattern {
                id: self.nid(),
                span: Span::new(start, close.span.end),
                kind: PatternKind::Tuple(elements),
            });
        }

        loop {
            // `..` rest：至多一次且必须是最后一个元素。
            if let Some(prev_rest) = rest_span {
                let tok = self.peek();
                let msg = if self.at_rest() {
                    "tuple pattern：`..` 只能出现一次"
                } else {
                    "`)`（`..` 必须是最后一个元素）"
                };
                self.err_expected(msg, tok);
                let _ = self.consume_balanced_after_open(Symbol::LParen, Symbol::RParen, start);
                return Ok(Pattern {
                    id: self.nid(),
                    span: Span::new(start, prev_rest.end),
                    kind: PatternKind::Tuple(elements),
                });
            }

            if self.at_rest() {
                let span = self.bump_rest();
                rest_span = Some(span);
                elements.push(Pattern {
                    id: self.nid(),
                    span,
                    kind: PatternKind::Rest,
                });
            } else {
                elements.push(self.parse_pattern()?);
            }

            if self.eat_sym(Symbol::Comma) {
                if self.at_sym(Symbol::RParen) {
                    break;
                }
                continue;
            }
            break;
        }

        let close = self.expect_sym(Symbol::RParen)?;
        Ok(Pattern {
            id: self.nid(),
            span: Span::new(start, close.span.end),
            kind: PatternKind::Tuple(elements),
        })
    }

    fn parse_struct_pattern(&mut self) -> PResult<Pattern> {
        let path = self.parse_pattern_type_path()?;
        self.expect_sym(Symbol::LBrace)?;
        let start = path.span.start;

        let mut fields = Vec::new();
        let mut rest_span: Option<Span> = None;
        if self.at_sym(Symbol::RBrace) {
            let close = self.bump();
            return Ok(Pattern {
                id: self.nid(),
                span: Span::new(start, close.span.end),
                kind: PatternKind::Struct {
                    path,
                    fields,
                    rest: None,
                },
            });
        }

        loop {
            if rest_span.is_some() {
                let tok = self.peek();
                let msg = if self.at_rest() {
                    "struct pattern：`..` 只能出现一次"
                } else {
                    "`}`（`..` 必须是最后一个字段）"
                };
                self.err_expected(msg, tok);
                let _ = self.consume_balanced_after_open(Symbol::LBrace, Symbol::RBrace, start);
                return Ok(Pattern {
                    id: self.nid(),
                    span: Span::new(start, tok.span.end),
                    kind: PatternKind::Struct {
                        path,
                        fields,
                        rest: rest_span,
                    },
                });
            }

            if self.at_rest() {
                rest_span = Some(self.bump_rest());
                if self.eat_sym(Symbol::Comma) {
                    if self.at_sym(Symbol::RBrace) {
                        break;
                    }
                    continue;
                }
                break;
            }

            let name_tok = self.expect_ident("字段名")?;
            let name = self.ident(name_tok);
            let value = if self.eat_sym(Symbol::Colon) {
                Some(self.parse_pattern()?)
            } else {
                None
            };
            let end = value
                .as_ref()
                .map(|p| p.span.end)
                .unwrap_or(name_tok.span.end);
            fields.push(StructPatternField {
                id: self.nid(),
                span: Span::new(name_tok.span.start, end),
                name,
                pattern: value,
            });

            if self.eat_sym(Symbol::Comma) {
                if self.at_sym(Symbol::RBrace) {
                    break;
                }
                continue;
            }
            break;
        }

        let close = self.expect_sym(Symbol::RBrace)?;
        Ok(Pattern {
            id: self.nid(),
            span: Span::new(start, close.span.end),
            kind: PatternKind::Struct {
                path,
                fields,
                rest: rest_span,
            },
        })
    }

    fn parse_variant_pattern(&mut self) -> PResult<Pattern> {
        let path = self.parse_pattern_type_path()?;
        let start = path.span.start;
        self.expect_sym(Symbol::LParen)?;

        let mut args = Vec::new();
        let mut rest_span: Option<Span> = None;
        if self.at_sym(Symbol::RParen) {
            let close = self.bump();
            return Ok(Pattern {
                id: self.nid(),
                span: Span::new(start, close.span.end),
                kind: PatternKind::Variant {
                    path,
                    args: Some(args),
                },
            });
        }

        loop {
            if rest_span.is_some() {
                let tok = self.peek();
                let msg = if self.at_rest() {
                    "variant pattern：`..` 只能出现一次"
                } else {
                    "`)`（`..` 必须是最后一个参数）"
                };
                self.err_expected(msg, tok);
                let _ = self.consume_balanced_after_open(Symbol::LParen, Symbol::RParen, start);
                return Ok(Pattern {
                    id: self.nid(),
                    span: Span::new(start, tok.span.end),
                    kind: PatternKind::Variant {
                        path,
                        args: Some(args),
                    },
                });
            }

            if self.at_rest() {
                let span = self.bump_rest();
                rest_span = Some(span);
                args.push(Pattern {
                    id: self.nid(),
                    span,
                    kind: PatternKind::Rest,
                });
            } else {
                args.push(self.parse_pattern()?);
            }

            if self.eat_sym(Symbol::Comma) {
                if self.at_sym(Symbol::RParen) {
                    break;
                }
                continue;
            }
            break;
        }

        let close = self.expect_sym(Symbol::RParen)?;
        Ok(Pattern {
            id: self.nid(),
            span: Span::new(start, close.span.end),
            kind: PatternKind::Variant {
                path,
                args: Some(args),
            },
        })
    }

    fn parse_pattern_type_path(&mut self) -> PResult<TypePath> {
        let first = self.expect_ident("类型名")?;
        let start = first.span.start;
        let mut segments = vec![self.ident(first)];
        while self.at_sym(Symbol::Dot) && self.peek_n(1).kind == TokenKind::Ident {
            self.bump(); // `.`
            let seg = self.bump();
            segments.push(self.ident(seg));
        }
        let end = segments
            .last()
            .map(|s| s.span.end)
            .unwrap_or(first.span.end);
        Ok(TypePath {
            segments,
            span: Span::new(start, end),
        })
    }

    // --------------------------------------------------------------
    // §9.2 when 分支模式
    // --------------------------------------------------------------

    pub(crate) fn parse_when_pat(&mut self) -> PResult<Pattern> {
        let first = self.parse_when_pat_atom(true)?;

        // or-pattern：`A | B`（`else` 不允许出现在 `|` 之后）。
        if !self.at_sym(Symbol::Or) {
            return Ok(first);
        }
        let mut pats = vec![first];
        while self.eat_sym(Symbol::Or) {
            pats.push(self.parse_when_pat_atom(false)?);
        }
        let start = pats.first().map(|p| p.span.start).unwrap_or(0);
        let end = pats.last().map(|p| p.span.end).unwrap_or(start);
        Ok(Pattern {
            id: self.nid(),
            span: Span::new(start, end),
            kind: PatternKind::Or(pats),
        })
    }

    fn parse_when_pat_atom(&mut self, allow_else: bool) -> PResult<Pattern> {
        if allow_else && self.at_kw(Keyword::Else) {
            let tok = self.bump();
            return Ok(Pattern {
                id: self.nid(),
                span: tok.span,
                kind: PatternKind::Else,
            });
        }

        if self.at_kw(Keyword::Is) {
            let is_tok = self.bump();
            let ty = self.parse_type_ref()?;
            return Ok(Pattern {
                id: self.nid(),
                span: Span::new(is_tok.span.start, ty.span.end),
                kind: PatternKind::Is(ty),
            });
        }

        if self.at_sym(Symbol::LParen) {
            return self.parse_when_tuple_pat();
        }

        // float 字面量在模式中是 parse error：记录后按 wildcard 继续（§9.2）。
        if self.at_kind(TokenKind::FloatLiteral) {
            let tok = self.bump();
            self.err_expected(
                "when 分支模式（不支持 Float 字面量；请改用 guard 或 if）",
                tok,
            );
            return Ok(Pattern {
                id: self.nid(),
                span: tok.span,
                kind: PatternKind::Wildcard,
            });
        }

        if self.at_kind(TokenKind::IntLiteral) {
            let tok = self.bump();
            return Ok(Pattern {
                id: self.nid(),
                span: tok.span,
                kind: PatternKind::Literal(PatternLiteral::Int(self.decode_int(tok))),
            });
        }

        if self.at_kind(TokenKind::CharLiteral) {
            let tok = self.bump();
            return Ok(Pattern {
                id: self.nid(),
                span: tok.span,
                kind: PatternKind::Literal(PatternLiteral::Char(self.decode_char(tok))),
            });
        }

        if matches!(self.peek().kind, TokenKind::StringLiteral(_)) {
            let tok = self.bump();
            return Ok(Pattern {
                id: self.nid(),
                span: tok.span,
                kind: PatternKind::Literal(PatternLiteral::String(self.decode_string(tok))),
            });
        }

        if self.at_kind(TokenKind::Ident) {
            // qualified variant pattern：`a.b.C` / `a.b.C(...)`。
            if self.peek_n(1).kind == TokenKind::Symbol(Symbol::Dot)
                && self.peek_n(2).kind == TokenKind::Ident
            {
                return self.parse_when_qualified_variant_pat();
            }

            let tok = self.bump();
            let text = self.token_text(tok);
            let ident = self.ident(tok);

            if text == "_" {
                return Ok(Pattern {
                    id: self.nid(),
                    span: tok.span,
                    kind: PatternKind::Wildcard,
                });
            }
            if text == "true" || text == "false" {
                return Ok(Pattern {
                    id: self.nid(),
                    span: tok.span,
                    kind: PatternKind::Literal(PatternLiteral::Bool {
                        value: text == "true",
                        span: tok.span,
                    }),
                });
            }

            // `Name(...)`：unqualified variant pattern。
            if self.at_sym(Symbol::LParen) {
                let path = TypePath {
                    segments: vec![ident],
                    span: tok.span,
                };
                return self.parse_when_variant_args(path);
            }

            // 裸 `Ident`：大写开头 → 0 参数 variant；否则 bind（normative 启发式，§9.2）。
            if text.chars().next().is_some_and(char::is_uppercase) {
                return Ok(Pattern {
                    id: self.nid(),
                    span: tok.span,
                    kind: PatternKind::Variant {
                        path: TypePath {
                            segments: vec![ident],
                            span: tok.span,
                        },
                        args: None,
                    },
                });
            }

            return Ok(Pattern {
                id: self.nid(),
                span: tok.span,
                kind: PatternKind::Bind(ident),
            });
        }

        let tok = self.peek();
        Err(self.err_expected_pattern(tok))
    }

    fn parse_when_qualified_variant_pat(&mut self) -> PResult<Pattern> {
        let path = self.parse_pattern_type_path()?;
        if !self.at_sym(Symbol::LParen) {
            let span = path.span;
            return Ok(Pattern {
                id: self.nid(),
                span,
                kind: PatternKind::Variant { path, args: None },
            });
        }
        self.parse_when_variant_args(path)
    }

    fn parse_when_variant_args(&mut self, path: TypePath) -> PResult<Pattern> {
        let start = path.span.start;
        self.expect_sym(Symbol::LParen)?;

        let mut args = Vec::new();
        let mut rest_span: Option<Span> = None;
        if self.at_sym(Symbol::RParen) {
            let close = self.bump();
            return Ok(Pattern {
                id: self.nid(),
                span: Span::new(start, close.span.end),
                kind: PatternKind::Variant {
                    path,
                    args: Some(args),
                },
            });
        }

        loop {
            if rest_span.is_some() {
                let tok = self.peek();
                let msg = if self.at_rest() {
                    "when variant pattern：`..` 只能出现一次"
                } else {
                    "`)`（`..` 必须是最后一个参数）"
                };
                self.err_expected(msg, tok);
                let _ = self.consume_balanced_after_open(Symbol::LParen, Symbol::RParen, start);
                return Ok(Pattern {
                    id: self.nid(),
                    span: Span::new(start, tok.span.end),
                    kind: PatternKind::Variant {
                        path,
                        args: Some(args),
                    },
                });
            }

            if self.at_rest() {
                let span = self.bump_rest();
                rest_span = Some(span);
                args.push(Pattern {
                    id: self.nid(),
                    span,
                    kind: PatternKind::Rest,
                });
            } else {
                args.push(self.parse_when_pat()?);
            }
            if self.eat_sym(Symbol::Comma) {
                if self.at_sym(Symbol::RParen) {
                    break;
                }
                continue;
            }
            break;
        }

        let close = self.expect_sym(Symbol::RParen)?;
        Ok(Pattern {
            id: self.nid(),
            span: Span::new(start, close.span.end),
            kind: PatternKind::Variant {
                path,
                args: Some(args),
            },
        })
    }

    fn parse_when_tuple_pat(&mut self) -> PResult<Pattern> {
        let open = self.expect_sym(Symbol::LParen)?;
        let start = open.span.start;

        let mut elements = Vec::new();
        let mut rest_span: Option<Span> = None;
        if self.at_sym(Symbol::RParen) {
            let close = self.bump();
            return Ok(Pattern {
                id: self.nid(),
                span: Span::new(start, close.span.end),
                kind: PatternKind::Tuple(elements),
            });
        }

        loop {
            if rest_span.is_some() {
                let tok = self.peek();
                let msg = if self.at_rest() {
                    "when tuple pattern：`..` 只能出现一次"
                } else {
                    "`)`（`..` 必须是最后一个元素）"
                };
                self.err_expected(msg, tok);
                let _ = self.consume_balanced_after_open(Symbol::LParen, Symbol::RParen, start);
                return Ok(Pattern {
                    id: self.nid(),
                    span: Span::new(start, tok.span.end),
                    kind: PatternKind::Tuple(elements),
                });
            }

            if self.at_rest() {
                let span = self.bump_rest();
                rest_span = Some(span);
                elements.push(Pattern {
                    id: self.nid(),
                    span,
                    kind: PatternKind::Rest,
                });
            } else {
                elements.push(self.parse_when_pat()?);
            }
            if self.eat_sym(Symbol::Comma) {
                if self.at_sym(Symbol::RParen) {
                    break;
                }
                continue;
            }
            break;
        }

        let close = self.expect_sym(Symbol::RParen)?;
        Ok(Pattern {
            id: self.nid(),
            span: Span::new(start, close.span.end),
            kind: PatternKind::Tuple(elements),
        })
    }

    // --------------------------------------------------------------
    // 字面量 best-effort 解码（lexer 已报错的字面量不重报）
    // --------------------------------------------------------------

    pub(crate) fn decode_int(&mut self, tok: Token) -> IntLit {
        let text = self.token_text(tok);
        let suffix = match int_literal::parse_int_literal_suffix(text) {
            int_literal::IntLiteralSuffix::None => None,
            int_literal::IntLiteralSuffix::UInt => Some(crate::ast::IntSuffix::U),
            int_literal::IntLiteralSuffix::Long => Some(crate::ast::IntSuffix::L),
            int_literal::IntLiteralSuffix::ULong => Some(crate::ast::IntSuffix::UL),
        };
        let value = int_literal::parse_int_literal_checked(text).unwrap_or(0);
        IntLit {
            value,
            suffix,
            span: tok.span,
        }
    }

    pub(crate) fn decode_char(&mut self, tok: Token) -> CharLit {
        let value = char_literal::parse_char_literal(self.token_text(tok)).unwrap_or('\0');
        CharLit {
            value,
            span: tok.span,
        }
    }

    pub(crate) fn decode_string(&mut self, tok: Token) -> StringLit {
        let text = self.token_text(tok);
        let value = string_literal::parse_string_literal_utf8(text)
            .unwrap_or_else(|_| text.trim_matches('"').to_string());
        StringLit {
            value,
            span: tok.span,
        }
    }
}
