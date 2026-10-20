use super::{constants::Constant, flow, graph::index, mir};

/// Fold only known scalar expressions; statements with effects are retained.
pub(super) fn blocks(
    module: &mir::Module,
    function: &mir::Function,
    arguments: &[flow::ValueFacts],
) -> Vec<(mir::BlockId, mir::BasicBlock)> {
    fold_blocks(module, function, arguments).0
}

fn fold_blocks(
    module: &mir::Module,
    function: &mir::Function,
    arguments: &[flow::ValueFacts],
) -> (Vec<(mir::BlockId, mir::BasicBlock)>, bool) {
    let analysis = flow::Analysis::new(module, function).with_arguments(arguments);
    let incoming = analysis.solve();
    let mut blocks = Vec::new();
    let mut changed = false;
    for (id, source) in function.body.blocks.iter() {
        let Some(mut state) = incoming[index(id)].clone() else {
            continue;
        };
        let mut block = source.clone();
        for statement in &mut block.statements {
            mir::visit_statement_exprs_mut(statement, &mut |expression| {
                if !matches!(
                    expression.kind,
                    mir::ExprKind::BoolLiteral(_) | mir::ExprKind::IntegerLiteral(_)
                ) && let Some(value) = analysis.expression(expression, &state).constant
                {
                    *expression = value.expression();
                    changed = true;
                }
            });
            analysis.statement(statement, &mut state);
        }
        mir::visit_terminator_exprs_mut(&mut block.terminator, &mut |expression| {
            if !matches!(
                expression.kind,
                mir::ExprKind::BoolLiteral(_) | mir::ExprKind::IntegerLiteral(_)
            ) && let Some(value) = analysis.expression(expression, &state).constant
            {
                *expression = value.expression();
                changed = true;
            }
        });
        if let mir::Terminator::Branch {
            cond,
            then_block,
            else_block,
        } = &block.terminator
            && let Some(Constant::Boolean(value)) = analysis.expression(cond, &state).constant
        {
            block.terminator = mir::Terminator::Goto(if value { *then_block } else { *else_block });
            changed = true;
        }
        blocks.push((id, block));
    }
    (blocks, changed)
}

pub(super) fn body(module: &mir::Module, function: &mir::Function) -> (mir::Body, bool) {
    let (blocks, mut changed) = fold_blocks(module, function, &[]);
    let mut body = function.body.clone();
    let mut reachable = vec![false; body.blocks.len()];
    for (id, block) in blocks {
        reachable[index(id)] = true;
        body.blocks[id] = block;
    }
    for (id, block) in body.blocks.iter_mut() {
        if !reachable[index(id)] {
            changed |= !block.statements.is_empty()
                || !matches!(block.terminator, mir::Terminator::Unreachable)
                || block.unwind.is_some();
            block.statements.clear();
            block.terminator = mir::Terminator::Unreachable;
            block.unwind = None;
        }
    }
    let polls = body.loop_header_polls.len();
    body.loop_header_polls
        .retain(|target| reachable[index(target.header())]);
    changed |= polls != body.loop_header_polls.len();
    (body, changed)
}
