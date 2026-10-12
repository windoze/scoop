use super::*;

impl<'a> CfgLowerer<'a> {
    pub(super) fn lower_while(
        &mut self,
        target: smir::LoopId,
        condition_setup: &'a [smir::Statement],
        cond: &smir::Expr,
        body: &'a [smir::Statement],
        span: Span,
    ) {
        let header = self.new_block("while.cond");
        let exit_block = self.new_block("while.exit");
        self.seal(mir::Terminator::Goto(header));
        self.push_loop_target(target, header, exit_block);
        self.enter(header);
        self.lower_statements(condition_setup);
        if self.current_sealed {
            let target = self.pop_loop_target(target);
            if target.break_reachable {
                self.enter(exit_block);
            }
            return;
        }
        let Some(cond) = self.lower_expr(cond, span) else {
            let target = self.pop_loop_target(target);
            if target.break_reachable {
                self.enter(exit_block);
            }
            return;
        };
        let body_block = self.new_block("while.body");
        let always = matches!(cond.kind, mir::ExprKind::BoolLiteral(true));
        if always {
            self.seal(mir::Terminator::Goto(body_block));
        } else {
            self.seal(mir::Terminator::Branch {
                cond,
                then_block: body_block,
                else_block: exit_block,
            });
        }
        self.enter(body_block);
        self.lower_statements(body);
        if !self.current_sealed {
            self.lower_continue(target);
        }
        let target = self.pop_loop_target(target);
        self.enter(exit_block);
        if always && !target.break_reachable {
            self.seal(mir::Terminator::Unreachable);
        }
    }

    pub(super) fn lower_do_while(
        &mut self,
        target: smir::LoopId,
        condition_setup: &'a [smir::Statement],
        cond: &smir::Expr,
        body: &'a [smir::Statement],
        span: Span,
    ) {
        let body_block = self.new_block("do.body");
        let header = self.new_block("do.cond");
        let exit_block = self.new_block("do.exit");
        self.seal(mir::Terminator::Goto(body_block));
        self.push_loop_target(target, header, exit_block);
        self.enter(body_block);
        self.lower_statements(body);
        if !self.current_sealed {
            self.lower_continue(target);
        }
        let reaches_condition = self
            .loop_targets
            .last()
            .is_some_and(|target| target.continue_reachable);
        self.enter(header);
        let mut condition_exits = false;
        if reaches_condition {
            self.lower_statements(condition_setup);
            if !self.current_sealed
                && let Some(cond) = self.lower_expr(cond, span)
            {
                if matches!(cond.kind, mir::ExprKind::BoolLiteral(true)) {
                    self.seal(mir::Terminator::Goto(body_block));
                } else {
                    self.seal(mir::Terminator::Branch {
                        cond,
                        then_block: body_block,
                        else_block: exit_block,
                    });
                    condition_exits = true;
                }
            }
        } else {
            self.seal(mir::Terminator::Unreachable);
        }
        let target = self.pop_loop_target(target);
        self.enter(exit_block);
        if !condition_exits && !target.break_reachable {
            self.seal(mir::Terminator::Unreachable);
        }
    }
}
