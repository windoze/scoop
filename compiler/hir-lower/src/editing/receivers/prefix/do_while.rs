use super::*;

impl<F: Fn() -> bool> PrefixQuery<'_, F> {
    pub(super) fn do_while(
        &mut self,
        lowerer: &mut Lowerer,
        do_while: &ast::RecoveredDoWhile,
        available: bool,
    ) -> Result<(), EditingCancelled> {
        check_cancelled(self.is_cancelled)?;
        let incoming = lowerer.facts_without_mutable_bindings(lowerer.smart_casts.clone());
        lowerer.with_smart_casts(incoming.clone(), |lowerer| {
            let target = lowerer.fresh_loop();
            lowerer.push_loop(target, do_while.label.as_ref());
            lowerer.push_scope();
            let result = match &do_while.part {
                ast::RecoveredDoWhilePart::Body(body) => {
                    self.control_body_contents(lowerer, do_while.block, *body, available)
                }
                ast::RecoveredDoWhilePart::Condition {
                    body,
                    expression_start,
                } => self.do_while_condition(
                    lowerer,
                    do_while.block,
                    body,
                    *expression_start,
                    available,
                    incoming,
                    target,
                ),
            };
            lowerer.pop_scope();
            lowerer.pop_loop(target);
            result
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn do_while_condition(
        &mut self,
        lowerer: &mut Lowerer,
        parent: Option<ast::BodyBlockId>,
        body: &ast::Block,
        start: u32,
        available: bool,
        incoming: crate::expr::SmartCastFacts,
        target: hir::LoopId,
    ) -> Result<(), EditingCancelled> {
        if let Some(statements) = self.complete_statements(lowerer, &body.statements, available)? {
            lowerer.with_do_while_condition(incoming, &statements, target, |lowerer| {
                self.expression(lowerer, parent, start, true)
            })
        } else {
            self.expression(lowerer, parent, start, false)
        }
    }
}
