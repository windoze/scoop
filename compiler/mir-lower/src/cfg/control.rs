use super::*;

mod exceptions;

impl<'a> CfgLowerer<'a> {
    pub(super) fn active_unwind(&self) -> Option<mir::BlockId> {
        self.try_stack.last().map(|target| target.pad)
    }

    pub(super) fn new_block_with(
        &mut self,
        base: &str,
        unwind: Option<mir::BlockId>,
    ) -> mir::BlockId {
        self.block_count += 1;
        self.blocks.alloc(mir::BasicBlock {
            name: format!("{base}.{}", self.block_count),
            statements: Vec::new(),
            terminator: mir::Terminator::Unreachable,
            unwind,
        })
    }

    pub(super) fn new_block(&mut self, base: &str) -> mir::BlockId {
        self.new_block_with(base, self.active_unwind())
    }

    pub(super) fn enter(&mut self, block: mir::BlockId) {
        self.current = block;
        self.current_sealed = false;
    }

    pub(super) fn seal(&mut self, terminator: mir::Terminator) {
        assert!(
            !self.current_sealed,
            "a MIR block is terminated exactly once"
        );
        self.blocks[self.current].terminator = terminator;
        self.current_sealed = true;
    }

    /// A MIR block has one explicit unwind successor. Split whenever the
    /// lexical exception context changes after the block has received work.
    pub(super) fn ensure_unwind_context(&mut self) {
        if self.current_sealed {
            return;
        }
        let desired = self.active_unwind();
        if self.blocks[self.current].unwind == desired {
            return;
        }
        if self.blocks[self.current].statements.is_empty() {
            self.blocks[self.current].unwind = desired;
            return;
        }
        let next = self.new_block_with("scope", desired);
        self.seal(mir::Terminator::Goto(next));
        self.enter(next);
    }

    pub(super) fn push(&mut self, kind: mir::StatementKind, span: Span) {
        self.ensure_unwind_context();
        self.blocks[self.current]
            .statements
            .push(mir::Statement { kind, span });
    }

    pub(super) fn lower_statements(&mut self, statements: &'a [smir::Statement]) {
        for statement in statements {
            if self.current_sealed {
                break;
            }
            self.ensure_unwind_context();
            self.lower_statement(statement);
        }
    }

    pub(super) fn lower_statement(&mut self, statement: &'a smir::Statement) {
        let span = statement.span;
        match &statement.kind {
            smir::StatementKind::Unreachable => self.seal(mir::Terminator::Unreachable),
            smir::StatementKind::Expr(expr) => {
                if let Some(message) = trap_message(expr) {
                    self.seal(mir::Terminator::Trap { message });
                } else {
                    let is_call = matches!(expr.kind, smir::ExprKind::Call(_));
                    let expr = self.lower_expr(expr, span);
                    if !is_call && !matches!(expr.kind, mir::ExprKind::UnitLiteral) {
                        self.push(mir::StatementKind::Expr(expr), span);
                    }
                }
            }
            smir::StatementKind::ValDecl { local, init } => {
                if let smir::ExprKind::Call(call) = &init.kind
                    && call.return_ty != mir::Type::Unit
                    && call.return_ty == init.ty
                {
                    self.lower_call(call, Some(*local), span);
                    return;
                }
                let init = self.lower_expr(init, span);
                self.push(
                    mir::StatementKind::ValDecl {
                        local: *local,
                        init,
                    },
                    span,
                );
            }
            smir::StatementKind::Assign { local, value } => {
                let value = self.lower_expr(value, span);
                self.push(
                    mir::StatementKind::Assign {
                        local: *local,
                        value,
                    },
                    span,
                );
            }
            smir::StatementKind::GlobalAssign { global, value } => {
                let value = self.lower_expr(value, statement.span);
                self.push(
                    mir::StatementKind::GlobalAssign {
                        global: *global,
                        value,
                    },
                    statement.span,
                );
            }
            smir::StatementKind::ArraySet {
                array_type,
                array,
                index,
                value,
            } => {
                let array = self.lower_expr(array, span);
                let index = self.lower_expr(index, span);
                let value = self.lower_expr(value, span);
                self.push(
                    mir::StatementKind::ArraySet {
                        array_type: *array_type,
                        array,
                        index,
                        value,
                    },
                    span,
                );
            }
            smir::StatementKind::FieldSet {
                object,
                index,
                value,
            } => {
                let object = self.lower_expr(object, span);
                let value = self.lower_expr(value, span);
                self.push(
                    mir::StatementKind::FieldSet {
                        object,
                        index: *index,
                        value,
                    },
                    span,
                );
            }
            smir::StatementKind::Return { value } => self.lower_return(value.as_ref(), span),
            smir::StatementKind::If {
                cond,
                then_body,
                else_body,
            } => self.lower_if(cond, then_body, else_body.as_deref(), span),
            smir::StatementKind::PatternDecision(decision) => {
                self.lower_pattern_decision(decision, span)
            }
            smir::StatementKind::While {
                condition_setup,
                cond,
                body,
            } => self.lower_while(condition_setup, cond, body, span),
            smir::StatementKind::Try(try_) => self.lower_try(try_),
            smir::StatementKind::Throw(exception) => {
                let exception = self.lower_expr(exception, span);
                let unwind = self.active_unwind();
                self.seal(mir::Terminator::Throw { exception, unwind });
            }
        }
    }

    pub(super) fn lower_return(&mut self, value: Option<&smir::Expr>, span: Span) {
        let mut result = value.map(|value| self.lower_expr(value, span));
        if !self.return_cleanups.is_empty() {
            if self.return_ty == mir::Type::Unit {
                if let Some(value) = result.take() {
                    self.push(mir::StatementKind::Expr(value), span);
                }
            } else {
                let local = self.new_hidden("return", self.return_ty.clone());
                self.push(
                    mir::StatementKind::ValDecl {
                        local,
                        init: result
                            .take()
                            .expect("non-Unit returns carry a value before MIR CFG lowering"),
                    },
                    span,
                );
                result = Some(mir::Expr::new(
                    self.return_ty.clone(),
                    mir::ExprKind::Local(local),
                ));
            }
            self.emit_return_cleanups();
            if self.current_sealed {
                return;
            }
        }
        self.seal(mir::Terminator::Return { value: result });
    }

    pub(super) fn lower_if(
        &mut self,
        cond: &smir::Expr,
        then_body: &'a [smir::Statement],
        else_body: Option<&'a [smir::Statement]>,
        span: Span,
    ) {
        let cond = self.lower_expr(cond, span);
        let then_block = self.new_block("if.then");
        let else_block = else_body.map(|_| self.new_block("if.else"));
        let merge_block = self.new_block("if.merge");
        self.seal(mir::Terminator::Branch {
            cond,
            then_block,
            else_block: else_block.unwrap_or(merge_block),
        });
        self.enter(then_block);
        self.lower_statements(then_body);
        if !self.current_sealed {
            self.seal(mir::Terminator::Goto(merge_block));
        }
        if let (Some(else_body), Some(else_block)) = (else_body, else_block) {
            self.enter(else_block);
            self.lower_statements(else_body);
            if !self.current_sealed {
                self.seal(mir::Terminator::Goto(merge_block));
            }
        }
        self.enter(merge_block);
    }

    pub(super) fn lower_while(
        &mut self,
        condition_setup: &'a [smir::Statement],
        cond: &smir::Expr,
        body: &'a [smir::Statement],
        span: Span,
    ) {
        let header = self.new_block("while.cond");
        self.seal(mir::Terminator::Goto(header));
        self.enter(header);
        self.lower_statements(condition_setup);
        if self.current_sealed {
            return;
        }
        let cond = self.lower_expr(cond, span);
        let body_block = self.new_block("while.body");
        let exit_block = self.new_block("while.exit");
        self.seal(mir::Terminator::Branch {
            cond,
            then_block: body_block,
            else_block: exit_block,
        });
        self.enter(body_block);
        self.lower_statements(body);
        if !self.current_sealed {
            self.seal(mir::Terminator::Goto(header));
        }
        self.enter(exit_block);
    }
}
