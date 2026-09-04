use super::*;

impl Lowerer {
    /// Lower a block in a fresh scope: declarations inside are not
    /// visible after the block ends.
    pub(crate) fn lower_block(&mut self, block: &ast::Block) -> Vec<hir::Statement> {
        self.push_scope();
        let mut statements = Vec::new();
        for statement in &block.statements {
            self.lower_statement(statement, &mut statements);
        }
        self.pop_scope();
        statements
    }

    /// Whether the trailing value of a block intrinsically needs an
    /// expected type (`None`, an empty array, ...). Structured expressions
    /// run their own branch inference and are therefore not deferred here.
    pub(super) fn value_block_requires_expected(&self, block: &ast::Block) -> bool {
        match block.statements.last().map(|statement| &statement.kind) {
            Some(ast::StatementKind::Expr(expr)) => self.expr_requires_expected_type(expr),
            _ => false,
        }
    }

    /// Lower a control-expression branch in a fresh scope. The final
    /// expression is removed from statement position and returned as the
    /// block's value. A final legacy control statement is interpreted as a
    /// value too, so nested `if` / `when` / `try` works even though their
    /// standalone parser forms remain statement nodes.
    pub(crate) fn lower_value_block(
        &mut self,
        block: &ast::Block,
        expected: Option<TypeId>,
    ) -> Option<ValueBlock> {
        self.push_scope();
        let lowered = (|| {
            let mut statements = Vec::new();
            let value = if let Some((last, prefix)) = block.statements.split_last() {
                for statement in prefix {
                    self.lower_statement(statement, &mut statements);
                }
                match &last.kind {
                    ast::StatementKind::Expr(expr) => {
                        let mut tail = Vec::new();
                        let value = self.lower_expr(expr, &mut tail, expected)?;
                        statements.extend(tail);
                        Some(value)
                    }
                    ast::StatementKind::If(if_) => {
                        self.lower_if_expression(if_, &mut statements, expected)
                    }
                    ast::StatementKind::When(when) => {
                        self.lower_when_expression(when, &mut statements, expected)
                    }
                    ast::StatementKind::Try(try_) => {
                        self.lower_try_expression(try_, &mut statements, expected)
                    }
                    _ => {
                        self.lower_statement(last, &mut statements);
                        None
                    }
                }
            } else {
                None
            };
            let value = if statements_can_fall_through(&statements) {
                Some(value.unwrap_or(hir::Expr {
                    kind: hir::ExprKind::UnitLiteral,
                    ty: self.unit,
                    span: block.span,
                    origin: self.expression_origin(block.span),
                }))
            } else {
                None
            };
            Some(ValueBlock { statements, value })
        })();
        self.pop_scope();
        lowered
    }

    /// Infer a provisional branch hint from already-lowered normal paths.
    fn value_block_hint<'a>(
        &mut self,
        blocks: impl Iterator<Item = &'a ValueBlock>,
    ) -> Option<TypeId> {
        let types: Vec<_> = blocks
            .filter_map(|block| block.value.as_ref().map(|value| value.ty))
            .collect();
        (!types.is_empty()).then(|| self.least_upper_bound(&types))
    }

    /// Type-check and materialize a structured expression's branch result.
    /// Each normal non-Unit branch assigns the same hidden local; Unit tails
    /// are merely evaluated for side effects.
    pub(super) fn finish_control_value(
        &mut self,
        kind: &str,
        span: Span,
        expected: Option<TypeId>,
        blocks: &mut [&mut ValueBlock],
    ) -> Option<hir::Expr> {
        let value_types: Vec<_> = blocks
            .iter()
            .filter_map(|block| block.value.as_ref().map(|value| value.ty))
            .collect();
        let result_ty = expected.unwrap_or_else(|| {
            if value_types.is_empty() {
                self.unit
            } else {
                self.least_upper_bound(&value_types)
            }
        });
        for block in blocks.iter() {
            if let Some(value) = &block.value
                && !self.is_subtype(value.ty, result_ty)
            {
                let expected = self.type_name(result_ty);
                let found = self.type_name(value.ty);
                let message = self.with_nominal_invariance_detail(
                    format!("{kind} branch result must be of type {expected}, found {found}"),
                    value.ty,
                    result_ty,
                );
                self.error(value.span, message);
                return None;
            }
        }

        if self.types_equal(result_ty, self.unit) {
            for block in blocks.iter_mut() {
                if let Some(value) = block.value.take()
                    && !matches!(value.kind, hir::ExprKind::UnitLiteral)
                {
                    block.statements.push(hir::Statement {
                        span: value.span,
                        kind: hir::StatementKind::Expr(value),
                    });
                }
            }
            return Some(hir::Expr {
                kind: hir::ExprKind::UnitLiteral,
                ty: self.unit,
                span,
                origin: self.expression_origin(span),
            });
        }

        let result = self.alloc_hidden_result(result_ty);
        for block in blocks.iter_mut() {
            if let Some(value) = block.value.take() {
                let value = self.adapt_to(value, result_ty);
                block.statements.push(hir::Statement {
                    span: value.span,
                    kind: hir::StatementKind::Assign {
                        target: hir::AssignTarget::Local(result),
                        value,
                    },
                });
            }
        }
        Some(hir::Expr {
            kind: hir::ExprKind::Local(result),
            ty: result_ty,
            span,
            origin: self.expression_origin(span),
        })
    }

    /// `if` in value position. The condition prelude and the structured HIR
    /// statement are appended to the caller's desugaring sink; the returned
    /// expression reads the branch-result local.
    pub(crate) fn lower_if_expression(
        &mut self,
        if_: &ast::If,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let Some(else_block) = &if_.else_block else {
            self.error(
                if_.span,
                "if expression requires an `else` branch".to_string(),
            );
            return None;
        };
        let mut cond_sink = Vec::new();
        let before = self.diagnostics.len();
        let cond = self.lower_condition(&if_.cond, "if", &mut cond_sink)?;
        let (then_narrowings, else_narrowings) = if self.diagnostics.len() == before {
            (
                self.resolve_smart_casts(&if_.cond, true),
                self.resolve_smart_casts(&if_.cond, false),
            )
        } else {
            (Vec::new(), Vec::new())
        };

        let defer_then = expected.is_none() && self.value_block_requires_expected(&if_.then_block);
        let defer_else = expected.is_none() && self.value_block_requires_expected(else_block);
        let mut then_value = if defer_then {
            None
        } else {
            Some(self.with_smart_casts(then_narrowings.clone(), |this| {
                this.lower_value_block(&if_.then_block, expected)
            })?)
        };
        let mut else_value = if defer_else {
            None
        } else {
            Some(self.with_smart_casts(else_narrowings.clone(), |this| {
                this.lower_value_block(else_block, expected)
            })?)
        };
        let hint = self.value_block_hint(then_value.iter().chain(else_value.iter()));
        if then_value.is_none() {
            then_value = Some(self.with_smart_casts(then_narrowings, |this| {
                this.lower_value_block(&if_.then_block, hint)
            })?);
        }
        if else_value.is_none() {
            else_value = Some(self.with_smart_casts(else_narrowings, |this| {
                this.lower_value_block(else_block, hint)
            })?);
        }
        let mut then_value = then_value.expect("both branches were lowered");
        let mut else_value = else_value.expect("both branches were lowered");
        let result = self.finish_control_value(
            "if",
            if_.span,
            expected,
            &mut [&mut then_value, &mut else_value],
        )?;
        sink.extend(cond_sink);
        sink.push(hir::Statement {
            span: if_.span,
            kind: hir::StatementKind::If {
                cond,
                then_body: then_value.statements,
                else_body: Some(else_value.statements),
            },
        });
        Some(result)
    }
}
