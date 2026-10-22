use super::loops::loop_entry_facts;
use super::*;

impl<F: Fn() -> bool> PrefixQuery<'_, F> {
    pub(super) fn for_loop(
        &mut self,
        lowerer: &mut Lowerer,
        for_loop: &ast::RecoveredFor,
        available: bool,
    ) -> Result<(), EditingCancelled> {
        check_cancelled(self.is_cancelled)?;
        if !available {
            return self.control_body(lowerer, for_loop.block, for_loop.body, false);
        }
        let incoming = lowerer.smart_casts.clone();
        let prepared = lowerer.with_pattern_transaction(|state| {
            let head = state.lower_iteration_head(&for_loop.iterable, for_loop.span)?;
            state.smart_casts = loop_entry_facts(state);
            state.push_scope();
            if state
                .lower_irrefutable_binding_from_subject(&for_loop.pattern, head.element, false)
                .is_none()
            {
                state.pop_scope();
                return None;
            }
            Some(())
        });
        let result = if prepared.is_some() {
            let target = lowerer.fresh_loop();
            lowerer.push_loop(target, for_loop.label.as_ref());
            let result = self.control_body_contents(lowerer, for_loop.block, for_loop.body, true);
            lowerer.pop_loop(target);
            lowerer.pop_scope();
            result
        } else {
            self.control_body(lowerer, for_loop.block, for_loop.body, false)
        };
        lowerer.smart_casts = incoming;
        result
    }
}
