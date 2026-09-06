use super::*;

impl<'a> CfgLowerer<'a> {
    pub(super) fn lower_try(&mut self, try_: &'a smir::Try) {
        let enclosing = self.try_stack.last().copied();
        let unwind = self.new_block_with("try.unwind", None);
        let dispatch = self.new_block_with("try.dispatch", None);
        let handler_target = if try_.catches.is_empty() {
            None
        } else {
            let pad = self.new_block_with("try.handler_pad", None);
            let continuation =
                self.new_block_with("try.handler_cleanup", enclosing.map(|target| target.pad));
            Some(self.new_unwind_target(
                pad,
                continuation,
                enclosing.is_some_and(|target| target.handles_in_function),
            ))
        };
        let exit_pad = self.new_block_with("try.exit_pad", None);
        let exit_continuation =
            self.new_block_with("try.exit_cleanup", enclosing.map(|target| target.pad));
        let exit_target = self.new_unwind_target(
            exit_pad,
            exit_continuation,
            enclosing.is_some_and(|target| target.handles_in_function),
        );
        let end = self.new_block_with("try.end", enclosing.map(|target| target.pad));
        let own_target = self.new_unwind_target(unwind, dispatch, true);
        let cleanup_base = self.cleanup_depth();

        let body_entry = self.new_block_with("try.body", Some(unwind));
        self.seal(mir::Terminator::Goto(body_entry));
        self.try_stack.push(own_target);
        if let Some(finally) = &try_.finally_body {
            self.normal_cleanups.push(NormalCleanup::Finally {
                owner: own_target.owner,
                body: finally,
            });
        }
        self.enter(body_entry);
        self.lower_statements(&try_.body);
        let finally = try_.finally_body.as_deref();
        let mut end_reachable = false;
        if !self.current_sealed {
            end_reachable |= self.route_transfer(PendingTransfer::Fallthrough(ResumeTarget {
                block: end,
                cleanup_depth: cleanup_base,
            }));
        }
        if finally.is_some() {
            let cleanup = self.normal_cleanups.pop();
            assert!(matches!(
                cleanup,
                Some(NormalCleanup::Finally { owner, .. }) if owner == own_target.owner
            ));
        }
        let active = self.try_stack.pop();
        assert_eq!(
            active.map(|target| target.owner),
            Some(own_target.owner),
            "try body unwind scopes are lexically nested"
        );

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
            if let Some(finally) = finally {
                self.normal_cleanups.push(NormalCleanup::Finally {
                    owner: own_target.owner,
                    body: finally,
                });
            }
            self.normal_cleanups.push(NormalCleanup::EndCatch {
                owner: handler_target.owner,
            });
            self.lower_statements(&catch.body);
            if !self.current_sealed {
                end_reachable |= self.route_transfer(PendingTransfer::Fallthrough(ResumeTarget {
                    block: end,
                    cleanup_depth: cleanup_base,
                }));
            }
            self.normal_cleanups.truncate(cleanup_base.0);
            let active = self.try_stack.pop();
            assert_eq!(
                active.map(|target| target.owner),
                Some(handler_target.owner),
                "catch body unwind scopes are lexically nested"
            );
            self.enter(next);
        }

        self.try_stack.push(exit_target);
        let unmatched_cleanup_base = self.cleanup_depth();
        self.normal_cleanups.push(NormalCleanup::EndCatch {
            owner: exit_target.owner,
        });
        if let Some(finally) = finally {
            self.lower_statements(finally);
        }
        self.normal_cleanups.truncate(unmatched_cleanup_base.0);
        if !self.current_sealed {
            self.seal(mir::Terminator::Rethrow {
                unwind: Some(exit_target.pad),
            });
        }
        let active = self.try_stack.pop();
        assert_eq!(
            active.map(|target| target.owner),
            Some(exit_target.owner),
            "unmatched catch cleanup scopes are lexically nested"
        );

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
}
