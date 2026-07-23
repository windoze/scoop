//! 类型引用（type refs）与 effect 行解析（grammar §5.2 / §6）。

use scoop2_base::Span;

use crate::ast::types::*;
use crate::ast::TypePath;
use crate::token::{Symbol, TokenKind};

use super::{PResult, Parser};

#[derive(Debug)]
pub(crate) struct ParenTypeList {
    pub span: Span,
    pub elements: Vec<TypeRef>,
    /// 是否出现过 `,`（区分 tuple type 与透明分组）。
    pub had_comma: bool,
}

impl<'a> Parser<'a> {
    /// `typeRef`（§6）：base + 零或多个后缀 `?` + 可选 receiver function tail（后再跟 `?`*）。
    pub(crate) fn parse_type_ref(&mut self) -> PResult<TypeRef> {
        self.enter()?;
        let result = self.parse_type_ref_inner();
        self.leave();
        result
    }

    fn parse_type_ref_inner(&mut self) -> PResult<TypeRef> {
        let mut ty = if self.at_sym(Symbol::LParen) {
            let paren = self.parse_paren_type_list()?;
            if self.at_sym(Symbol::Arrow) {
                self.parse_function_type(None, paren)?
            } else if paren.elements.is_empty() {
                TypeRef {
                    id: self.nid(),
                    span: paren.span,
                    kind: TypeRefKind::Unit,
                }
            } else if paren.had_comma {
                TypeRef {
                    id: self.nid(),
                    span: paren.span,
                    kind: TypeRefKind::Tuple(paren.elements),
                }
            } else {
                // 透明分组 `(T)` → `T`（不产生节点）。
                let mut elements = paren.elements;
                elements
                    .pop()
                    .unwrap_or_else(|| TypeRef {
                        // invariant: had_comma == false 且非空时恰好一个元素；防御性兜底。
                        id: self.nid(),
                        span: paren.span,
                        kind: TypeRefKind::Unit,
                    })
            }
        } else {
            self.parse_path_type()?
        };

        // 后缀 `?`（每个包一层 Option，§6：不拍平）。
        while self.at_sym(Symbol::Question) {
            let q = self.bump();
            let span = Span::new(ty.span.start, q.span.end);
            ty = TypeRef {
                id: self.nid(),
                span,
                kind: TypeRefKind::Nullable(Box::new(ty)),
            };
        }

        // `T?.(` 被 lexer 合成一个 `?.` token：在 receiver function type 位置拆成 `?` + `.`。
        if self.at_sym(Symbol::QuestionDot) && self.at_sym_n(1, Symbol::LParen) {
            let qd = self.peek();
            let span = Span::new(ty.span.start, qd.span.start + 1);
            ty = TypeRef {
                id: self.nid(),
                span,
                kind: TypeRefKind::Nullable(Box::new(ty)),
            };
            // 原地把 `?.` 改成 `.`（右半）。
            self.tokens[self.i] = crate::token::Token {
                kind: TokenKind::Symbol(Symbol::Dot),
                span: Span::new(qd.span.start + 1, qd.span.end),
            };
        }

        // receiverFnTail：`. (A, B) -> R effectAnn?`
        if self.at_sym(Symbol::Dot) && self.at_sym_n(1, Symbol::LParen) {
            self.bump(); // `.`
            let params = self.parse_paren_type_list()?;
            ty = self.parse_function_type(Some(ty), params)?;

            // receiver function type 之后同样允许 `?`*。
            while self.at_sym(Symbol::Question) {
                let q = self.bump();
                let span = Span::new(ty.span.start, q.span.end);
                ty = TypeRef {
                    id: self.nid(),
                    span,
                    kind: TypeRefKind::Nullable(Box::new(ty)),
                };
            }
        }

        Ok(ty)
    }

    fn parse_paren_type_list(&mut self) -> PResult<ParenTypeList> {
        let open = self.expect_sym(Symbol::LParen)?;
        let start = open.span.start;

        if self.at_sym(Symbol::RParen) {
            let close = self.bump();
            return Ok(ParenTypeList {
                span: Span::new(start, close.span.end),
                elements: Vec::new(),
                had_comma: false,
            });
        }

        let first = self.parse_type_ref()?;
        if self.eat_sym(Symbol::Comma) {
            let mut elements = vec![first];
            while !self.at_sym(Symbol::RParen) && !self.at_eof() {
                elements.push(self.parse_type_ref()?);
                if self.eat_sym(Symbol::Comma) {
                    if self.at_sym(Symbol::RParen) {
                        break;
                    }
                    continue;
                }
                break;
            }
            let close = self.expect_sym(Symbol::RParen)?;
            return Ok(ParenTypeList {
                span: Span::new(start, close.span.end),
                elements,
                had_comma: true,
            });
        }

        let close = self.expect_sym(Symbol::RParen)?;
        Ok(ParenTypeList {
            span: Span::new(start, close.span.end),
            elements: vec![first],
            had_comma: false,
        })
    }

    fn parse_function_type(
        &mut self,
        receiver: Option<TypeRef>,
        params: ParenTypeList,
    ) -> PResult<TypeRef> {
        self.expect_sym(Symbol::Arrow)?;
        let return_ty = self.parse_type_ref()?;

        let effect = if self.eat_sym(Symbol::Slash) {
            Some(self.parse_effect_row_expr()?)
        } else {
            None
        };

        let start = receiver
            .as_ref()
            .map(|r| r.span.start)
            .unwrap_or(params.span.start);
        let end = effect
            .as_ref()
            .map(|r| r.span.end)
            .unwrap_or(return_ty.span.end);

        let kind = match receiver {
            Some(receiver) => TypeRefKind::ReceiverFunction {
                receiver: Box::new(receiver),
                params: params.elements,
                ret: Box::new(return_ty),
                effect,
            },
            None => TypeRefKind::Function {
                params: params.elements,
                ret: Box::new(return_ty),
                effect,
            },
        };

        Ok(TypeRef {
            id: self.nid(),
            span: Span::new(start, end),
            kind,
        })
    }

    fn parse_path_type(&mut self) -> PResult<TypeRef> {
        // `ref` / `value` 在任何 typeRef 位置都是硬错误（§10）。
        if let Some(keyword) = self.peek_bound_keyword() {
            let tok = self.peek();
            return Err(self.err_bound_keyword_type_position(keyword, tok.span));
        }

        let first = self.peek();
        if first.kind != TokenKind::Ident {
            return Err(self.err_expected_type(first));
        }
        self.bump();
        let start = first.span.start;
        let first_ident = self.ident(first);
        let mut segments = vec![first_ident];

        // 仅当 `.` 后仍是标识符时才视为路径段（`.(` 让位 receiver function type）。
        while self.at_sym(Symbol::Dot) && self.peek_n(1).kind == TokenKind::Ident {
            self.bump(); // `.`
            let seg = self.bump();
            segments.push(self.ident(seg));
        }

        let mut args = Vec::new();
        let mut end = segments
            .last()
            .map(|s| s.span.end)
            .unwrap_or(first.span.end);
        if self.at_sym(Symbol::Lt) {
            let (a, gt_end) = self.parse_type_args()?;
            args = a;
            end = gt_end;
        }

        Ok(TypeRef {
            id: self.nid(),
            span: Span::new(start, end),
            kind: TypeRefKind::Path {
                path: TypePath {
                    segments,
                    span: Span::new(start, end),
                },
                args,
            },
        })
    }

    pub(crate) fn peek_bound_keyword(&self) -> Option<&'static str> {
        if self.at_ident_text("ref") {
            Some("ref")
        } else if self.at_ident_text("value") {
            Some("value")
        } else {
            None
        }
    }

    /// `typeArgs`（§5.2）：返回实参与闭合 `>` 的结束偏移。
    pub(crate) fn parse_type_args(&mut self) -> PResult<(Vec<TypeArg>, usize)> {
        self.expect_sym(Symbol::Lt)?;
        let mut args = Vec::new();

        // 空实参 `<>`。
        if self.at_sym(Symbol::Gt) {
            let gt = self.bump();
            return Ok((args, gt.span.end));
        }

        loop {
            // `eff` effect-row 实参（上下文关键字；至多一个且必须最后）。
            if self.at_ident_text("eff") {
                let eff_kw = self.bump();
                let row = self.parse_effect_row_expr()?;
                args.push(TypeArg {
                    id: self.nid(),
                    span: Span::new(eff_kw.span.start, row.span.end),
                    kind: TypeArgKind::Effect(row),
                });
                if self.eat_sym(Symbol::Comma) && !self.at_gt_like() {
                    let tok = self.peek();
                    return Err(self.err_expected("`>`（`eff` 实参必须位于类型实参列表末尾）", tok));
                }
                break;
            }

            if self.at_sym(Symbol::Star) {
                let star = self.bump();
                args.push(TypeArg {
                    id: self.nid(),
                    span: star.span,
                    kind: TypeArgKind::Star,
                });
            } else {
                let ty = self.parse_type_ref()?;
                args.push(TypeArg {
                    id: self.nid(),
                    span: ty.span,
                    kind: TypeArgKind::Type(ty),
                });
            }
            if self.eat_sym(Symbol::Comma) {
                if self.at_gt_like() {
                    break;
                }
                continue;
            }
            break;
        }

        let gt = self.expect_gt_close()?;
        Ok((args, gt.span.end))
    }

    fn at_gt_like(&self) -> bool {
        self.at_sym(Symbol::Gt) || self.at_sym(Symbol::GtGt) || self.at_sym(Symbol::GtEq)
    }

    /// `effectRowExpr`（§6.1）：`( Row )` 或 `term (+ term)*`，尾部 `!` 闭合整行。
    pub(crate) fn parse_effect_row_expr(&mut self) -> PResult<EffectRowExpr> {
        let mut expr = if self.at_sym(Symbol::LParen) {
            let open = self.bump();
            let inner = self.parse_effect_row_expr()?;
            let close = self.expect_sym(Symbol::RParen)?;
            EffectRowExpr {
                id: self.nid(),
                span: Span::new(open.span.start, close.span.end),
                terms: inner.terms,
                closed: inner.closed,
            }
        } else {
            let start = self.peek().span.start;
            let mut terms = vec![self.parse_effect_row_term()?];
            let mut end = terms[0].span.end;
            while self.eat_sym(Symbol::Plus) {
                let term = self.parse_effect_row_term()?;
                end = term.span.end;
                terms.push(term);
            }
            EffectRowExpr {
                id: self.nid(),
                span: Span::new(start, end),
                terms,
                closed: None,
            }
        };

        // 闭合行 `!`：作用于整行（优先级低于 `+`）。
        if self.at_sym(Symbol::Bang) {
            let bang = self.bump();
            expr.closed = Some(bang.span);
            expr.span = Span::new(expr.span.start, bang.span.end);
        }

        Ok(expr)
    }

    fn parse_effect_row_term(&mut self) -> PResult<EffectRowTerm> {
        // effect row 的项语法上就是 `pathType`（`Pure` 不特判：普通单段路径）。
        let ty = self.parse_path_type()?;
        let TypeRefKind::Path { path, args } = ty.kind else {
            // invariant: parse_path_type 只产生 Path。
            return Err(self.err_expected_type(self.peek()));
        };
        Ok(EffectRowTerm {
            id: self.nid(),
            span: ty.span,
            path,
            args,
        })
    }
}
