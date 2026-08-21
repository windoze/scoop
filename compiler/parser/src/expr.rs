//! Expression parsing: precedence climbing over the M2 operator set.
//!
//! Precedence, low to high: `||` < `&&` < `== !=` < `< <= > >=` < `+ -` <
//! `* /` < unary `- !` < postfix `.name` / `._n` < atoms. All binary
//! operators are left-associative.

use scoop_ast::{BinOp, CallExpr, Diagnostic, Expr, FieldAccess, FieldSelector, Ident, Span, UnOp};

use crate::lexer::TokenKind;
use crate::parser::Parser;

/// Maps an infix operator token to its AST operator and precedence level
/// (higher binds tighter); `None` for non-operator tokens.
fn binary_op(kind: &TokenKind) -> Option<(BinOp, u8)> {
    let (op, precedence) = match kind {
        TokenKind::PipePipe => (BinOp::Or, 1),
        TokenKind::AmpAmp => (BinOp::And, 2),
        TokenKind::EqualEqual => (BinOp::Eq, 3),
        TokenKind::BangEqual => (BinOp::Ne, 3),
        TokenKind::Less => (BinOp::Lt, 4),
        TokenKind::LessEqual => (BinOp::Le, 4),
        TokenKind::Greater => (BinOp::Gt, 4),
        TokenKind::GreaterEqual => (BinOp::Ge, 4),
        TokenKind::Plus => (BinOp::Add, 5),
        TokenKind::Minus => (BinOp::Sub, 5),
        TokenKind::Star => (BinOp::Mul, 6),
        TokenKind::Slash => (BinOp::Div, 6),
        _ => return None,
    };
    Some((op, precedence))
}

/// `._<n>` tuple selectors lex as identifiers (`_1`, `_2`, ...). Returns
/// `Some(n)` when `text` is `_` followed by digits only.
fn tuple_index(text: &str) -> Option<u32> {
    let digits = text.strip_prefix('_')?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

impl Parser {
    pub(crate) fn parse_expr(&mut self) -> Result<Expr, Diagnostic> {
        self.parse_binary(1)
    }

    fn parse_binary(&mut self, min_precedence: u8) -> Result<Expr, Diagnostic> {
        let mut lhs = self.parse_unary()?;
        while let Some((op, precedence)) = binary_op(&self.peek().kind) {
            if precedence < min_precedence {
                break;
            }
            self.bump();
            let rhs = self.parse_binary(precedence + 1)?;
            lhs = Expr::Binary {
                op,
                span: Span::new(lhs.span().start, rhs.span().end),
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.peek().clone();
        let op = match token.kind {
            TokenKind::Minus => UnOp::Neg,
            TokenKind::Bang => UnOp::Not,
            _ => return self.parse_postfix(),
        };
        self.pos += 1;
        let operand = self.parse_unary()?;
        Ok(Expr::Unary {
            op,
            span: Span::new(token.span.start, operand.span().end),
            operand: Box::new(operand),
        })
    }

    fn parse_postfix(&mut self) -> Result<Expr, Diagnostic> {
        let mut receiver = self.parse_atom()?;
        while matches!(self.peek().kind, TokenKind::Dot) {
            self.bump();
            let token = self.peek().clone();
            let TokenKind::Ident(text) = token.kind else {
                return self.unexpected("field name or tuple index");
            };
            self.pos += 1;
            let selector = match tuple_index(&text) {
                Some(index) => FieldSelector::Index(index, token.span),
                None => FieldSelector::Name(Ident {
                    text,
                    span: token.span,
                }),
            };
            receiver = Expr::FieldAccess(FieldAccess {
                span: Span::new(receiver.span().start, token.span.end),
                receiver: Box::new(receiver),
                selector,
            });
        }
        Ok(receiver)
    }

    fn parse_atom(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.peek().clone();
        match token.kind {
            TokenKind::Str(value) => {
                self.pos += 1;
                Ok(Expr::StringLiteral {
                    value,
                    span: token.span,
                })
            }
            TokenKind::Int(value) => {
                self.pos += 1;
                Ok(Expr::IntLiteral {
                    value,
                    span: token.span,
                })
            }
            TokenKind::True => {
                self.pos += 1;
                Ok(Expr::BoolLiteral {
                    value: true,
                    span: token.span,
                })
            }
            TokenKind::False => {
                self.pos += 1;
                Ok(Expr::BoolLiteral {
                    value: false,
                    span: token.span,
                })
            }
            TokenKind::Ident(text) => {
                if text == "when" {
                    return Err(Diagnostic::at(
                        token.span,
                        "`when` expressions are not supported yet (milestone M2)",
                    ));
                }
                self.pos += 1;
                // `Unit` is an ordinary identifier; in expression position
                // it denotes the unit value (spec section 4.3).
                if text == "Unit" {
                    return Ok(Expr::UnitLiteral { span: token.span });
                }
                let ident = Ident {
                    text,
                    span: token.span,
                };
                if matches!(self.peek().kind, TokenKind::LParen) {
                    return self.parse_call(ident);
                }
                Ok(Expr::Var(ident))
            }
            TokenKind::LParen => self.parse_paren_expr(),
            _ => self.unexpected("expression"),
        }
    }

    /// `callee(args...)` — `callee` is already consumed. Struct
    /// construction shares this syntax in M2 (`Point(1, 2)`); hir-lower
    /// tells function calls and struct constructions apart.
    fn parse_call(&mut self, callee: Ident) -> Result<Expr, Diagnostic> {
        self.expect("`(`", |k| matches!(k, TokenKind::LParen))?;
        let mut args = Vec::new();
        if !matches!(self.peek().kind, TokenKind::RParen) {
            loop {
                args.push(self.parse_expr()?);
                if matches!(self.peek().kind, TokenKind::Comma) {
                    self.bump();
                } else {
                    break;
                }
            }
        }
        let close = self.expect("`)`", |k| matches!(k, TokenKind::RParen))?;
        let span = Span::new(callee.span.start, close.span.end);
        Ok(Expr::Call(CallExpr { callee, args, span }))
    }

    /// `( ... )` disambiguation (spec section 4.3): `()` is the unit
    /// literal, `(e)` is just `e` (no node), `(e,)` a 1-tuple, and
    /// `(e1, e2, ...)` a tuple literal.
    fn parse_paren_expr(&mut self) -> Result<Expr, Diagnostic> {
        let open = self.bump(); // `(`
        if matches!(self.peek().kind, TokenKind::RParen) {
            let close = self.bump();
            return Ok(Expr::UnitLiteral {
                span: Span::new(open.span.start, close.span.end),
            });
        }
        let first = self.parse_expr()?;
        if !matches!(self.peek().kind, TokenKind::Comma) {
            self.expect("`)`", |k| matches!(k, TokenKind::RParen))?;
            return Ok(first);
        }
        let mut elements = vec![first];
        while matches!(self.peek().kind, TokenKind::Comma) {
            self.bump();
            if matches!(self.peek().kind, TokenKind::RParen) {
                break; // trailing comma: `(e,)` / `(e1, e2,)`
            }
            elements.push(self.parse_expr()?);
        }
        let close = self.expect("`)`", |k| matches!(k, TokenKind::RParen))?;
        Ok(Expr::TupleLiteral {
            elements,
            span: Span::new(open.span.start, close.span.end),
        })
    }
}
