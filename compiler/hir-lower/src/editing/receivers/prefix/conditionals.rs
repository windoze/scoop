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
            lowerer.with_smart_casts(facts, |lowerer| match branch.body {
                ast::RecoveredControlBody::Block(id) => {
                    self.block(lowerer, id, condition.is_some())
                }
                ast::RecoveredControlBody::Statement(start) => {
                    lowerer.push_scope();
                    let result =
                        self.expression(lowerer, conditional.block, start, condition.is_some());
                    lowerer.pop_scope();
                    result
                }
            })?;
        }
        Ok(())
    }
}
