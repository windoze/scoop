//! Select call sites in stable order and retain cumulative caller growth.

use std::cell::OnceCell;

use super::{Choice, Context, Target, class_fields, flow, graph, index, mir, policy, simplify};
use crate::optimization::allocation::InlineReceivers;

impl Context<'_> {
    pub(super) fn choose(
        &self,
        function: &mir::Function,
        ancestry: &[Vec<Target>],
        remaining: &mut usize,
    ) -> Vec<Choice> {
        let analysis = flow::Analysis::new(self.module, function);
        let incoming = analysis.solve();
        let in_loop = graph::cyclic_nodes(&graph::block_edges(&function.body));
        let mut choices = Vec::new();
        let receivers = OnceCell::new();
        for (block_id, block) in function.body.blocks.iter() {
            if self.ordinary && ancestry[index(block_id)].len() > 6 {
                continue;
            }
            let Some(mut state) = incoming[index(block_id)].clone() else {
                continue;
            };
            for (statement_index, statement) in block.statements.iter().enumerate() {
                if self.ordinary && *remaining == 0 {
                    return choices;
                }
                // Release publication is adjacent to its outer initializer
                // and allocation. Keep that call when copying the sequence.
                let publishes_release =
                    block
                        .statements
                        .get(statement_index + 1)
                        .is_some_and(|next| {
                            matches!(next.kind, mir::StatementKind::PublishReleaseReady { .. })
                        });
                if let mir::StatementKind::Call(effect) = &statement.kind
                    && !publishes_release
                {
                    let call = crate::optimization::call(effect);
                    if let Some(callee) = self.target(call)
                        && (self.ordinary || self.required(callee))
                        && !ancestry[index(block_id)].contains(&callee)
                    {
                        let arguments = call
                            .args
                            .iter()
                            .map(|argument| analysis.expression(argument, &state))
                            .collect::<Vec<_>>();
                        let source = self.template(callee);
                        let blocks = simplify::blocks(self.module, source, &arguments);
                        let cost = policy::blocks_cost(
                            &self.module.structs,
                            blocks.iter().map(|(id, block)| (*id, block)),
                        )
                        .with_parameters(&self.module.structs, source);
                        let worthwhile = |cost: policy::Cost| {
                            cost.size.saturating_sub(6).max(1) <= *remaining
                                && policy::worthwhile(
                                    &self.module.structs,
                                    cost.execution,
                                    self.cost(callee).execution,
                                    in_loop[index(block_id)],
                                    &arguments,
                                    source,
                                )
                        };
                        let selected = if self.required(callee) || worthwhile(cost) {
                            Some(cost)
                        } else if self.allocation_elision
                            && source
                                .params
                                .first()
                                .is_some_and(|p| matches!(p.ty, mir::Type::Class(_)))
                        {
                            receivers
                                .get_or_init(|| InlineReceivers::new(self.module, function))
                                .as_ref()
                                .and_then(|receivers| {
                                    class_fields::cost(
                                        self.module,
                                        function,
                                        call,
                                        source,
                                        &blocks,
                                        receivers,
                                    )
                                })
                                .filter(|cost| worthwhile(*cost))
                        } else {
                            None
                        };
                        if let Some(cost) = selected {
                            if !self.required(callee) {
                                *remaining -= cost.size.saturating_sub(6).max(1);
                            }
                            choices.push(Choice {
                                block: block_id,
                                statement: statement_index,
                                callee,
                                blocks,
                            });
                        }
                    }
                }
                analysis.statement(statement, &mut state);
            }
        }
        choices
    }
}
