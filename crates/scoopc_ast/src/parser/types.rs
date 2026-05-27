//! 类型引用（TypeRef）解析（最小子集）。

use crate::ast;
use crate::span::Span;
use crate::syntax::token::{Symbol, TokenKind};

use super::{ParseError, Parser};

#[derive(Debug, Clone)]
struct ParenTypeList {
    span: Span,
    elements: Vec<ast::TypeRef>,
    /// 是否出现过 `,`（用于区分 tuple type vs grouping type）。
    had_comma: bool,
}

impl<'a> Parser<'a> {
    pub(super) fn parse_type_ref(&mut self) -> Result<ast::TypeRef, ParseError> {
        // 1) 解析基础类型（tuple/group/path），以及最小的 function type 形式。
        //
        // 注意：函数类型语法要求参数列表必须是括号包裹的 `(T, U)`；
        // 因此当看到 `(` 时，需要先解析出括号内的 type 列表，再根据是否紧跟 `->`
        // 决定它是 tuple/group type 还是 function type 的参数列表。
        let mut ty = if self.peek_symbol(Symbol::LParen) {
            let paren = self.parse_paren_type_list()?;
            if self.peek_symbol(Symbol::Arrow) {
                self.parse_function_type(None, paren)?
            } else if paren.elements.is_empty() || paren.had_comma {
                ast::TypeRef::Tuple(ast::TypeTuple {
                    span: paren.span,
                    elements: paren.elements,
                })
            } else {
                // grouping type: `(T)` → `T`
                paren
                    .elements
                    .into_iter()
                    .next()
                    .expect("grouping type must have one element")
            }
        } else {
            self.parse_path_type()?
        };

        // 2) nullable：`T?`
        if self.peek_symbol(Symbol::Question) {
            let q = self.bump();
            let span = Span::new(ty.span().start, q.span.end);
            ty = ast::TypeRef::Nullable {
                span,
                inner: Box::new(ty),
            };
        }

        // 3) receiver function type：`T.(A, B) -> R`
        if self.peek_symbol(Symbol::Dot) && self.peek_n(1).kind == TokenKind::Symbol(Symbol::LParen)
        {
            // 这里的 `ty` 就是 receiver TypeRef（允许 `T?` 这种 nullable receiver）。
            let receiver = ty;
            self.bump(); // `.`

            let params = self.parse_paren_type_list()?;
            ty = self.parse_function_type(Some(receiver), params)?;

            // receiver function type 也允许再接 `?`（例如 `T.() -> R?` / `(...) -> ... / Pure?` 等）。
            if self.peek_symbol(Symbol::Question) {
                let q = self.bump();
                let span = Span::new(ty.span().start, q.span.end);
                ty = ast::TypeRef::Nullable {
                    span,
                    inner: Box::new(ty),
                };
            }
        }

        Ok(ty)
    }

    fn parse_paren_type_list(&mut self) -> Result<ParenTypeList, ParseError> {
        let open = self.expect_symbol(Symbol::LParen)?;
        let start = open.span.start;

        if self.peek_symbol(Symbol::RParen) {
            let close = self.bump();
            return Ok(ParenTypeList {
                span: Span::new(start, close.span.end),
                elements: Vec::new(),
                had_comma: true,
            });
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
            return Ok(ParenTypeList {
                span: Span::new(start, close.span.end),
                elements,
                had_comma: true,
            });
        }

        let close = self.expect_symbol(Symbol::RParen)?;
        Ok(ParenTypeList {
            span: Span::new(start, close.span.end),
            elements: vec![first],
            had_comma: false,
        })
    }

    fn parse_function_type(
        &mut self,
        receiver: Option<ast::TypeRef>,
        params: ParenTypeList,
    ) -> Result<ast::TypeRef, ParseError> {
        let _arrow = self.expect_symbol(Symbol::Arrow)?;
        let return_ty = self.parse_type_ref()?;

        let effects = if self.eat_symbol(Symbol::Slash) {
            Some(self.parse_effect_row_expr()?)
        } else {
            None
        };

        let start = receiver
            .as_ref()
            .map(|r| r.span().start)
            .unwrap_or(params.span.start);
        let end = effects
            .as_ref()
            .map(|r| r.span.end)
            .unwrap_or(return_ty.span().end);

        Ok(ast::TypeRef::Function(ast::TypeFunction {
            span: Span::new(start, end),
            receiver: receiver.map(Box::new),
            params_span: params.span,
            params: params.elements,
            return_ty: Box::new(return_ty),
            effects,
        }))
    }

    fn parse_path_type(&mut self) -> Result<ast::TypeRef, ParseError> {
        if let Some(keyword) = self.peek_bound_keyword() {
            let tok = *self.peek();
            return Err(ParseError::BoundKeywordTypePosition {
                keyword,
                span: tok.span.into(),
            });
        }

        let first = self.expect_kind(TokenKind::Ident, "类型名（标识符）")?;
        let start = first.span.start;
        let mut segments = vec![ast::Ident::new(first.span)];

        // 仅当 `.` 后面仍是标识符时，才将其视为路径段分隔。
        //
        // 这允许 receiver function type 的 `T.(...) -> ...`：当看到 `.(` 时停止消费路径段，
        // 交由外层解析函数类型。
        while self.peek_symbol(Symbol::Dot) && self.peek_n(1).kind == TokenKind::Ident {
            self.bump(); // `.`
            let seg = self.bump(); // ident
            segments.push(ast::Ident::new(seg.span));
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

    pub(super) fn peek_bound_keyword(&self) -> Option<&'static str> {
        if self.peek_ident_text("ref") {
            Some("ref")
        } else if self.peek_ident_text("value") {
            Some("value")
        } else {
            None
        }
    }

    pub(super) fn parse_type_args(&mut self) -> Result<(Vec<ast::TypeRef>, usize), ParseError> {
        let _lt = self.expect_symbol(Symbol::Lt)?;
        let mut args = Vec::new();

        // Handle empty type args `<>`.
        if self.peek_symbol(Symbol::Gt) {
            let gt = self.bump();
            return Ok((args, gt.span.end));
        }

        loop {
            // T0253：use-site effect row 实参 `Type<eff Row>`（spec §3.4 / §5.8）。
            //
            // 说明：
            // - `eff` 是上下文关键字：仅在 `<...>` 内被当作关键字处理；
            // - 按 spec 约束：`eff` 子句至多一个，且必须位于类型实参列表末尾。
            if self.peek_ident_text("eff") {
                let eff_kw = self.bump(); // ident("eff")
                let row = self.parse_effect_row_expr()?;
                args.push(ast::TypeRef::EffectRowArg {
                    span: Span::new(eff_kw.span.start, row.span.end),
                    row,
                });

                // allow trailing comma, but `eff` 必须是最后一个条目。
                if self.eat_symbol(Symbol::Comma) && !self.peek_gt_or_gtgt() {
                    let tok = *self.peek();
                    return Err(ParseError::Expected {
                        expected: "`>`（`eff` 实参必须位于类型实参列表末尾）",
                        found: tok.kind,
                        span: tok.span.into(),
                    });
                }

                break;
            }

            // T0249：支持 star projection `*`（仅允许出现在类型实参位置，例如 `List<*>`）。
            if self.peek_symbol(Symbol::Star) {
                let star = self.bump();
                args.push(ast::TypeRef::Star { span: star.span });
            } else {
                args.push(self.parse_type_ref()?);
            }
            if self.eat_symbol(Symbol::Comma) {
                // Allow trailing comma before `>` or `>>` (nested generics).
                if self.peek_gt_or_gtgt() {
                    break;
                }
                continue;
            }
            break;
        }
        // Handle `>>` as two `>` tokens for nested generics (e.g., `Continuation<Continuation<Int, Unit>>`).
        let gt = self.expect_gt_or_split_gtgt()?;
        Ok((args, gt.span.end))
    }

    pub(super) fn parse_effect_row_expr(&mut self) -> Result<ast::EffectRowExpr, ParseError> {
        // 支持括号：`/ (Async + Raise<IOError>)`
        let mut expr = if self.peek_symbol(Symbol::LParen) {
            let open = self.bump();
            let mut inner = self.parse_effect_row_expr()?;
            let close = self.expect_symbol(Symbol::RParen)?;

            // 注意：括号只是分组；闭合标记 `!`（若存在）由 inner 自身解析。
            inner.span = Span::new(open.span.start, close.span.end);
            inner
        } else {
            let start = self.peek().span.start;

            let mut terms = Vec::new();
            let first = self.parse_effect_row_term()?;
            if let Some(term) = first.path {
                terms.push(term);
            }
            let mut end = first.end;

            while self.eat_symbol(Symbol::Plus) {
                let next = self.parse_effect_row_term()?;
                if let Some(term) = next.path {
                    terms.push(term);
                }
                end = next.end;
            }

            ast::EffectRowExpr {
                span: Span::new(start, end),
                terms,
                closed: false,
            }
        };

        // 闭合 effect row：`E!`（spec §5.8.4）
        //
        // 说明：`!` 的优先级低于 `+`，因此它作用于整个 row 表达式，而不是最后一个 effect 项。
        if self.peek_symbol(Symbol::Bang) {
            let bang = self.bump();
            expr.closed = true;
            expr.span = Span::new(expr.span.start, bang.span.end);
        }

        Ok(expr)
    }

    fn parse_effect_row_term(&mut self) -> Result<EffectRowTermParse, ParseError> {
        // effect row 的项在语法上与 type path 很接近（`Raise<IOError>`），因此复用 `parse_path_type`。
        let ty = self.parse_path_type()?;
        let ast::TypeRef::Path(path) = ty else {
            unreachable!("parse_path_type must return TypeRef::Path");
        };

        let end = path.span.end;

        // `Pure`：空 row。
        if path.segments.len() == 1
            && path.args.is_empty()
            && self
                .source_text
                .get(path.segments[0].span.start..path.segments[0].span.end)
                == Some("Pure")
        {
            return Ok(EffectRowTermParse { end, path: None });
        }

        Ok(EffectRowTermParse {
            end,
            path: Some(path),
        })
    }
}

#[derive(Debug, Clone)]
struct EffectRowTermParse {
    end: usize,
    path: Option<ast::TypePath>,
}
