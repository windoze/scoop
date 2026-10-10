use scoop_ast::{
    Assign, AssignmentOp, Block, CompoundAssignOp, Diagnostic, LocalDelegatedPropertyDecl, Pattern,
    SafetyMode, Span, Statement, StatementKind, ValDecl, VisibilitySyntax,
};

use crate::expr::loop_jump_expression_diagnostic;
use crate::lexer::{Token, TokenKind};
use crate::parser::Parser;

mod control_flow;
mod jumps;
mod when;

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
        if self.starts_context_declaration() {
            let function = self.parse_context_local_function()?;
            return Ok(Statement {
                span: function.span,
                kind: StatementKind::LocalFunction(function),
            });
        }
        if self.starts_unsupported_loop_label() {
            return self.reject_loop_label();
        }
        match &self.peek().kind {
            TokenKind::Fun if !matches!(self.tokens[self.pos + 1].kind, TokenKind::LParen) => {
                let function = self.parse_non_member_function(
                    Vec::new(),
                    VisibilitySyntax::Omitted,
                    crate::decl::FunctionContext::Local,
                )?;
                Ok(Statement {
                    span: function.span,
                    kind: StatementKind::LocalFunction(function),
                })
            }
            TokenKind::Suspend
                if matches!(self.tokens[self.pos + 1].kind, TokenKind::Fun)
                    && !matches!(self.tokens[self.pos + 2].kind, TokenKind::LParen) =>
            {
                let function = self.parse_non_member_function(
                    Vec::new(),
                    VisibilitySyntax::Omitted,
                    crate::decl::FunctionContext::Local,
                )?;
                Ok(Statement {
                    span: function.span,
                    kind: StatementKind::LocalFunction(function),
                })
            }
            TokenKind::Infix => {
                let function = self.parse_non_member_function(
                    Vec::new(),
                    VisibilitySyntax::Omitted,
                    crate::decl::FunctionContext::Local,
                )?;
                Ok(Statement {
                    span: function.span,
                    kind: StatementKind::LocalFunction(function),
                })
            }
            TokenKind::Ident(text)
                if text == "operator"
                    && !matches!(self.tokens[self.pos + 1].kind, TokenKind::LParen) =>
            {
                let function = self.parse_non_member_function(
                    Vec::new(),
                    VisibilitySyntax::Omitted,
                    crate::decl::FunctionContext::Local,
                )?;
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
            TokenKind::For => self.parse_for(),
            TokenKind::Break | TokenKind::Continue => self.parse_loop_jump(),
            TokenKind::Ident(text) if text == "do" => Err(Diagnostic::at(
                self.peek().span,
                "`do-while` loops are not supported in M22",
            )),
            TokenKind::Ident(text) if text == "typealias" => Err(Diagnostic::at(
                self.peek().span,
                "local typealias declarations are not supported in M22; only top-level non-generic typealias declarations are supported",
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
        if matches!(&self.peek().kind, TokenKind::Ident(text) if text == "by") {
            let Pattern::Binding(name) = target else {
                return Err(Diagnostic::at(
                    pattern_span(&target),
                    "a local delegated property requires a plain name",
                ));
            };
            let by = self.bump();
            let expression = self.parse_expr()?;
            let span = Span::new(keyword.span.start, expression.span().end);
            return Ok(Statement {
                span,
                kind: StatementKind::LocalDelegatedProperty(LocalDelegatedPropertyDecl {
                    mutable,
                    name,
                    ty,
                    expression,
                    by_span: by.span,
                    span,
                }),
            });
        }
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

    /// Parse the M22 payload-free, unlabelled `break` / `continue` statement.
    /// Expression positions are rejected in `parse_atom`; direct `if` and
    /// `when` branch bodies call this same routine and wrap its result in a
    /// synthetic block.
    pub(crate) fn parse_loop_jump(&mut self) -> Result<Statement, Diagnostic> {
        let keyword = self.bump();
        let name = match &keyword.kind {
            TokenKind::Break => "break",
            TokenKind::Continue => "continue",
            _ => unreachable!("parse_loop_jump starts on break or continue"),
        };
        self.reject_jump_label(&keyword, name)?;
        if !self.peek().newline_before
            && !matches!(
                self.peek().kind,
                TokenKind::Semicolon | TokenKind::RBrace | TokenKind::Eof | TokenKind::Else
            )
        {
            return Err(loop_jump_expression_diagnostic(name, keyword.span));
        }
        let kind = match name {
            "break" => StatementKind::Break,
            "continue" => StatementKind::Continue,
            _ => unreachable!("checked loop jump spelling"),
        };
        Ok(Statement {
            kind,
            span: keyword.span,
        })
    }

    fn reject_jump_label(&mut self, keyword: &Token, name: &str) -> Result<(), Diagnostic> {
        if !matches!(self.peek().kind, TokenKind::At) {
            return Ok(());
        }
        let at = self.bump();
        let end = if !self.peek().newline_before && matches!(self.peek().kind, TokenKind::Ident(_))
        {
            self.bump().span.end
        } else {
            at.span.end
        };
        Err(Diagnostic::at(
            Span::new(keyword.span.start, end),
            format!("`{name}` labels are not supported in M22"),
        ))
    }

    fn starts_unsupported_loop_label(&self) -> bool {
        if !matches!(self.peek().kind, TokenKind::Ident(_))
            || !matches!(
                self.tokens.get(self.pos + 1).map(|token| &token.kind),
                Some(TokenKind::At)
            )
        {
            return false;
        }
        self.tokens.get(self.pos + 2).is_some_and(|token| {
            matches!(&token.kind, TokenKind::While | TokenKind::For)
                || matches!(
                    &token.kind,
                    TokenKind::Ident(name) if name == "do"
                )
        })
    }

    fn reject_loop_label(&mut self) -> Result<Statement, Diagnostic> {
        let label = self.bump();
        let at = self.bump();
        Err(Diagnostic::at(
            Span::new(label.span.start, at.span.end),
            "loop labels are not supported in M22",
        ))
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
        scoop_ast::PlaceExpr::QualifiedInterfaceSuperProperty { span, .. } => *span,
    }
}

fn pattern_span(pattern: &Pattern) -> Span {
    match pattern {
        Pattern::Binding(name) => name.span,
        Pattern::Wildcard { span }
        | Pattern::Literal { span, .. }
        | Pattern::Positional { span, .. }
        | Pattern::Named { span, .. }
        | Pattern::Tuple { span, .. } => *span,
    }
}
