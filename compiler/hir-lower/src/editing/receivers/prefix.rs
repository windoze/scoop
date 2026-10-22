use super::*;

impl Lowerer {
    pub(super) fn check_recovered_receivers(
        &mut self,
        function: hir::FunctionId,
        body: &ast::RecoveredFunctionBody,
        is_cancelled: &impl Fn() -> bool,
    ) -> Result<Vec<hir::EditingReceiver>, EditingCancelled> {
        let mut facts = unavailable_receivers(body, U::IncompleteSyntax);
        let Some((prefix, start, block)) = receiver_prefix(body) else {
            return Ok(facts);
        };
        let eligible = body
            .selectors
            .iter()
            .enumerate()
            .filter_map(|(index, selector)| {
                let ast::RecoveredReceiver::Complete(receiver) = &selector.receiver else {
                    return None;
                };
                (selector.block == block && receiver.span().start == start)
                    .then_some((index, receiver))
            })
            .collect::<Vec<_>>();
        if eligible.is_empty() {
            return Ok(facts);
        }
        self.current_file = self
            .function_definition_file(function)
            .expect("source function has a definition file");
        let name = self.functions[function].name.clone();
        let previous_params = self.functions[function].params.clone();
        let before = self.diagnostics.len();
        let result = self.with_function_scope(function, &name, |lowerer| {
            if block.is_some() {
                lowerer.push_scope();
            }
            let result = (|| {
                let mut statements = Vec::new();
                for statement in prefix {
                    check_cancelled(is_cancelled)?;
                    let ast::RecoveredStatement::Complete(statement) = statement else {
                        unreachable!("prefix ends before the first unknown statement");
                    };
                    lowerer.lower_statement(statement, &mut statements);
                    if lowerer.diagnostics.len() != before {
                        break;
                    }
                }
                for (index, receiver) in eligible {
                    check_cancelled(is_cancelled)?;
                    if lowerer.diagnostics.len() == before {
                        lowerer.check_receiver(receiver, &mut facts[index]);
                    } else {
                        facts[index].value_type = V::Unavailable(U::BodyFailure);
                    }
                }
                Ok(facts)
            })();
            if block.is_some() {
                lowerer.pop_scope();
            }
            result
        });
        self.functions[function].params = previous_params;
        result.value
    }
}

/// A receiver is checked only after the actual function-body prefix.
fn receiver_prefix(
    body: &ast::RecoveredFunctionBody,
) -> Option<(&[ast::RecoveredStatement], u32, Option<ast::BodyBlockId>)> {
    match body.root {
        ast::RecoveredBodyRoot::Expression(start) => Some((&[], start, None)),
        ast::RecoveredBodyRoot::Block(id) => {
            let block = body.blocks.iter().find(|block| block.id == id)?;
            let (index, start) = block.statements.iter().enumerate().find_map(
                |(index, statement)| match statement {
                    ast::RecoveredStatement::Unavailable(span) => Some((index, span.start)),
                    ast::RecoveredStatement::Complete(_) => None,
                },
            )?;
            Some((&block.statements[..index], start, Some(id)))
        }
    }
}
