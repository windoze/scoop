use super::*;

impl Flow<'_> {
    pub(super) fn loop_outcomes(
        &self,
        kind: hir::LoopKind,
        target: hir::LoopId,
        condition_setup: &[hir::Statement],
        cond: &hir::Expr,
        body: &[hir::Statement],
    ) -> HirControlOutcomes {
        let mut result = HirControlOutcomes::empty();
        let reaches_condition = if kind == hir::LoopKind::DoWhile {
            result = self.statements(body);
            let falls_through = result.values.remove(&ControlOutcome::Fallthrough);
            let continues = result.values.contains(&ControlOutcome::Continue(target));
            let breaks = result.consume_loop_jumps(target);
            if breaks {
                result.values.insert(ControlOutcome::Fallthrough);
            }
            falls_through || continues
        } else {
            true
        };
        if !reaches_condition {
            return result;
        }
        let mut setup = self.statements(condition_setup).then(self.expression(cond));
        let reaches_body = setup.values.remove(&ControlOutcome::Fallthrough);
        let setup_breaks = setup.consume_loop_jumps(target);
        result = result.union(setup);
        if setup_breaks {
            result.values.insert(ControlOutcome::Fallthrough);
        }
        if reaches_body && !matches!(cond.kind, hir::ExprKind::BoolLiteral(true)) {
            result.values.insert(ControlOutcome::Fallthrough);
        }
        if reaches_body && kind == hir::LoopKind::While {
            let mut body_outcomes = self.statements(body);
            body_outcomes.values.remove(&ControlOutcome::Fallthrough);
            if body_outcomes.consume_loop_jumps(target) {
                result.values.insert(ControlOutcome::Fallthrough);
            }
            result = result.union(body_outcomes);
        }
        result
    }
}
