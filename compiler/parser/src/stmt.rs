use scoop_ast::{
    Assign, AssignmentOp, Block, CompoundAssignOp, Diagnostic, SafetyMode, Span, Statement,
    StatementKind, ValDecl,
};

use crate::lexer::{Token, TokenKind};
use crate::parser::Parser;

mod control_flow;

impl Parser {
    pub(crate) fn parse_block(&mut self) -> Result<Block, Diagnostic> {
        let open = self.expect("`{`", |k| matches!(k, TokenKind::LBrace))?;
        self.parse_block_after_open(open)
    }

    /// Parse block items after the opening brace has already been consumed.
    /// Lambda parsing uses this after its optional parameter header.
    pub(crate) fn parse_block_after_open(&mut self, open: Token) -> Result<Block, Diagnostic> {
        let body_depth = self.brace_depth();
        let mut statements = Vec::new();
        loop {
            match self.peek().kind {
                TokenKind::RBrace => {
                    let close = self.bump();
                    return Ok(Block {
                        statements,
                        span: Span::new(open.span.start, close.span.end),
                    });
                }
                TokenKind::Eof => return self.unexpected("`}`"),
                _ => {
                    let start = self.pos;
                    match self.parse_statement() {
                        Ok(statement) => {
                            statements.push(statement);
                            if let Err(diagnostic) = self.expect_statement_end() {
                                self.diagnostics.push(diagnostic);
                                self.synchronize_body_item(body_depth, start);
                            }
                        }
                        Err(diagnostic) => {
                            if self.at_eof() {
                                return Err(diagnostic);
                            }
                            self.diagnostics.push(diagnostic);
                            self.synchronize_body_item(body_depth, start);
                        }
                    }
                }
            }
        }
    }

    /// A statement ends at a newline, at `}`, or at an explicit `;`.
    pub(crate) fn expect_statement_end(&mut self) -> Result<(), Diagnostic> {
        if matches!(self.peek().kind, TokenKind::Semicolon) {
            while matches!(self.peek().kind, TokenKind::Semicolon) {
                self.bump();
            }
            return Ok(());
        }
        let token = self.peek();
        if token.newline_before || matches!(token.kind, TokenKind::RBrace | TokenKind::Eof) {
            Ok(())
        } else {
            self.unexpected("`;` or newline after statement")
        }
    }

    fn parse_statement(&mut self) -> Result<Statement, Diagnostic> {
        match &self.peek().kind {
            TokenKind::Fun if !matches!(self.tokens[self.pos + 1].kind, TokenKind::LParen) => {
                let function = self
                    .parse_non_member_function(Vec::new(), crate::decl::FunctionContext::Local)?;
                Ok(Statement {
                    span: function.span,
                    kind: StatementKind::LocalFunction(function),
                })
            }
            TokenKind::Suspend
                if matches!(self.tokens[self.pos + 1].kind, TokenKind::Fun)
                    && !matches!(self.tokens[self.pos + 2].kind, TokenKind::LParen) =>
            {
                let function = self
                    .parse_non_member_function(Vec::new(), crate::decl::FunctionContext::Local)?;
                Ok(Statement {
                    span: function.span,
                    kind: StatementKind::LocalFunction(function),
                })
            }
            TokenKind::Infix => {
                let function = self
                    .parse_non_member_function(Vec::new(), crate::decl::FunctionContext::Local)?;
                Ok(Statement {
                    span: function.span,
                    kind: StatementKind::LocalFunction(function),
                })
            }
            TokenKind::Ident(text)
                if text == "operator"
                    && !matches!(self.tokens[self.pos + 1].kind, TokenKind::LParen) =>
            {
                let function = self
                    .parse_non_member_function(Vec::new(), crate::decl::FunctionContext::Local)?;
                Ok(Statement {
                    span: function.span,
                    kind: StatementKind::LocalFunction(function),
                })
            }
            TokenKind::Val | TokenKind::Var => self.parse_val_decl(),
            TokenKind::If => self.parse_if(),
            TokenKind::When => self.parse_when(),
            TokenKind::While => self.parse_while(),
            TokenKind::Return => self.parse_return(),
            TokenKind::LBrace => {
                let block = self.parse_block()?;
                Ok(Statement {
                    span: block.span,
                    kind: StatementKind::Block(block),
                })
            }
            TokenKind::At => self.parse_safety_block(),
            TokenKind::Ident(text) if text == "for" => Err(Diagnostic::at(
                self.peek().span,
                "`for` loops are not supported yet (milestone M5)",
            )),
            // `try` / `catch` / `finally` / `throw` are contextual: they
            // stay identifiers everywhere except statement position.
            TokenKind::Ident(text) if text == "try" => self.parse_try(),
            TokenKind::Ident(text) if text == "throw" => self.parse_throw(),
            _ => {
                let expr = self.parse_expr()?;
                if let Some(op) = assignment_op(&self.peek().kind) {
                    let target = Parser::expr_into_place(expr, "assignment target")?;
                    let start = place_span(&target).start;
                    self.bump();
                    let value = self.parse_expr()?;
                    let span = Span::new(start, value.span().end);
                    return Ok(Statement {
                        span,
                        kind: StatementKind::Assign(Assign {
                            target,
                            op,
                            value,
                            span,
                        }),
                    });
                }
                Ok(Statement {
                    span: expr.span(),
                    kind: StatementKind::Expr(expr),
                })
            }
        }
    }

    /// `@Unsafe { ... }` / `@Safe { ... }`. Declaration annotations remain
    /// generic, but annotated blocks have a closed syntax and a dedicated AST
    /// node so later stages never confuse them with calls.
    fn parse_safety_block(&mut self) -> Result<Statement, Diagnostic> {
        let annotations = self.parse_annotations()?;
        if annotations.len() != 1 {
            let span = Span::new(
                annotations
                    .first()
                    .map_or(self.peek().span.start, |a| a.span.start),
                annotations
                    .last()
                    .map_or(self.peek().span.end, |a| a.span.end),
            );
            return Err(Diagnostic::at(
                span,
                "a safety block requires exactly one of `@Unsafe` or `@Safe`",
            ));
        }
        let annotation = &annotations[0];
        if !annotation.args.is_empty() {
            return Err(Diagnostic::at(
                annotation.span,
                "safety block annotations do not accept arguments",
            ));
        }
        let mode = match annotation.name.text.as_str() {
            "Unsafe" => SafetyMode::Unsafe,
            "Safe" => SafetyMode::Safe,
            _ => {
                return Err(Diagnostic::at(
                    annotation.span,
                    "only `@Unsafe` or `@Safe` may annotate a block",
                ));
            }
        };
        if !matches!(self.peek().kind, TokenKind::LBrace) {
            return Err(Diagnostic::at(
                self.peek().span,
                "safety annotation must be followed by a block",
            ));
        }
        let block = self.parse_block()?;
        Ok(Statement {
            span: Span::new(annotation.span.start, block.span.end),
            kind: StatementKind::SafetyBlock { mode, block },
        })
    }

    /// `(val|var) <pattern> (: <type>)? = <expr>` — the initializer is
    /// mandatory. A plain identifier target is `Pattern::Binding`;
    /// destructuring uses tuple/struct patterns (spec 4.6). `var` with a
    /// pattern behaves like `val` (value types are immutable).
    fn parse_val_decl(&mut self) -> Result<Statement, Diagnostic> {
        let keyword = self.bump();
        let mutable = matches!(keyword.kind, TokenKind::Var);
        let target = self.parse_pattern()?;
        let ty = if matches!(self.peek().kind, TokenKind::Colon) {
            self.bump();
            Some(self.parse_type_ref()?)
        } else {
            None
        };
        self.expect("`=`", |k| matches!(k, TokenKind::Equal))?;
        let init = self.parse_expr()?;
        let span = Span::new(keyword.span.start, init.span().end);
        Ok(Statement {
            span,
            kind: StatementKind::ValDecl(ValDecl {
                mutable,
                target,
                ty,
                init,
                span,
            }),
        })
    }

    /// `return <expr>?` — a bare `return` (for `Unit` functions) ends at a
    /// newline, `;`, `}`, or end of file; anything else is the return value.
    fn parse_return(&mut self) -> Result<Statement, Diagnostic> {
        let keyword = self.bump(); // `return`
        let token = self.peek();
        let has_value = !token.newline_before
            && !matches!(
                token.kind,
                TokenKind::RBrace | TokenKind::Semicolon | TokenKind::Eof
            );
        let value = if has_value {
            Some(self.parse_expr()?)
        } else {
            None
        };
        let end = value
            .as_ref()
            .map(|expr| expr.span().end)
            .unwrap_or(keyword.span.end);
        Ok(Statement {
            span: Span::new(keyword.span.start, end),
            kind: StatementKind::Return { value },
        })
    }

    /// `throw <expr>` — the operand is a full expression running to the
    /// end of the statement (`throw f(1) + 2` throws `f(1) + 2`).
    fn parse_throw(&mut self) -> Result<Statement, Diagnostic> {
        let keyword = self.bump(); // `throw`
        let value = self.parse_expr()?;
        let span = Span::new(keyword.span.start, value.span().end);
        Ok(Statement {
            span,
            kind: StatementKind::Throw(value),
        })
    }
}

fn assignment_op(kind: &TokenKind) -> Option<AssignmentOp> {
    Some(match kind {
        TokenKind::Equal => AssignmentOp::Assign,
        TokenKind::PlusEqual => AssignmentOp::Compound(CompoundAssignOp::Add),
        TokenKind::MinusEqual => AssignmentOp::Compound(CompoundAssignOp::Sub),
        TokenKind::StarEqual => AssignmentOp::Compound(CompoundAssignOp::Mul),
        TokenKind::SlashEqual => AssignmentOp::Compound(CompoundAssignOp::Div),
        TokenKind::PercentEqual => AssignmentOp::Compound(CompoundAssignOp::Rem),
        _ => return None,
    })
}

fn place_span(place: &scoop_ast::PlaceExpr) -> Span {
    match place {
        scoop_ast::PlaceExpr::Name(name) => name.span,
        scoop_ast::PlaceExpr::Field { span, .. } | scoop_ast::PlaceExpr::Index { span, .. } => {
            *span
        }
    }
}
