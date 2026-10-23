use scoop_ast::{
    Assign, AssignmentOp, CompoundAssignOp, Diagnostic, LocalDelegatedPropertyDecl, Pattern,
    SafetyMode, Span, Statement, StatementKind, ValDecl, VisibilitySyntax,
};

use crate::lexer::{Token, TokenKind};
use crate::parser::Parser;

mod blocks;
mod control_flow;
mod declarations;
mod jumps;
mod when;

impl Parser {
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
        self.with_node(scoop_syntax::SyntaxKind::Statement, |parser| {
            parser.parse_documented_statement()
        })
    }

    fn parse_documented_statement(&mut self) -> Result<Statement, Diagnostic> {
        let start = self.pos;
        let statement = self.parse_statement_inner()?;
        if matches!(
            statement.kind,
            StatementKind::LocalFunction(_)
                | StatementKind::LocalType(_)
                | StatementKind::TypeAlias(_)
        ) {
            self.attach_documentation(start, statement.span);
        }
        Ok(statement)
    }

    fn parse_statement_inner(&mut self) -> Result<Statement, Diagnostic> {
        if self.starts_local_type() {
            let declaration = self.parse_local_type()?;
            return Ok(Statement {
                span: declaration.span(),
                kind: StatementKind::LocalType(declaration),
            });
        }
        if matches!(self.peek().kind, TokenKind::At)
            && let Some(statement) = self.parse_annotated_local_declaration()?
        {
            return Ok(statement);
        }
        if self.starts_context_declaration() {
            let function = self.parse_context_local_function()?;
            return Ok(Statement {
                span: function.span,
                kind: StatementKind::LocalFunction(function),
            });
        }
        let alias = matches!(&self.peek().kind, TokenKind::Ident(text) if text == "typealias")
            || matches!(&self.peek().kind, TokenKind::Ident(text) if matches!(text.as_str(), "public" | "internal" | "private" | "protected"))
                && matches!(&self.tokens[self.pos + 1].kind, TokenKind::Ident(text) if text == "typealias");
        if alias {
            let declaration = self.parse_local_type_alias()?;
            return Ok(Statement {
                span: declaration.span,
                kind: StatementKind::TypeAlias(declaration),
            });
        }
        if self.starts_loop_label() {
            return self.parse_labeled_loop();
        }
        let anonymous = matches!(self.peek().kind, TokenKind::Fun)
            || matches!(self.peek().kind, TokenKind::Suspend)
                && matches!(self.tokens[self.pos + 1].kind, TokenKind::Fun);
        let anonymous = anonymous && self.starts_anonymous_function();
        if !anonymous && self.starts_modified_local_function() {
            let function = self.parse_non_member_function(
                Vec::new(),
                VisibilitySyntax::Omitted,
                crate::decl::FunctionContext::Local,
                false,
            )?;
            return Ok(Statement {
                span: function.span,
                kind: StatementKind::LocalFunction(function),
            });
        }
        let statement_kind = self.peek().kind.clone();
        match &statement_kind {
            TokenKind::Fun if !anonymous => {
                let function = self.parse_non_member_function(
                    Vec::new(),
                    VisibilitySyntax::Omitted,
                    crate::decl::FunctionContext::Local,
                    false,
                )?;
                Ok(Statement {
                    span: function.span,
                    kind: StatementKind::LocalFunction(function),
                })
            }
            TokenKind::Suspend
                if matches!(self.tokens[self.pos + 1].kind, TokenKind::Fun) && !anonymous =>
            {
                let function = self.parse_non_member_function(
                    Vec::new(),
                    VisibilitySyntax::Omitted,
                    crate::decl::FunctionContext::Local,
                    false,
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
                    false,
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
                    false,
                )?;
                Ok(Statement {
                    span: function.span,
                    kind: StatementKind::LocalFunction(function),
                })
            }
            TokenKind::Val | TokenKind::Var => self.parse_val_decl(),
            TokenKind::If => self.parse_if(),
            TokenKind::When => self.parse_when(),
            TokenKind::While => self.parse_while(None),
            TokenKind::Return => self.parse_return(),
            TokenKind::LBrace => {
                let block = self.parse_block()?;
                Ok(Statement {
                    span: block.span,
                    kind: StatementKind::Block(block),
                })
            }
            TokenKind::At => self.parse_safety_block(),
            TokenKind::For => self.parse_for(None),
            TokenKind::Break | TokenKind::Continue => self.parse_loop_jump(),
            TokenKind::Do => self.parse_do_while(None),
            // `try` / `catch` / `finally` / `throw` are contextual: they
            // stay identifiers everywhere except statement position.
            TokenKind::Ident(text) if text == "try" => self.parse_try(),
            TokenKind::Ident(text) if text == "throw" => self.parse_throw(),
            _ => {
                let expr = self.parse_expr()?;
                if let Some(op) = assignment_op(&self.peek().kind) {
                    let target = Parser::expr_into_place(expr, "assignment target")?;
                    let start = place_span(&target).start;
                    let operator_span = self.bump().span;
                    let value = self.parse_expr()?;
                    let span = Span::new(start, value.span().end);
                    return Ok(Statement {
                        span,
                        kind: StatementKind::Assign(Assign {
                            target,
                            op,
                            operator_span: Some(operator_span),
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
        let mode = match annotation.name.single_text() {
            Some("Unsafe") => SafetyMode::Unsafe,
            Some("Safe") => SafetyMode::Safe,
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
        let header_start = self.checkpoint();
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
        let init = if self.fragment_is_complete(header_start) {
            self.parse_statement_operand(keyword.span.start, || {
                scoop_ast::RecoveredOperandContext::Initializer {
                    annotation: ty.clone().map(Box::new),
                }
            })?
        } else {
            self.parse_expr()?
        };
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

    fn starts_loop_label(&self) -> bool {
        if !matches!(
            self.peek().kind,
            TokenKind::Ident(_) | TokenKind::EscapedIdent(_)
        ) || !matches!(
            self.tokens.get(self.pos + 1).map(|token| &token.kind),
            Some(TokenKind::At)
        ) {
            return false;
        }
        self.tokens.get(self.pos + 2).is_some_and(|token| {
            matches!(
                &token.kind,
                TokenKind::While | TokenKind::For | TokenKind::Do
            )
        })
    }

    fn parse_labeled_loop(&mut self) -> Result<Statement, Diagnostic> {
        let label = self.expect_ident("loop label")?;
        self.expect("`@`", |kind| matches!(kind, TokenKind::At))?;
        match self.peek().kind {
            TokenKind::While => self.parse_while(Some(label)),
            TokenKind::Do => self.parse_do_while(Some(label)),
            TokenKind::For => self.parse_for(Some(label)),
            _ => self.unexpected("loop after label"),
        }
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
