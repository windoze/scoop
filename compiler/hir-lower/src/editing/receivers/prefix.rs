use super::*;

mod callables;
mod conditionals;
mod do_while;
mod exceptions;
mod iteration;
mod loops;
mod operands;
mod when;

impl Lowerer {
    pub(super) fn check_recovered_facts(
        &mut self,
        function: hir::FunctionId,
        body: &ast::RecoveredFunctionBody,
        is_cancelled: &impl Fn() -> bool,
    ) -> Result<RecoveredFacts, EditingCancelled> {
        let mut facts = unavailable_facts(body, U::IncompleteSyntax);
        if body.selectors.is_empty() && body.calls.is_empty() {
            return Ok(facts);
        }
        self.current_file = self
            .function_definition_file(function)
            .expect("source function has a definition file");
        let name = self.functions[function].name.clone();
        let previous_params = self.functions[function].params.clone();
        let known_result = (!self.signatures[&function].result_pending)
            .then_some(self.signatures[&function].return_ty);
        let result = self.with_function_scope(function, &name, |lowerer| {
            let mut query = PrefixQuery {
                body,
                facts: &mut facts,
                is_cancelled,
                expression_body_return: matches!(body.root, ast::RecoveredBodyRoot::Expression(_)),
                capture_definitions: Default::default(),
            };
            match body.root {
                ast::RecoveredBodyRoot::Block(id) => query.block(lowerer, id, true),
                ast::RecoveredBodyRoot::Expression(start) => {
                    query.expression_with_expected(lowerer, None, start, true, known_result)
                }
            }
        });
        self.functions[function].params = previous_params;
        result.value?;
        Ok(facts)
    }
}

struct PrefixQuery<'a, F> {
    body: &'a ast::RecoveredFunctionBody,
    facts: &'a mut RecoveredFacts,
    is_cancelled: &'a F,
    expression_body_return: bool,
    capture_definitions: std::collections::HashMap<hir::BindingId, hir::EditingLocalReference>,
}

impl<F: Fn() -> bool> PrefixQuery<'_, F> {
    fn complete_statements(
        &mut self,
        lowerer: &mut Lowerer,
        source: &[ast::Statement],
        available: bool,
    ) -> Result<Option<Vec<hir::Statement>>, EditingCancelled> {
        if !available {
            return Ok(None);
        }
        let mut statements = Vec::new();
        for statement in source {
            check_cancelled(self.is_cancelled)?;
            let before = lowerer.diagnostics.len();
            lowerer.lower_statement(statement, &mut statements);
            if lowerer.diagnostics.len() != before {
                return Ok(None);
            }
        }
        Ok(Some(statements))
    }

    fn block(
        &mut self,
        lowerer: &mut Lowerer,
        id: ast::BodyBlockId,
        available: bool,
    ) -> Result<(), EditingCancelled> {
        lowerer.push_scope();
        let result = self.block_contents(lowerer, id, available);
        lowerer.pop_scope();
        result
    }

    fn block_contents(
        &mut self,
        lowerer: &mut Lowerer,
        id: ast::BodyBlockId,
        mut available: bool,
    ) -> Result<(), EditingCancelled> {
        check_cancelled(self.is_cancelled)?;
        let block = self
            .body
            .blocks
            .iter()
            .find(|block| block.id == id)
            .expect("a recovered block entry retains its grammar block");
        let mut statements = Vec::new();
        for statement in &block.statements {
            check_cancelled(self.is_cancelled)?;
            match statement {
                ast::RecoveredStatement::Complete(statement) => {
                    if available {
                        let before = lowerer.diagnostics.len();
                        lowerer.lower_statement(statement, &mut statements);
                        available = lowerer.diagnostics.len() == before;
                    }
                }
                ast::RecoveredStatement::Unavailable(span) => {
                    return self.expression(lowerer, Some(id), span.start, available);
                }
            }
        }
        Ok(())
    }

    fn expression(
        &mut self,
        lowerer: &mut Lowerer,
        block: Option<ast::BodyBlockId>,
        start: u32,
        available: bool,
    ) -> Result<(), EditingCancelled> {
        self.expression_with_expected(lowerer, block, start, available, None)
    }

    fn expression_with_expected(
        &mut self,
        lowerer: &mut Lowerer,
        block: Option<ast::BodyBlockId>,
        start: u32,
        mut available: bool,
        expected: Option<hir::TypeId>,
    ) -> Result<(), EditingCancelled> {
        check_cancelled(self.is_cancelled)?;
        for (index, selector) in self.body.selectors.iter().enumerate() {
            let ast::RecoveredReceiver::Complete(receiver) = &selector.receiver else {
                continue;
            };
            if selector.block != block || receiver.span().start != start {
                continue;
            }
            if available {
                let before = lowerer.diagnostics.len();
                lowerer.check_receiver(
                    receiver,
                    &mut self.facts.receivers[index],
                    &self.capture_definitions,
                    self.is_cancelled,
                )?;
                available = lowerer.diagnostics.len() == before;
            } else {
                self.facts.receivers[index].value_type = V::Unavailable(U::BodyFailure);
                self.facts.receivers[index].members = V::Unavailable(U::BodyFailure);
            }
        }
        for (index, call) in self.body.calls.iter().enumerate() {
            if call.block != block || call.span.start != start {
                continue;
            }
            check_cancelled(self.is_cancelled)?;
            self.facts.calls[index].candidates = if available {
                lowerer.editing_call_candidates(call, self.is_cancelled)?
            } else {
                V::Unavailable(U::BodyFailure)
            };
        }
        if let Some(operand) = self
            .body
            .operands
            .iter()
            .find(|operand| operand.block == block && operand.statement_start == start)
        {
            return self.operand(lowerer, operand, available);
        }
        if let Some(literal) = self
            .body
            .callable_literals
            .iter()
            .find(|literal| literal.block == block && literal.span.start == start)
        {
            return self.callable_literal(lowerer, literal, available, expected);
        }
        if let Some(child) = self.body.blocks.iter().find(|child| {
            child.parent == block
                && child.span.start == start
                && child.scope == ast::RecoveredBlockScope::Block
        }) {
            return self.block(lowerer, child.id, available);
        }
        if let Some(conditional) = self
            .body
            .conditionals
            .iter()
            .find(|conditional| conditional.block == block && conditional.start == start)
        {
            return self.conditional(lowerer, conditional, available);
        }
        if let Some(while_loop) = self
            .body
            .while_loops
            .iter()
            .find(|while_loop| while_loop.block == block && while_loop.start == start)
        {
            return self.while_loop(lowerer, while_loop, available);
        }
        if let Some(for_loop) = self
            .body
            .for_loops
            .iter()
            .find(|for_loop| for_loop.block == block && for_loop.span.start == start)
        {
            return self.for_loop(lowerer, for_loop, available);
        }
        if let Some(do_while) = self
            .body
            .do_while_loops
            .iter()
            .find(|do_while| do_while.block == block && do_while.start == start)
        {
            return self.do_while(lowerer, do_while, available);
        }
        if let Some(when) = self
            .body
            .whens
            .iter()
            .find(|when| when.block == block && when.start == start)
        {
            return self.when(lowerer, when, available);
        }
        if let Some(try_) = self
            .body
            .tries
            .iter()
            .find(|try_| try_.block == block && try_.start == start)
        {
            return self.try_(lowerer, try_, available);
        }
        Ok(())
    }

    fn control_body(
        &mut self,
        lowerer: &mut Lowerer,
        parent: Option<ast::BodyBlockId>,
        body: ast::RecoveredControlBody,
        available: bool,
    ) -> Result<(), EditingCancelled> {
        lowerer.push_scope();
        let result = self.control_body_contents(lowerer, parent, body, available);
        lowerer.pop_scope();
        result
    }

    fn control_body_contents(
        &mut self,
        lowerer: &mut Lowerer,
        parent: Option<ast::BodyBlockId>,
        body: ast::RecoveredControlBody,
        available: bool,
    ) -> Result<(), EditingCancelled> {
        match body {
            ast::RecoveredControlBody::Block(id) => self.block_contents(lowerer, id, available),
            ast::RecoveredControlBody::Statement(start) => {
                self.expression(lowerer, parent, start, available)
            }
        }
    }
}
