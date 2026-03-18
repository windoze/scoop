//! 类型引用（TypeRef）解析（最小子集）。

use crate::ast;
use crate::span::Span;
use crate::syntax::token::{Symbol, TokenKind};

use super::{ParseError, Parser};

impl<'a> Parser<'a> {
    pub(super) fn parse_type_ref(&mut self) -> Result<ast::TypeRef, ParseError> {
        let mut ty = if self.peek_symbol(Symbol::LParen) {
            self.parse_tuple_or_group_type()?
        } else {
            self.parse_path_type()?
        };

        if self.peek_symbol(Symbol::Question) {
            let q = self.bump();
            let span = Span::new(ty.span().start, q.span.end);
            ty = ast::TypeRef::Nullable {
                span,
                inner: Box::new(ty),
            };
        }

        Ok(ty)
    }

    fn parse_tuple_or_group_type(&mut self) -> Result<ast::TypeRef, ParseError> {
        let open = self.expect_symbol(Symbol::LParen)?;
        let start = open.span.start;

        if self.peek_symbol(Symbol::RParen) {
            let close = self.bump();
            return Ok(ast::TypeRef::Tuple(ast::TypeTuple {
                span: Span::new(start, close.span.end),
                elements: Vec::new(),
            }));
        }

        let first = self.parse_type_ref()?;
        if self.eat_symbol(Symbol::Comma) {
            let mut elements = vec![first];
            while !self.peek_symbol(Symbol::RParen) && !self.peek_kind(TokenKind::Eof) {
                elements.push(self.parse_type_ref()?);
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
            return Ok(ast::TypeRef::Tuple(ast::TypeTuple {
                span: Span::new(start, close.span.end),
                elements,
            }));
        }

        // grouping type: `(T)` → `T`
        let _close = self.expect_symbol(Symbol::RParen)?;
        Ok(first)
    }

    fn parse_path_type(&mut self) -> Result<ast::TypeRef, ParseError> {
        let first = self.expect_kind(TokenKind::Ident, "类型名（标识符）")?;
        let start = first.span.start;
        let mut segments = vec![ast::Ident { span: first.span }];

        while self.peek_symbol(Symbol::Dot) {
            self.bump();
            let seg = self.expect_kind(TokenKind::Ident, "类型名（标识符）")?;
            segments.push(ast::Ident { span: seg.span });
        }

        let mut args = Vec::new();
        let mut end = segments.last().unwrap().span.end;
        if self.peek_symbol(Symbol::Lt) {
            let (a, gt_end) = self.parse_type_args()?;
            args = a;
            end = gt_end;
        }

        Ok(ast::TypeRef::Path(ast::TypePath {
            span: Span::new(start, end),
            segments,
            args,
        }))
    }

    fn parse_type_args(&mut self) -> Result<(Vec<ast::TypeRef>, usize), ParseError> {
        let _lt = self.expect_symbol(Symbol::Lt)?;
        let mut args = Vec::new();

        if self.peek_symbol(Symbol::Gt) {
            let gt = self.bump();
            return Ok((args, gt.span.end));
        }

        loop {
            args.push(self.parse_type_ref()?);
            if self.eat_symbol(Symbol::Comma) {
                if self.peek_symbol(Symbol::Gt) {
                    break;
                }
                continue;
            }
            break;
        }
        let gt = self.expect_symbol(Symbol::Gt)?;
        Ok((args, gt.span.end))
    }
}
