use super::*;

impl<F: Fn() -> bool> PrefixQuery<'_, F> {
    pub(super) fn call(
        &mut self,
        lowerer: &mut Lowerer,
        index: usize,
        call: &ast::RecoveredCall,
        available: bool,
    ) -> Result<(), EditingCancelled> {
        check_cancelled(self.is_cancelled)?;
        if !available {
            self.facts.calls[index].candidates = V::Unavailable(U::BodyFailure);
            return Ok(());
        }
        let candidates = lowerer.editing_call_candidates(call, self.is_cancelled)?;
        let mut candidates = match candidates {
            V::Known(candidates) => candidates,
            V::Unavailable(reason) => {
                self.facts.calls[index].candidates = V::Unavailable(reason);
                return Ok(());
            }
        };
        let argument = match candidates.as_mut_slice() {
            [candidate] => candidate.argument.take(),
            _ => None,
        };
        self.facts.calls[index].candidates = V::Known(
            candidates
                .into_iter()
                .map(|candidate| candidate.signature)
                .collect(),
        );
        if let Some(argument) = argument {
            check_cancelled(self.is_cancelled)?;
            let literal = self
                .body
                .callable_literals
                .iter()
                .find(|literal| literal.body == argument.body)
                .expect("a callable argument refers to its grammar header and body");
            // Expected types and checked argument flow belong to this state.
            // No candidate-local type is interpreted through another arena.
            *lowerer = argument.state;
            self.callable_literal(lowerer, literal, true, Some(argument.expected))?;
        }
        Ok(())
    }
}
