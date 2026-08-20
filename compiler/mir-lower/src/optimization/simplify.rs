use super::{constants::Constant, flow, graph::index, mir};

/// Fold only known scalar expressions; statements with effects are retained.
pub(super) fn blocks(
    module: &mir::Module,
    function: &mir::Function,
    arguments: &[flow::ValueFacts],
) -> Vec<(mir::BlockId, mir::BasicBlock)> {
    let analysis = flow::Analysis::new(module, function).with_arguments(arguments);
    let incoming = analysis.solve();
    let mut blocks = Vec::new();
    for (id, source) in function.body.blocks.iter() {
        let Some(mut state) = incoming[index(id)].clone() else {
            continue;
        };
        let mut block = source.clone();
        for statement in &mut block.statements {
            mir::visit_statement_exprs_mut(statement, &mut |expression| {
                if let Some(value) = analysis.expression(expression, &state).constant {
                    *expression = value.expression();
                }
            });
            analysis.statement(statement, &mut state);
        }
        mir::visit_terminator_exprs_mut(&mut block.terminator, &mut |expression| {
            if let Some(value) = analysis.expression(expression, &state).constant {
                *expression = value.expression();
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
        }
        blocks.push((id, block));
    }
    blocks
}

pub(super) fn body(module: &mir::Module, function: &mir::Function) -> mir::Body {
    let blocks = blocks(module, function, &[]);
    let mut body = function.body.clone();
    for (_, block) in body.blocks.iter_mut() {
        block.statements.clear();
        block.terminator = mir::Terminator::Unreachable;
        block.unwind = None;
    }
    let mut reachable = vec![false; body.blocks.len()];
    for (id, block) in blocks {
        reachable[index(id)] = true;
        body.blocks[id] = block;
    }
    body.loop_header_polls
        .retain(|target| reachable[index(target.header())]);
    body
}
