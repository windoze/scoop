use super::*;

impl<'a> CfgLowerer<'a> {
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
