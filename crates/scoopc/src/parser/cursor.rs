//! Token cursor 与基础消费函数。
//!
//! 这一层只提供“看/吃 token”的能力，不引入更高层的语法概念。

use crate::span::Span;
use crate::syntax::token::{Keyword, Symbol, Token, TokenKind};

use super::{ParseError, Parser};

impl<'a> Parser<'a> {
    pub(super) fn consume_balanced(
        &mut self,
        open: Symbol,
        close: Symbol,
    ) -> Result<Span, ParseError> {
        let open_tok = self.expect_symbol(open)?;
        let start = open_tok.span.start;

        let mut depth = 1usize;
        while !self.peek_kind(TokenKind::Eof) {
            let tok = self.bump();
            if let TokenKind::Symbol(sym) = tok.kind {
                if sym == open {
                    depth += 1;
                } else if sym == close {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(Span::new(start, tok.span.end));
                    }
                }
            }
        }

        Err(ParseError::UnterminatedGroup {
            close,
            span: Span::new(start, self.peek().span.end).into(),
        })
    }

    pub(super) fn expect_keyword(&mut self, kw: Keyword) -> Result<Token, ParseError> {
        if self.peek_keyword(kw) {
            Ok(self.bump())
        } else {
            let tok = *self.peek();
            Err(ParseError::Expected {
                expected: kw_name(kw),
                found: tok.kind,
                span: tok.span.into(),
            })
        }
    }

    pub(super) fn expect_symbol(&mut self, sym: Symbol) -> Result<Token, ParseError> {
        if self.peek_symbol(sym) {
            Ok(self.bump())
        } else {
            let tok = *self.peek();
            Err(ParseError::Expected {
                expected: sym_name(sym),
                found: tok.kind,
                span: tok.span.into(),
            })
        }
    }

    pub(super) fn expect_kind(
        &mut self,
        kind: TokenKind,
        expected: &'static str,
    ) -> Result<Token, ParseError> {
        if self.peek_kind(kind) {
            Ok(self.bump())
        } else {
            let tok = *self.peek();
            Err(ParseError::Expected {
                expected,
                found: tok.kind,
                span: tok.span.into(),
            })
        }
    }

    pub(super) fn eat_symbol(&mut self, sym: Symbol) -> bool {
        if self.peek_symbol(sym) {
            self.bump();
            true
        } else {
            false
        }
    }

    pub(super) fn peek(&self) -> &Token {
        self.tokens.get(self.i).unwrap_or_else(|| {
            self.tokens
                .last()
                .expect("lexer must produce at least EOF token")
        })
    }

    /// 向前看第 `n` 个 token（`n=0` 等价于 `peek()`）。
    ///
    /// 超出范围时返回最后一个 token（lexer 保证至少有 EOF）。
    pub(super) fn peek_n(&self, n: usize) -> &Token {
        self.tokens.get(self.i + n).unwrap_or_else(|| {
            self.tokens
                .last()
                .expect("lexer must produce at least EOF token")
        })
    }

    pub(super) fn bump(&mut self) -> Token {
        let tok = *self.peek();
        self.i = (self.i + 1).min(self.tokens.len());
        tok
    }

    pub(super) fn peek_kind(&self, kind: TokenKind) -> bool {
        self.peek().kind == kind
    }

    pub(super) fn peek_keyword(&self, kw: Keyword) -> bool {
        self.peek().kind == TokenKind::Keyword(kw)
    }

    pub(super) fn peek_symbol(&self, sym: Symbol) -> bool {
        self.peek().kind == TokenKind::Symbol(sym)
    }

    pub(super) fn peek_after_modifiers(&self) -> &Token {
        let mut idx = self.i;
        loop {
            let tok = self.tokens.get(idx).unwrap_or_else(|| {
                self.tokens
                    .last()
                    .expect("lexer must produce at least EOF token")
            });

            match tok.kind {
                TokenKind::Keyword(kw) if is_modifier_keyword(kw) => {
                    idx = idx.saturating_add(1);
                    continue;
                }
                _ => return tok,
            }
        }
    }

    pub(super) fn is_type_decl_start(&self) -> bool {
        matches!(
            self.peek_after_modifiers().kind,
            TokenKind::Keyword(
                Keyword::Class
                    | Keyword::Interface
                    | Keyword::Struct
                    | Keyword::Enum
                    | Keyword::Effect
            )
        )
    }

    pub(super) fn is_top_level_item_start(&self) -> bool {
        if matches!(
            self.peek().kind,
            TokenKind::Keyword(Keyword::Package | Keyword::Import)
        ) {
            return true;
        }

        matches!(
            self.peek_after_modifiers().kind,
            TokenKind::Keyword(
                Keyword::Fun
                    | Keyword::Val
                    | Keyword::Var
                    | Keyword::Class
                    | Keyword::Interface
                    | Keyword::Struct
                    | Keyword::Enum
                    | Keyword::Effect
            )
        )
    }

    pub(super) fn is_type_member_start(&self) -> bool {
        matches!(
            self.peek_after_modifiers().kind,
            TokenKind::Keyword(
                Keyword::Val
                    | Keyword::Var
                    | Keyword::Fun
                    | Keyword::Class
                    | Keyword::Interface
                    | Keyword::Struct
                    | Keyword::Enum
                    | Keyword::Effect
            )
        )
    }

    /// 粗粒度判断：当前位置是否“可能是一个语句的起始”。
    ///
    /// 该函数主要用于错误恢复（T0220），用于在 block 内尽量恢复到下一个语句边界，
    /// 而不是因为一个语法错误吞掉后续整个 block。
    pub(super) fn is_stmt_start(&self) -> bool {
        matches!(
            self.peek().kind,
            TokenKind::Keyword(
                Keyword::Val
                    | Keyword::Var
                    | Keyword::Return
                    | Keyword::Comptime
                    | Keyword::If
                    | Keyword::When
                    | Keyword::While
                    | Keyword::Break
                    | Keyword::Continue
                    | Keyword::Try
                    | Keyword::Handle
                    | Keyword::Perform
                    | Keyword::Async
                    | Keyword::Await
            ) | TokenKind::Ident
                | TokenKind::IntLiteral
                | TokenKind::StringLiteral(_)
                | TokenKind::Symbol(Symbol::LBrace | Symbol::LParen)
        )
    }
}

fn kw_name(kw: Keyword) -> &'static str {
    match kw {
        Keyword::Public => "`public`",
        Keyword::Internal => "`internal`",
        Keyword::Private => "`private`",
        Keyword::Open => "`open`",
        Keyword::Abstract => "`abstract`",
        Keyword::Sealed => "`sealed`",
        Keyword::Inline => "`inline`",
        Keyword::Override => "`override`",
        Keyword::Const => "`const`",
        Keyword::Package => "`package`",
        Keyword::Import => "`import`",
        Keyword::Fun => "`fun`",
        Keyword::Val => "`val`",
        Keyword::Var => "`var`",
        Keyword::Class => "`class`",
        Keyword::Interface => "`interface`",
        Keyword::Struct => "`struct`",
        Keyword::Enum => "`enum`",
        Keyword::Effect => "`effect`",
        Keyword::Handle => "`handle`",
        Keyword::With => "`with`",
        Keyword::Perform => "`perform`",
        Keyword::Try => "`try`",
        Keyword::Catch => "`catch`",
        Keyword::Finally => "`finally`",
        Keyword::Async => "`async`",
        Keyword::Await => "`await`",
        Keyword::Return => "`return`",
        Keyword::Comptime => "`comptime`",
        Keyword::If => "`if`",
        Keyword::Else => "`else`",
        Keyword::When => "`when`",
        Keyword::For => "`for`",
        Keyword::In => "`in`",
        Keyword::While => "`while`",
        Keyword::Break => "`break`",
        Keyword::Continue => "`continue`",
        Keyword::Is => "`is`",
        Keyword::As => "`as`",
        Keyword::AsQ => "`as?`",
    }
}

fn is_modifier_keyword(kw: Keyword) -> bool {
    matches!(
        kw,
        Keyword::Public
            | Keyword::Internal
            | Keyword::Private
            | Keyword::Open
            | Keyword::Abstract
            | Keyword::Sealed
            | Keyword::Inline
            | Keyword::Override
            | Keyword::Const
    )
}

fn sym_name(sym: Symbol) -> &'static str {
    match sym {
        Symbol::At => "`@`",
        Symbol::LParen => "`(`",
        Symbol::RParen => "`)`",
        Symbol::LBrace => "`{`",
        Symbol::RBrace => "`}`",
        Symbol::LBracket => "`[`",
        Symbol::RBracket => "`]`",
        Symbol::Comma => "`,`",
        Symbol::Colon => "`:`",
        Symbol::Semicolon => "`;`",
        Symbol::Dot => "`.`",
        Symbol::Plus => "`+`",
        Symbol::Minus => "`-`",
        Symbol::Star => "`*`",
        Symbol::Slash => "`/`",
        Symbol::Percent => "`%`",
        Symbol::And => "`&`",
        Symbol::Or => "`|`",
        Symbol::Caret => "`^`",
        Symbol::Tilde => "`~`",
        Symbol::Eq => "`=`",
        Symbol::Lt => "`<`",
        Symbol::Gt => "`>`",
        Symbol::Bang => "`!`",
        Symbol::Question => "`?`",
        Symbol::Arrow => "`->`",
        Symbol::EqEq => "`==`",
        Symbol::BangEq => "`!=`",
        Symbol::LtEq => "`<=`",
        Symbol::GtEq => "`>=`",
        Symbol::LtLt => "`<<`",
        Symbol::GtGt => "`>>`",
        Symbol::AndAnd => "`&&`",
        Symbol::OrOr => "`||`",
        Symbol::BangBang => "`!!`",
        Symbol::QuestionDot => "`?.`",
        Symbol::Elvis => "`?:`",
    }
}
