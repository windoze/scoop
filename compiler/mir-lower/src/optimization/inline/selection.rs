//! Spend bounded region growth on deeper loops before colder call sites.

use std::cell::OnceCell;
use std::cmp::Reverse;
use std::collections::HashSet;

use super::{
    Choice, Context, Target, class_fields, exception_paths, flow, index, mir, policy, simplify,
    summary,
};
use crate::optimization::allocation::InlineReceivers;
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
        growth: &mut policy::Growth,
    ) -> Vec<Choice> {
        let analysis = flow::Analysis::new(self.module, function);
        let incoming = analysis.solve();
        let heats = policy::heats(&function.body);
        let mut caller_size = policy::function_cost(&self.module.structs, function).size;
        let mut reads = HashSet::new();
        for (_, block) in function.body.blocks.iter() {
            mir::visit_block_exprs(block, &mut |expr| {
                if let mir::ExprKind::Local(local) | mir::ExprKind::AddressOf { local, .. } =
                    expr.kind
                {
                    reads.insert(local);
                }
            });
        }
        let mut order = function
            .body
            .blocks
            .iter()
            .map(|(id, _)| id)
            .collect::<Vec<_>>();
        if self.ordinary {
            order.sort_by_key(|&id| {
                let heat = heats[index(id)];
                Reverse(heat.numerator * 8 / heat.denominator)
            });
        }
        let mut choices = Vec::new();
        let receivers = OnceCell::new();
        let ranges = OnceCell::new();
        let mut exceptions =
            exception_paths::Effects::new(self.module, self.selected, self.dispatch, self.imported);
        for block_id in order {
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
                if let mir::StatementKind::Call(effect) = &statement.kind {
                    let site = policy::Site {
                        heat: heats[index(block_id)],
                        caller_size,
                        result_used: match effect {
                            mir::CallEffect::Value { destination, .. } => {
                                reads.contains(destination)
                            }
                            mir::CallEffect::Unit(_) => false,
                        },
                    };
                    let remaining = growth.remaining(site.heat);
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
                    let target = if publishes_release {
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
                        if self.ordinary && remaining == 0 && !self.required(callee) {
                            record(InlineOutcome::GrowthExhausted, None);
                            analysis.statement(statement, &mut state);
                            continue;
                        }
                        if self.ordinary || self.required(callee) {
                            let arguments = call
                                .args
                                .iter()
                                .map(|argument| analysis.expression(argument, &state))
                                .collect::<Vec<_>>();
                            let source = self.template(callee);
                            let facts = self.summary(callee, &exceptions);
                            let mut blocks = if facts.argument_sensitive(&arguments) {
                                let blocks = simplify::blocks(self.module, source, &arguments);
                                if facts.indirect_calls {
                                    exceptions.resolve_calls(source, &arguments, blocks)
                                } else {
                                    blocks
                                }
                            } else {
                                facts.blocks.clone()
                            };
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
                            let assess = |cost: policy::Cost, blocks: &[_], projection| {
                                policy::assess(
                                    facts,
                                    cost,
                                    summary::complexity(blocks),
                                    facts.savings(&self.module.structs, blocks),
                                    projection,
                                    &site,
                                )
                            };
                            let mut ordinary = assess(cost, &blocks, 0);
                            if !self.required(callee)
                                && !accepted(ordinary, remaining)
                                && site.heat.loop_depth != 0
                                && facts.array_access
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
                                    &specialized,
                                    0,
                                );
                                blocks = specialized;
                            }
                            let mut projection = InlineProjection::NotAttempted;
                            let (selected, outcome) = if self.required(callee) {
                                (Some(ordinary), InlineOutcome::Required)
                            } else if accepted(ordinary, remaining) {
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
                                    .map(|cost| {
                                        assess(
                                            cost,
                                            &blocks,
                                            ordinary.execution.saturating_sub(cost.execution),
                                        )
                                    });
                                projection = projected.map_or(
                                    InlineProjection::Unproven,
                                    InlineProjection::Estimated,
                                );
                                if let Some(cost) =
                                    projected.filter(|cost| accepted(*cost, remaining))
                                {
                                    (Some(cost), InlineOutcome::ClassFields)
                                } else {
                                    (None, rejected(projected.unwrap_or(ordinary), remaining))
                                }
                            } else {
                                (None, rejected(ordinary, remaining))
                            };
                            if self.observer.is_some() {
                                record(
                                    outcome,
                                    Some(Box::new(InlineEstimate {
                                        original_execution: facts.original.execution,
                                        original_size: facts.original.size,
                                        baseline_execution: facts.baseline.execution,
                                        baseline_size: facts.baseline.size,
                                        ordinary,
                                        projection,
                                        remaining_growth: remaining,
                                        region_remaining_growth: growth.region_remaining(site.heat),
                                        caller_remaining_growth: growth.caller_remaining(),
                                        in_loop: site.heat.loop_depth != 0,
                                        heat: site.heat,
                                        caller_size: site.caller_size,
                                        result_used: site.result_used,
                                        constant_arguments: arguments
                                            .iter()
                                            .filter(|a| a.constant.is_some())
                                            .count(),
                                        exact_arguments: arguments
                                            .iter()
                                            .filter(|a| a.actual != flow::ActualType::Unknown)
                                            .count(),
                                    })),
                                );
                            }
                            if let Some(cost) = selected {
                                if !self.required(callee) {
                                    growth.charge(cost.growth, site.heat);
                                    caller_size += cost.net_growth;
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

fn accepted(cost: InlineCost, remaining: usize) -> bool {
    cost.growth <= remaining && cost.weighted_benefit >= cost.penalty
}

fn rejected(cost: InlineCost, remaining: usize) -> InlineOutcome {
    if cost.growth > remaining {
        InlineOutcome::CallerGrowth
    } else {
        InlineOutcome::Profitability
    }
}
