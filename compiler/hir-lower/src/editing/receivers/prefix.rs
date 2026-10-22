use super::*;

impl Lowerer {
    pub(super) fn check_recovered_facts(
        &mut self,
        function: hir::FunctionId,
        body: &ast::RecoveredFunctionBody,
        is_cancelled: &impl Fn() -> bool,
    ) -> Result<RecoveredFacts, EditingCancelled> {
        let mut facts = unavailable_facts(body, U::IncompleteSyntax);
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
        let calls = body
            .calls
            .iter()
            .enumerate()
            .filter(|(_, call)| call.block == block && call.span.start == start)
            .collect::<Vec<_>>();
        if eligible.is_empty() && calls.is_empty() {
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
                        lowerer.check_receiver(
                            receiver,
                            &mut facts.receivers[index],
                            is_cancelled,
                        )?;
                    } else {
                        facts.receivers[index].value_type = V::Unavailable(U::BodyFailure);
                        facts.receivers[index].members = V::Unavailable(U::BodyFailure);
                    }
                }
                for (index, call) in calls {
                    check_cancelled(is_cancelled)?;
                    facts.calls[index].candidates = if lowerer.diagnostics.len() == before {
                        lowerer.editing_call_candidates(call, is_cancelled)?
                    } else {
                        V::Unavailable(U::BodyFailure)
                    };
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
