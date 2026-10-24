use std::collections::{BTreeMap, BTreeSet};

use super::{
    CHUNK, Loop, header, linear, mir, operations,
    values::{definition, integer, resolve},
};
use crate::optimization::graph::index;

pub(super) fn counted(
    module: &mir::Module,
    function: &mir::Function,
    header: mir::BlockId,
    head: &header::Header,
    incoming: &[Vec<mir::BlockId>],
) -> Option<Loop> {
    let body = &function.body;
    let header_chain = &head.chain;
    let header_locals = &head.locals;
    let (inner, exit, preheader, iv, kind) =
        (head.inner, head.exit, head.preheader, head.index, head.kind);
    let bound = &head.bound;
    let body_chain = linear::chain(body, inner, header, incoming)?;
    let last = *body_chain.last()?;
    if body_chain.contains(&exit)
        || body_chain.iter().any(|id| {
            body.loop_header_polls
                .iter()
                .any(|poll| poll.header() == *id)
        })
        || incoming[index(inner)] != [*header_chain.last()?]
        || !matches!(body.blocks[last].terminator, mir::Terminator::Goto(target) if target == header)
        || last == preheader
        || !incoming[index(header)].contains(&last)
        || body_chain
            .iter()
            .any(|id| !operations::supported(module, &body.blocks[*id], true))
    {
        return None;
    }
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
                if !header_chain.contains(&id) && header_locals.contains(&local) {
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
                    if local == iv || header_locals.contains(&local) || temporaries.contains(&local))
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
    if start < 0
        || integer(head.invariants.resolve(bound))
            .is_some_and(|end| end - start <= i128::from(CHUNK))
    {
        return None;
    }
    let update = writes
        .iter()
        .find_map(|(id, statement)| body_chain.contains(id).then_some(*statement))?;
    let mut aliases = BTreeMap::new();
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
        bound: bound.clone(),
        kind,
        header_chain: header_chain.clone(),
        body_chain,
        temporaries,
    })
}
