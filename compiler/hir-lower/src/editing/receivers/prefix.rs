use super::*;

mod conditionals;
mod loops;

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
        let result = self.with_function_scope(function, &name, |lowerer| {
            let mut query = PrefixQuery {
                body,
                facts: &mut facts,
                is_cancelled,
            };
            match body.root {
                ast::RecoveredBodyRoot::Block(id) => query.block(lowerer, id, true),
                ast::RecoveredBodyRoot::Expression(start) => {
                    query.expression(lowerer, None, start, true)
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
}

impl<F: Fn() -> bool> PrefixQuery<'_, F> {
    fn block(
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
        lowerer.push_scope();
        let result = (|| {
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
        })();
        lowerer.pop_scope();
        result
    }

    fn expression(
        &mut self,
        lowerer: &mut Lowerer,
        block: Option<ast::BodyBlockId>,
        start: u32,
        mut available: bool,
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
        Ok(())
    }

    fn control_body(
        &mut self,
        lowerer: &mut Lowerer,
        parent: Option<ast::BodyBlockId>,
        body: ast::RecoveredControlBody,
        available: bool,
    ) -> Result<(), EditingCancelled> {
        match body {
            ast::RecoveredControlBody::Block(id) => self.block(lowerer, id, available),
            ast::RecoveredControlBody::Statement(start) => {
                lowerer.push_scope();
                let result = self.expression(lowerer, parent, start, available);
                lowerer.pop_scope();
                result
            }
        }
    }

    fn operand(
        &mut self,
        lowerer: &mut Lowerer,
        operand: &ast::RecoveredOperand,
        available: bool,
    ) -> Result<(), EditingCancelled> {
        if !available {
            return self.expression(lowerer, operand.block, operand.expression_start, false);
        }
        let available = match &operand.context {
            ast::RecoveredOperandContext::Initializer { annotation } => annotation
                .as_ref()
                .is_none_or(|ty| lowerer.resolve_type_ref(ty).is_some()),
            ast::RecoveredOperandContext::Return { label } => {
                !matches!(self.body.root, ast::RecoveredBodyRoot::Expression(_))
                    && lowerer
                        .check_return_context(
                            label.as_ref(),
                            ast::Span::new(operand.statement_start, operand.expression_start),
                        )
                        .is_some()
            }
            ast::RecoveredOperandContext::Throw | ast::RecoveredOperandContext::IfCondition => true,
            ast::RecoveredOperandContext::WhileCondition { label } => {
                return self.while_condition(lowerer, operand, label.as_ref());
            }
        };
        self.expression(lowerer, operand.block, operand.expression_start, available)
    }
}
