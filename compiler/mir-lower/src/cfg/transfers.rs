use super::*;

impl<'a> CfgLowerer<'a> {
    pub(super) fn cleanup_depth(&self) -> CleanupDepth {
        CleanupDepth(self.normal_cleanups.len())
    }

    pub(super) fn new_unwind_target(
        &mut self,
        pad: mir::BlockId,
        continuation: mir::BlockId,
        handles_in_function: bool,
    ) -> UnwindTarget {
        let owner = UnwindScopeId(self.unwind_scope_count);
        self.unwind_scope_count = self
            .unwind_scope_count
            .checked_add(1)
            .expect("one callable cannot contain u32::MAX unwind scopes");
        UnwindTarget {
            owner,
            pad,
            continuation,
            handles_in_function,
        }
    }

    pub(super) fn prepare_return(
        &mut self,
        value: Option<&smir::Expr>,
        span: Span,
    ) -> ReturnPayload {
        if self.return_ty == mir::Type::Unit {
            assert!(
                value.is_none(),
                "Unit returns are bare before MIR CFG lowering"
            );
            return ReturnPayload::Unit;
        }

        let value = self.lower_expr(
            value.expect("non-Unit returns carry a value before MIR CFG lowering"),
            span,
        );
        if self.normal_cleanups.is_empty() {
            return ReturnPayload::Value(value);
        }
        let local = self.new_hidden("return", self.return_ty.clone());
        self.push(mir::StatementKind::ValDecl { local, init: value }, span);
        ReturnPayload::Value(mir::Expr::new(
            self.return_ty.clone(),
            mir::ExprKind::Local(local),
        ))
    }

    /// Route one normal transfer through exactly the cleanup suffix between
    /// its current lexical position and its typed destination.
    ///
    /// Before a cleanup body is entered, the visible cleanup stack is cut to
    /// the cursor immediately outside that cleanup. A new abrupt transfer in
    /// a finally therefore starts after the active finally and cannot reenter
    /// it. If the finally falls through, this invocation resumes the saved
    /// transfer at the next outer cleanup.
    pub(super) fn route_transfer(&mut self, transfer: PendingTransfer) -> bool {
        let stop = transfer.cleanup_depth();
        let all = std::mem::take(&mut self.normal_cleanups);
        assert!(
            stop.0 <= all.len(),
            "a normal transfer target keeps a prefix of the active cleanup stack"
        );
        let saved_try_stack = self.try_stack.clone();
        self.normal_cleanups = all.clone();
        let mut cursor = CleanupCursor::new(CleanupDepth(all.len()), stop);

        while let Some(index) = cursor.take_next() {
            if self.current_sealed {
                break;
            }
            self.normal_cleanups.truncate(index);
            match all[index] {
                NormalCleanup::Finally { owner, body } => {
                    self.leave_unwind_scope(owner);
                    self.lower_statements(body);
                }
                NormalCleanup::EndCatch { owner } => {
                    self.leave_nested_unwind_scopes(owner);
                    self.push(
                        mir::StatementKind::Eh(mir::EhStatement::EndCatch),
                        synthetic_span(),
                    );
                    let active = self.try_stack.pop();
                    assert_eq!(
                        active.map(|target| target.owner),
                        Some(owner),
                        "a normal catch exit ends its exact active unwind scope"
                    );
                }
            }
        }

        let reached_target = if self.current_sealed {
            false
        } else {
            self.emit_transfer(transfer);
            true
        };
        self.normal_cleanups = all;
        self.try_stack = saved_try_stack;
        reached_target
    }

    fn leave_unwind_scope(&mut self, owner: UnwindScopeId) {
        if let Some(position) = self
            .try_stack
            .iter()
            .rposition(|target| target.owner == owner)
        {
            self.try_stack.truncate(position);
        }
    }

    fn leave_nested_unwind_scopes(&mut self, owner: UnwindScopeId) {
        let position = self
            .try_stack
            .iter()
            .rposition(|target| target.owner == owner)
            .expect("an EndCatch cleanup owns an active unwind scope");
        self.try_stack.truncate(position + 1);
    }

    fn emit_transfer(&mut self, transfer: PendingTransfer) {
        let terminator = match transfer {
            PendingTransfer::Fallthrough(target) => mir::Terminator::Goto(target.block),
            PendingTransfer::Return(ReturnPayload::Unit) => mir::Terminator::Return { value: None },
            PendingTransfer::Return(ReturnPayload::Value(value)) => {
                mir::Terminator::Return { value: Some(value) }
            }
            PendingTransfer::Break(target) => mir::Terminator::Goto(target.block),
            PendingTransfer::Continue(target) => mir::Terminator::Goto(target.block),
        };
        self.seal(terminator);
    }

    pub(super) fn push_loop_target(
        &mut self,
        source: smir::LoopId,
        header: mir::BlockId,
        exit: mir::BlockId,
    ) {
        assert!(
            self.loop_targets
                .iter()
                .all(|target| target.source != source),
            "one structured loop identity has one active CFG target"
        );
        let cleanup_depth = self.cleanup_depth();
        self.loop_targets.push(LoopTarget {
            source,
            exit: LoopExitTarget {
                block: exit,
                cleanup_depth,
            },
            header: LoopHeaderTarget {
                block: header,
                cleanup_depth,
            },
            break_reachable: false,
        });
    }

    pub(super) fn pop_loop_target(&mut self, source: smir::LoopId) -> LoopTarget {
        let target = self
            .loop_targets
            .pop()
            .expect("an active structured loop has a CFG target");
        assert_eq!(
            target.source, source,
            "CFG loop targets are lexically nested"
        );
        target
    }

    pub(super) fn lower_break(&mut self, source: smir::LoopId) {
        let index = self
            .loop_targets
            .len()
            .checked_sub(1)
            .expect("structured MIR binds break inside an active loop");
        assert_eq!(
            self.loop_targets[index].source, source,
            "unlabelled structured break targets the lexical innermost loop"
        );
        let target = self.loop_targets[index].exit;
        if self.route_transfer(PendingTransfer::Break(target)) {
            self.loop_targets[index].break_reachable = true;
        }
    }

    pub(super) fn lower_continue(&mut self, source: smir::LoopId) {
        let target = self
            .loop_targets
            .last()
            .expect("structured MIR binds continue inside an active loop");
        assert_eq!(
            target.source, source,
            "unlabelled structured continue targets the lexical innermost loop"
        );
        let target = target.header;
        self.route_transfer(PendingTransfer::Continue(target));
    }
}
