use super::*;

impl<F: Fn() -> bool> PrefixQuery<'_, F> {
    pub(super) fn conditional(
        &mut self,
        lowerer: &mut Lowerer,
        conditional: &ast::RecoveredIf,
        available: bool,
    ) -> Result<(), EditingCancelled> {
        check_cancelled(self.is_cancelled)?;
        let condition = if available {
            let before = lowerer.diagnostics.len();
            let result = lowerer.lower_condition(&conditional.condition, "if", &mut Vec::new());
            result.filter(|_| lowerer.diagnostics.len() == before)
        } else {
            None
        };
        for branch in &conditional.branches {
            check_cancelled(self.is_cancelled)?;
            let facts = condition
                .as_ref()
                .map(|condition| lowerer.checked_condition_facts(condition, branch.on_true))
                .unwrap_or_default();
            lowerer.with_smart_casts(facts, |lowerer| {
                self.control_body(lowerer, conditional.block, branch.body, condition.is_some())
            })?;
        }
        Ok(())
    }
}
