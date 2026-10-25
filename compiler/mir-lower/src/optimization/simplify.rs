use super::{constants::Constant, flow, graph::index, mir};
use std::collections::HashSet;

/// Fold known values and total typed reads; statements with effects are retained.
pub(super) fn blocks(
    module: &mir::Module,
    function: &mir::Function,
    arguments: &[flow::ValueFacts],
) -> Vec<(mir::BlockId, mir::BasicBlock)> {
    fold_blocks(module, function, arguments, false).0
}

fn fold_blocks(
    module: &mir::Module,
    function: &mir::Function,
    arguments: &[flow::ValueFacts],
    current_copies: bool,
) -> (Vec<(mir::BlockId, mir::BasicBlock)>, bool) {
    let analysis = if current_copies {
        flow::Analysis::for_copies(module, function)
    } else {
        flow::Analysis::new(module, function)
    }
    .with_arguments(arguments);
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
                    mir::ExprKind::BoolLiteral(_)
                        | mir::ExprKind::IntegerLiteral(_)
                        | mir::ExprKind::StringConst(_)
                ) && let Some(value) = analysis.expression(expression, &state).constant
                {
                    *expression = value.expression();
                    changed = true;
                } else if let mir::ExprKind::Local(local) = expression.kind
                    && let Some(alias) = analysis.expression(expression, &state).alias
                    && local != alias
                    && expression.ty == function.body.locals[alias].ty
                {
                    expression.kind = mir::ExprKind::Local(alias);
                    changed = true;
                }
            });
            if let mir::StatementKind::Call(mir::CallEffect::Value { destination, call }) =
                &statement.kind
                && let Some(value) = analysis.call_constant(call, &state)
            {
                statement.kind = mir::StatementKind::ValDecl {
                    local: *destination,
                    init: value.expression(),
                };
                changed = true;
            }
            analysis.statement(statement, &mut state);
        }
        mir::visit_terminator_exprs_mut(&mut block.terminator, &mut |expression| {
            if !matches!(
                expression.kind,
                mir::ExprKind::BoolLiteral(_)
                    | mir::ExprKind::IntegerLiteral(_)
                    | mir::ExprKind::StringConst(_)
            ) && let Some(value) = analysis.expression(expression, &state).constant
            {
                *expression = value.expression();
                changed = true;
            } else if let mir::ExprKind::Local(local) = expression.kind
                && let Some(alias) = analysis.expression(expression, &state).alias
                && local != alias
                && expression.ty == function.body.locals[alias].ty
            {
                expression.kind = mir::ExprKind::Local(alias);
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
    let mut read = HashSet::new();
    for (_, block) in &blocks {
        mir::visit_block_exprs(block, &mut |expression| {
            if let mir::ExprKind::Local(local) | mir::ExprKind::AddressOf { local, .. } =
                expression.kind
            {
                read.insert(local);
            }
        });
    }
    for (_, block) in &mut blocks {
        block.statements.retain(|statement| {
            let unused_copy = matches!(statement.kind,
                mir::StatementKind::ValDecl { local, init: mir::Expr { kind: mir::ExprKind::Local(_), .. } }
                    if !read.contains(&local)) || (current_copies && matches!(statement.kind,
                mir::StatementKind::Assign { local, value: mir::Expr { kind: mir::ExprKind::Local(_), .. } }
                    if !read.contains(&local)));
            changed |= unused_copy;
            !unused_copy
        });
    }
    (blocks, changed)
}

pub(super) fn body(module: &mir::Module, function: &mir::Function) -> (mir::Body, bool) {
    rewrite_body(module, function, false)
}

pub(super) fn copies(module: &mut mir::Module) {
    let changed = module
        .functions
        .iter()
        .filter_map(|(id, function)| {
            let (body, changed) = rewrite_body(module, function, true);
            changed.then_some((id, body))
        })
        .collect::<Vec<_>>();
    for (id, body) in changed {
        module.functions[id].body = body;
    }
}

fn rewrite_body(
    module: &mir::Module,
    function: &mir::Function,
    current_copies: bool,
) -> (mir::Body, bool) {
    let (blocks, mut changed) = fold_blocks(module, function, &[], current_copies);
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
