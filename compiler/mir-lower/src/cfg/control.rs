use super::*;

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
            smir::StatementKind::While { cond, body } => self.lower_while(cond, body, span),
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
        cond: &smir::Expr,
        body: &'a [smir::Statement],
        span: Span,
    ) {
        let cond_block = self.new_block("while.cond");
        self.seal(mir::Terminator::Goto(cond_block));
        self.enter(cond_block);
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
            self.seal(mir::Terminator::Goto(cond_block));
        }
        self.enter(exit_block);
    }

    pub(super) fn lower_try(&mut self, try_: &'a smir::Try) {
        let enclosing = self.try_stack.last().copied();
        let unwind = self.new_block_with("try.unwind", None);
        let dispatch = self.new_block_with("try.dispatch", None);
        let handler_target = if try_.catches.is_empty() {
            None
        } else {
            Some(UnwindTarget {
                pad: self.new_block_with("try.handler_pad", None),
                continuation: self
                    .new_block_with("try.handler_cleanup", enclosing.map(|target| target.pad)),
                handles_in_function: enclosing.is_some_and(|target| target.handles_in_function),
            })
        };
        let exit_target = UnwindTarget {
            pad: self.new_block_with("try.exit_pad", None),
            continuation: self
                .new_block_with("try.exit_cleanup", enclosing.map(|target| target.pad)),
            handles_in_function: enclosing.is_some_and(|target| target.handles_in_function),
        };
        let end = self.new_block_with("try.end", enclosing.map(|target| target.pad));
        let own_target = UnwindTarget {
            pad: unwind,
            continuation: dispatch,
            handles_in_function: true,
        };

        let body_entry = self.new_block_with("try.body", Some(unwind));
        self.seal(mir::Terminator::Goto(body_entry));
        self.try_stack.push(own_target);
        if let Some(finally) = &try_.finally_body {
            self.return_cleanups.push(ReturnCleanup::Finally {
                owner_unwind: unwind,
                body: finally,
            });
        }
        self.enter(body_entry);
        self.lower_statements(&try_.body);
        self.try_stack.pop();
        let finally = try_.finally_body.as_deref();
        if finally.is_some() {
            self.return_cleanups.pop();
        }

        let mut end_reachable = false;
        if !self.current_sealed {
            self.ensure_unwind_context();
            if let Some(finally) = finally {
                self.lower_statements(finally);
            }
            if !self.current_sealed {
                self.seal(mir::Terminator::Goto(end));
                end_reachable = true;
            }
        }

        self.enter(unwind);
        self.push(
            mir::StatementKind::Eh(mir::EhStatement::LandingPad { cleanup: false }),
            synthetic_span(),
        );
        self.seal(mir::Terminator::Goto(dispatch));

        self.enter(dispatch);
        self.push(
            mir::StatementKind::Eh(mir::EhStatement::BeginCatch),
            synthetic_span(),
        );
        for catch in &try_.catches {
            let handler_target = handler_target.expect("a catch has a handler cleanup");
            let catch_block = self.new_block_with("try.catch", Some(handler_target.pad));
            let next = self.new_block_with("try.next", None);
            self.seal(mir::Terminator::Branch {
                cond: mir::Expr::new(
                    mir::Type::Boolean,
                    mir::ExprKind::IsInstance {
                        operand: Box::new(mir::Expr::new(
                            mir::Type::Any,
                            mir::ExprKind::CaughtException,
                        )),
                        check_ty: catch.ty.clone(),
                    },
                ),
                then_block: catch_block,
                else_block: next,
            });
            self.enter(catch_block);
            self.push(
                mir::StatementKind::ValDecl {
                    local: catch.local,
                    init: mir::Expr::new(
                        catch.ty.as_ref().clone(),
                        mir::ExprKind::Retype {
                            operand: Box::new(mir::Expr::new(
                                mir::Type::Any,
                                mir::ExprKind::CaughtException,
                            )),
                            ty: catch.ty.clone(),
                        },
                    ),
                },
                catch.span,
            );
            self.try_stack.push(handler_target);
            let cleanup_base = self.return_cleanups.len();
            if let Some(finally) = finally {
                self.return_cleanups.push(ReturnCleanup::Finally {
                    owner_unwind: unwind,
                    body: finally,
                });
            }
            self.return_cleanups.push(ReturnCleanup::EndCatch {
                cleanup_pad: handler_target.pad,
            });
            self.lower_statements(&catch.body);
            self.return_cleanups.truncate(cleanup_base);
            self.try_stack.pop();
            if !self.current_sealed {
                self.push(
                    mir::StatementKind::Eh(mir::EhStatement::EndCatch),
                    synthetic_span(),
                );
                if let Some(finally) = finally {
                    self.lower_statements(finally);
                }
                if !self.current_sealed {
                    self.seal(mir::Terminator::Goto(end));
                    end_reachable = true;
                }
            }
            self.enter(next);
        }

        self.try_stack.push(exit_target);
        let cleanup_base = self.return_cleanups.len();
        self.return_cleanups.push(ReturnCleanup::EndCatch {
            cleanup_pad: exit_target.pad,
        });
        if let Some(finally) = finally {
            self.lower_statements(finally);
        }
        self.return_cleanups.truncate(cleanup_base);
        if !self.current_sealed {
            self.seal(mir::Terminator::Rethrow {
                unwind: Some(exit_target.pad),
            });
        }
        self.try_stack.pop();

        if let Some(handler_target) = handler_target {
            self.enter(handler_target.pad);
            self.push(
                mir::StatementKind::Eh(mir::EhStatement::LandingPad {
                    cleanup: !handler_target.handles_in_function,
                }),
                synthetic_span(),
            );
            self.seal(mir::Terminator::Goto(handler_target.continuation));

            self.enter(handler_target.continuation);
            self.push(
                mir::StatementKind::Eh(mir::EhStatement::EndCatch),
                synthetic_span(),
            );
            if let Some(finally) = finally {
                self.lower_statements(finally);
            }
            if !self.current_sealed {
                match enclosing {
                    Some(target) => self.seal(mir::Terminator::Goto(target.continuation)),
                    None => self.seal(mir::Terminator::Resume),
                }
            }
        }

        self.enter(exit_target.pad);
        self.push(
            mir::StatementKind::Eh(mir::EhStatement::LandingPad {
                cleanup: !exit_target.handles_in_function,
            }),
            synthetic_span(),
        );
        self.seal(mir::Terminator::Goto(exit_target.continuation));

        self.enter(exit_target.continuation);
        self.push(
            mir::StatementKind::Eh(mir::EhStatement::EndCatch),
            synthetic_span(),
        );
        match enclosing {
            Some(target) => self.seal(mir::Terminator::Goto(target.continuation)),
            None => self.seal(mir::Terminator::Resume),
        }

        self.enter(end);
        if !end_reachable {
            self.seal(mir::Terminator::Unreachable);
        }
    }

    pub(super) fn emit_return_cleanups(&mut self) {
        let all = std::mem::take(&mut self.return_cleanups);
        let saved_try_stack = self.try_stack.clone();
        let mut remaining = all.clone();
        while let Some(cleanup) = remaining.pop() {
            if self.current_sealed {
                break;
            }
            self.return_cleanups = remaining.clone();
            match cleanup {
                ReturnCleanup::Finally { owner_unwind, body } => {
                    if let Some(pos) = self
                        .try_stack
                        .iter()
                        .rposition(|target| target.pad == owner_unwind)
                    {
                        self.try_stack.truncate(pos);
                    }
                    self.lower_statements(body);
                }
                ReturnCleanup::EndCatch { cleanup_pad } => {
                    self.push(
                        mir::StatementKind::Eh(mir::EhStatement::EndCatch),
                        synthetic_span(),
                    );
                    let active = self.try_stack.pop();
                    assert_eq!(
                        active.map(|target| target.pad),
                        Some(cleanup_pad),
                        "active catch cleanup nesting"
                    );
                }
            }
        }
        self.return_cleanups = all;
        self.try_stack = saved_try_stack;
    }
}
