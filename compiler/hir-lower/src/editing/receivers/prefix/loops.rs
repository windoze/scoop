use super::*;

impl<F: Fn() -> bool> PrefixQuery<'_, F> {
    pub(super) fn while_loop(
        &mut self,
        lowerer: &mut Lowerer,
        while_loop: &ast::RecoveredWhile,
        available: bool,
    ) -> Result<(), EditingCancelled> {
        check_cancelled(self.is_cancelled)?;
        lowerer.with_smart_casts(loop_entry_facts(lowerer), |lowerer| {
            let target = lowerer.fresh_loop();
            lowerer.push_loop(target, while_loop.label.as_ref());
            let condition = if available {
                let before = lowerer.diagnostics.len();
                let result =
                    lowerer.lower_condition(&while_loop.condition, "while", &mut Vec::new());
                result.filter(|_| lowerer.diagnostics.len() == before)
            } else {
                None
            };
            if let Some(condition) = &condition {
                lowerer.smart_casts = lowerer.checked_condition_facts(condition, true);
            }
            let result = self.control_body(
                lowerer,
                while_loop.block,
                while_loop.body,
                condition.is_some(),
            );
            lowerer.pop_loop(target);
            result
        })
    }

    pub(super) fn while_condition(
        &mut self,
        lowerer: &mut Lowerer,
        operand: &ast::RecoveredOperand,
        label: Option<&ast::Ident>,
    ) -> Result<(), EditingCancelled> {
        check_cancelled(self.is_cancelled)?;
        lowerer.with_smart_casts(loop_entry_facts(lowerer), |lowerer| {
            let target = lowerer.fresh_loop();
            lowerer.push_loop(target, label);
            let result = self.expression(lowerer, operand.block, operand.expression_start, true);
            lowerer.pop_loop(target);
            result
        })
    }
}

pub(super) fn loop_entry_facts(lowerer: &Lowerer) -> crate::expr::SmartCastFacts {
    // The unknown backedge can write mutable values but cannot replace val bindings.
    let mut facts = lowerer.smart_casts.clone();
    for (_, local) in lowerer.locals.iter() {
        if local.mutable {
            facts.forget(local.binding);
        }
    }
    for context in &lowerer.capture_contexts {
        for capture in context.available.values() {
            if capture.mutable {
                facts.forget(capture.binding);
            }
        }
    }
    facts
}
