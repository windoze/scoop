//! Spend cumulative caller growth on deeper loops before colder call sites.

use std::cell::OnceCell;
use std::cmp::Reverse;

use super::{
    Choice, Context, Target, class_fields, exception_paths, flow, graph, index, mir, policy,
    simplify,
};
use crate::optimization::allocation::{ControlFlow, InlineReceivers};
use crate::optimization::poll_chunks::CallRanges;
use mir::optimization_report::{
    InlineCost, InlineEstimate, InlineOutcome, InlineProjection, OptimizationDecision,
    OptimizationSite,
};

impl Context<'_, '_> {
    pub(super) fn choose(
        &self,
        function_id: mir::FunctionId,
        function: &mir::Function,
        ancestry: &[Vec<Target>],
        remaining: &mut usize,
    ) -> Vec<Choice> {
        let analysis = flow::Analysis::new(self.module, function);
        let incoming = analysis.solve();
        let in_loop = graph::cyclic_nodes(&graph::block_edges(&function.body));
        let mut order = function
            .body
            .blocks
            .iter()
            .map(|(id, _)| id)
            .collect::<Vec<_>>();
        if self.ordinary && in_loop.iter().any(|&cyclic| cyclic) {
            let mut depths = ControlFlow::new(&function.body).loop_depths();
            for (depth, &cyclic) in depths.iter_mut().zip(&in_loop) {
                *depth = (*depth).max(usize::from(cyclic));
            }
            order.sort_by_key(|&id| Reverse(depths[index(id)]));
        }
        let mut choices = Vec::new();
        let receivers = OnceCell::new();
        let ranges = OnceCell::new();
        let mut exceptions =
            exception_paths::Effects::new(self.module, self.selected, self.dispatch, self.imported);
        'blocks: for block_id in order {
            let block = &function.body.blocks[block_id];
            if self.ordinary && ancestry[index(block_id)].len() > 6 {
                self.observe_skipped_block(
                    function_id,
                    block_id,
                    block,
                    &ancestry[index(block_id)],
                    InlineOutcome::Depth,
                );
                continue;
            }
            let Some(mut state) = incoming[index(block_id)].clone() else {
                continue;
            };
            for (statement_index, statement) in block.statements.iter().enumerate() {
                if self.ordinary && *remaining == 0 && self.observer.is_none() {
                    break 'blocks;
                }
                if let mir::StatementKind::Call(effect) = &statement.kind {
                    let resolved = self
                        .ordinary
                        .then(|| exceptions.resolve(function, effect, &analysis, &state))
                        .flatten();
                    let call = resolved
                        .as_ref()
                        .unwrap_or_else(|| crate::optimization::call(effect));
                    let record = |outcome, estimate| {
                        if let Some(observer) = self.observer {
                            observer.record(
                                OptimizationSite {
                                    function: function_id,
                                    block: block_id,
                                    statement: statement_index,
                                    span: statement.span,
                                    inline_path: ancestry[index(block_id)]
                                        .iter()
                                        .map(|t| t.callee())
                                        .collect(),
                                },
                                OptimizationDecision::Inline {
                                    target: crate::optimization::observation::call_target(
                                        &call.target,
                                    ),
                                    outcome,
                                    estimate,
                                },
                            );
                        }
                    };
                    // Keep release publication adjacent to its outer initializer.
                    let publishes_release =
                        block
                            .statements
                            .get(statement_index + 1)
                            .is_some_and(|next| {
                                matches!(next.kind, mir::StatementKind::PublishReleaseReady { .. })
                            });
                    let target = if self.ordinary && *remaining == 0 {
                        Err(InlineOutcome::GrowthExhausted)
                    } else if publishes_release {
                        Err(InlineOutcome::ReleasePublication)
                    } else {
                        self.target(call).and_then(|callee| {
                            if ancestry[index(block_id)].contains(&callee) {
                                Err(InlineOutcome::RecursiveExpansion)
                            } else {
                                Ok(callee)
                            }
                        })
                    };
                    if let Ok(callee) = target {
                        if self.ordinary || self.required(callee) {
                            let arguments = call
                                .args
                                .iter()
                                .map(|argument| analysis.expression(argument, &state))
                                .collect::<Vec<_>>();
                            let source = self.template(callee);
                            let mut blocks = simplify::blocks(self.module, source, &arguments);
                            if self.exceptional.contains(&callee)
                                && !policy::supported_blocks(blocks.iter().map(|(_, b)| b))
                            {
                                let specialized = self
                                    .ordinary
                                    .then(|| exceptions.specialize(source, &arguments, blocks))
                                    .flatten();
                                let Some(specialized) = specialized else {
                                    record(InlineOutcome::UnsupportedBody, None);
                                    analysis.statement(statement, &mut state);
                                    continue;
                                };
                                blocks = specialized;
                            }
                            let cost = policy::blocks_cost(
                                &self.module.structs,
                                blocks.iter().map(|(id, block)| (*id, block)),
                            )
                            .with_parameters(&self.module.structs, source);
                            let original = OnceCell::new();
                            let assess = |cost: policy::Cost| {
                                let growth = cost.size.saturating_sub(6).max(1);
                                InlineCost {
                                    execution: cost.execution,
                                    size: cost.size,
                                    growth,
                                    execution_limit: (!self.required(callee)
                                        && growth <= *remaining)
                                        .then(|| {
                                            policy::execution_limit(
                                                &self.module.structs,
                                                cost.execution,
                                                original
                                                    .get_or_init(|| self.cost(callee))
                                                    .execution,
                                                in_loop[index(block_id)],
                                                &arguments,
                                                source,
                                            )
                                        }),
                                }
                            };
                            let mut ordinary = assess(cost);
                            if !self.required(callee)
                                && !accepted(ordinary)
                                && in_loop[index(block_id)]
                                && has_array_access(&blocks)
                                && let Some(specialized) = ranges
                                    .get_or_init(|| CallRanges::new(self.module, function))
                                    .blocks(
                                        self.module,
                                        (block_id, statement_index),
                                        call,
                                        source,
                                        &blocks,
                                    )
                            {
                                ordinary = assess(
                                    policy::blocks_cost(
                                        &self.module.structs,
                                        specialized.iter().map(|(id, block)| (*id, block)),
                                    )
                                    .with_parameters(&self.module.structs, source),
                                );
                                blocks = specialized;
                            }
                            let mut projection = InlineProjection::NotAttempted;
                            let (selected, outcome) = if self.required(callee) {
                                (Some(ordinary), InlineOutcome::Required)
                            } else if accepted(ordinary) {
                                (Some(ordinary), InlineOutcome::Inlined)
                            } else if self.allocation_elision
                                && source
                                    .params
                                    .first()
                                    .is_some_and(|p| matches!(p.ty, mir::Type::Class(_)))
                            {
                                let projected = receivers
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
                                    .map(assess);
                                projection = projected.map_or(
                                    InlineProjection::Unproven,
                                    InlineProjection::Estimated,
                                );
                                if let Some(cost) = projected.filter(|cost| accepted(*cost)) {
                                    (Some(cost), InlineOutcome::ClassFields)
                                } else {
                                    (None, rejected(projected.unwrap_or(ordinary)))
                                }
                            } else {
                                (None, rejected(ordinary))
                            };
                            if self.observer.is_some() {
                                let original = original.get_or_init(|| self.cost(callee));
                                record(
                                    outcome,
                                    Some(InlineEstimate {
                                        original_execution: original.execution,
                                        original_size: original.size,
                                        ordinary,
                                        projection,
                                        remaining_growth: *remaining,
                                        in_loop: in_loop[index(block_id)],
                                    }),
                                );
                            }
                            if let Some(cost) = selected {
                                if !self.required(callee) {
                                    *remaining -= cost.growth;
                                }
                                choices.push(Choice {
                                    block: block_id,
                                    statement: statement_index,
                                    callee,
                                    call: call.clone(),
                                    blocks,
                                });
                            }
                        }
                    } else if let Err(reason) = target {
                        record(reason, None);
                    }
                }
                analysis.statement(statement, &mut state);
            }
        }
        // Applying independent choices still follows their original positions.
        choices.sort_unstable_by_key(|choice| (index(choice.block), choice.statement));
        choices
    }

    fn observe_skipped_block(
        &self,
        function: mir::FunctionId,
        block_id: mir::BlockId,
        block: &mir::BasicBlock,
        ancestry: &[Target],
        outcome: InlineOutcome,
    ) {
        let Some(observer) = self.observer else {
            return;
        };
        for (statement, node) in block.statements.iter().enumerate() {
            if let mir::StatementKind::Call(effect) = &node.kind {
                observer.record(
                    OptimizationSite {
                        function,
                        block: block_id,
                        statement,
                        span: node.span,
                        inline_path: ancestry.iter().map(|t| t.callee()).collect(),
                    },
                    OptimizationDecision::Inline {
                        target: crate::optimization::observation::call_target(
                            &crate::optimization::call(effect).target,
                        ),
                        outcome,
                        estimate: None,
                    },
                );
            }
        }
    }
}

fn accepted(cost: InlineCost) -> bool {
    cost.execution_limit
        .is_some_and(|limit| cost.execution <= limit)
}

fn has_array_access(blocks: &[(mir::BlockId, mir::BasicBlock)]) -> bool {
    blocks.iter().any(|(_, block)| {
        let mut found = block
            .statements
            .iter()
            .any(|statement| matches!(statement.kind, mir::StatementKind::ArraySet { .. }));
        mir::visit_block_exprs(block, &mut |expr| {
            found |= matches!(expr.kind, mir::ExprKind::ArrayGet { .. });
        });
        found
    })
}

fn rejected(cost: InlineCost) -> InlineOutcome {
    if cost.execution_limit.is_none() {
        InlineOutcome::CallerGrowth
    } else {
        InlineOutcome::ExecutionCost
    }
}
