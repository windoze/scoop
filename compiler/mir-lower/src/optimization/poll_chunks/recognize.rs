use std::collections::{BTreeMap, BTreeSet};

use super::{CHUNK, Loop, linear, mir, operations};
use crate::optimization::graph::index;

pub(super) fn counted(
    module: &mir::Module,
    function: &mir::Function,
    header: mir::BlockId,
) -> Option<Loop> {
    let body = &function.body;
    if header == body.entry {
        return None;
    }
    let incoming = linear::predecessors(body);
    let header_chain = linear::chain(body, header, header, &incoming)?;
    let mir::Terminator::Branch {
        cond,
        then_block: inner,
        else_block: exit,
    } = &body.blocks[*header_chain.last()?].terminator
    else {
        return None;
    };
    if header_chain.contains(inner) || header_chain.contains(exit) || inner == exit {
        return None;
    }
    let inner = *inner;
    let body_chain = linear::chain(body, inner, header, &incoming)?;
    let last = *body_chain.last()?;
    if body_chain.contains(exit)
        || body_chain.iter().any(|id| {
            body.loop_header_polls
                .iter()
                .any(|poll| poll.header() == *id)
        })
        || incoming[index(inner)] != [*header_chain.last()?]
        || !matches!(body.blocks[last].terminator, mir::Terminator::Goto(target) if target == header)
        || header_chain
            .iter()
            .any(|id| !operations::supported(module, &body.blocks[*id], false))
        || body_chain
            .iter()
            .any(|id| !operations::supported(module, &body.blocks[*id], true))
    {
        return None;
    }
    if incoming[index(header)].len() != 2 {
        return None;
    }
    let preheader = incoming[index(header)]
        .iter()
        .copied()
        .find(|id| *id != last)?;
    if !matches!(body.blocks[preheader].terminator, mir::Terminator::Goto(target) if target == header)
    {
        return None;
    }
    let mut aliases = BTreeMap::new();
    let mut header_locals = BTreeSet::new();
    for statement in header_chain
        .iter()
        .flat_map(|id| &body.blocks[*id].statements)
    {
        let (local, value) = definition(statement)?;
        if !header_locals.insert(local) {
            return None;
        }
        aliases.insert(local, resolve(value, &aliases).clone());
    }
    let mir::ExprKind::IntegerCompare {
        operation,
        lhs,
        rhs,
    } = &cond.kind
    else {
        return None;
    };
    if operation.operator() != mir::IntegerComparisonOperator::LessThan {
        return None;
    }
    let (lhs, rhs) = match &resolve(lhs, &aliases).kind {
        mir::ExprKind::IntegerCompareTo { lhs: a, rhs: b, .. }
            if integer(resolve(rhs, &aliases)) == Some(0) =>
        {
            (resolve(a, &aliases), resolve(b, &aliases))
        }
        _ => (resolve(lhs, &aliases), resolve(rhs, &aliases)),
    };
    let mir::ExprKind::Local(iv) = lhs.kind else {
        return None;
    };
    let mir::Type::Integer(kind) = lhs.ty else {
        return None;
    };
    if ![mir::IntegerKind::SIGNED_32, mir::IntegerKind::SIGNED_64].contains(&kind)
        || rhs.ty != lhs.ty
    {
        return None;
    }
    let bound_local = match rhs.kind {
        mir::ExprKind::Local(local) if function.params.iter().any(|param| param.local == local) => {
            Some(local)
        }
        mir::ExprKind::IntegerLiteral(_) => None,
        _ => return None,
    };
    let mut temporaries = BTreeSet::new();
    let mut reassigned = BTreeSet::new();
    for local in body_chain
        .iter()
        .flat_map(|id| &body.blocks[*id].statements)
        .filter_map(definition)
        .map(|(local, _)| local)
    {
        if !temporaries.insert(local) {
            reassigned.insert(local);
        }
    }
    for param in &function.params {
        temporaries.remove(&param.local);
        if header_locals.contains(&param.local) {
            return None;
        }
    }
    let mut writes = Vec::new();
    for (id, candidate) in body.blocks.iter() {
        for statement in &candidate.statements {
            let local = definition(statement)
                .map(|(local, _)| local)
                .or(match &statement.kind {
                    mir::StatementKind::Call(mir::CallEffect::Value { destination, .. }) => {
                        Some(*destination)
                    }
                    _ => None,
                });
            if let Some(local) = local {
                if !body_chain.contains(&id) {
                    temporaries.remove(&local);
                }
                if local == iv {
                    writes.push((id, statement));
                }
                if Some(local) == bound_local
                    || (!header_chain.contains(&id) && header_locals.contains(&local))
                {
                    return None;
                }
            }
        }
    }
    let mut valid = true;
    for (id, candidate) in body.blocks.iter() {
        mir::visit_block_exprs(candidate, &mut |expr| {
            if matches!(expr.kind, mir::ExprKind::Local(local)
                if (!body_chain.contains(&id) && temporaries.contains(&local))
                    || (!header_chain.contains(&id) && header_locals.contains(&local)))
                || matches!(expr.kind, mir::ExprKind::AddressOf { local, .. }
                    if local == iv || Some(local) == bound_local
                        || header_locals.contains(&local) || temporaries.contains(&local))
            {
                valid = false;
            }
        });
    }
    if !valid || writes.len() != 2 {
        return None;
    }
    let initial = writes
        .iter()
        .find_map(|(id, statement)| (*id == preheader).then_some(*statement))?;
    let (_, init) = definition(initial)?;
    let start = integer(init)?;
    if start < 0 || integer(rhs).is_some_and(|end| end - start <= i128::from(CHUNK)) {
        return None;
    }
    let bound = rhs.clone();
    let update = writes
        .iter()
        .find_map(|(id, statement)| body_chain.contains(id).then_some(*statement))?;
    aliases.clear();
    for statement in body_chain
        .iter()
        .flat_map(|id| &body.blocks[*id].statements)
    {
        let Some((local, value)) = definition(statement) else {
            continue;
        };
        if std::ptr::eq(statement, update) {
            let mir::ExprKind::IntegerBinary {
                operation,
                lhs,
                rhs,
            } = &resolve(value, &aliases).kind
            else {
                return None;
            };
            if operation.kind() != kind
                || operation.operator() != mir::IntegerBinaryOperator::Add
                || !matches!(resolve(lhs, &aliases).kind, mir::ExprKind::Local(local) if local == iv)
                || integer(resolve(rhs, &aliases)) != Some(1)
            {
                return None;
            }
            break;
        }
        // Earlier expressions read the old value of a reassigned local.
        if temporaries.contains(&local) && !reassigned.contains(&local) {
            aliases.insert(local, resolve(value, &aliases).clone());
        } else {
            aliases.remove(&local);
        }
    }
    Some(Loop {
        header,
        body: inner,
        index: iv,
        bound,
        kind,
        header_chain,
        body_chain,
        temporaries,
    })
}

fn resolve<'a>(
    expression: &'a mir::Expr,
    aliases: &'a BTreeMap<mir::LocalId, mir::Expr>,
) -> &'a mir::Expr {
    if let mir::ExprKind::Local(local) = expression.kind {
        aliases.get(&local).unwrap_or(expression)
    } else {
        expression
    }
}

fn integer(expression: &mir::Expr) -> Option<i128> {
    match expression.kind {
        mir::ExprKind::IntegerLiteral(value) => Some(value.mathematical_value()),
        _ => None,
    }
}

fn definition(statement: &mir::Statement) -> Option<(mir::LocalId, &mir::Expr)> {
    match &statement.kind {
        mir::StatementKind::ValDecl { local, init } => Some((*local, init)),
        mir::StatementKind::Assign { local, value } => Some((*local, value)),
        _ => None,
    }
}
