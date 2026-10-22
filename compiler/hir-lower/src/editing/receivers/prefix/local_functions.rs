use super::*;

impl<'a, F: Fn() -> bool> PrefixQuery<'a, F> {
    pub(super) fn function_body(
        &mut self,
        lowerer: &mut Lowerer,
        body: &'a ast::RecoveredFunctionBody,
        available: bool,
        known_result: Option<hir::TypeId>,
    ) -> Result<(), EditingCancelled> {
        let outer_body = std::mem::replace(&mut self.body, body);
        let outer_positions =
            std::mem::replace(&mut self.positions, self.facts.body_starts[&body.root]);
        let outer_expression_body = std::mem::replace(
            &mut self.expression_body_return,
            matches!(body.root, ast::RecoveredBodyRoot::Expression(_)),
        );
        let result = match body.root {
            ast::RecoveredBodyRoot::Block(id) => self.block(lowerer, id, available),
            ast::RecoveredBodyRoot::Expression(start) => {
                self.expression_with_expected(lowerer, None, start, available, known_result)
            }
        };
        self.body = outer_body;
        self.positions = outer_positions;
        self.expression_body_return = outer_expression_body;
        result
    }

    pub(super) fn local_function(
        &mut self,
        lowerer: &mut Lowerer,
        function: &'a ast::RecoveredLocalFunction,
        available: bool,
    ) -> Result<(), EditingCancelled> {
        check_cancelled(self.is_cancelled)?;
        if !available {
            return self.function_body(lowerer, &function.body, false, None);
        }
        self.capture_definitions
            .extend(lowerer.locals.iter().filter_map(|(_, local)| {
                lowerer
                    .editing_local_reference(local)
                    .map(|reference| (reference.binding, reference))
            }));
        let result = lowerer.with_recovered_local_function(function.header(), |state, id| {
            let signature = &state.signatures[&id];
            let known_result = (!signature.result_pending).then_some(signature.return_ty);
            self.function_body(state, &function.body, true, known_result)
        });
        match result {
            Some(result) => result,
            None => self.function_body(lowerer, &function.body, false, None),
        }
    }
}
