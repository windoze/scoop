use std::collections::{BTreeMap, BTreeSet};

use super::{
    linear, mir, operations,
    values::{Invariants, definition, integer, resolve},
};
use crate::optimization::graph::index;

pub(super) struct Header {
    pub chain: Vec<mir::BlockId>,
    pub inner: mir::BlockId,
    pub exit: mir::BlockId,
    pub preheader: mir::BlockId,
    pub index: mir::LocalId,
    pub kind: mir::IntegerKind,
    pub bound: mir::Expr,
    pub locals: BTreeSet<mir::LocalId>,
    pub invariants: Invariants,
}

pub(super) fn analyze(
    module: &mir::Module,
    function: &mir::Function,
    header: mir::BlockId,
    incoming: &[Vec<mir::BlockId>],
) -> Option<Header> {
    let body = &function.body;
    if header == body.entry || incoming[index(header)].len() != 2 {
        return None;
    }
    let chain = linear::chain(body, header, header, incoming)?;
    let mir::Terminator::Branch {
        cond,
        then_block,
        else_block,
    } = &body.blocks[*chain.last()?].terminator
    else {
        return None;
    };
    if chain.contains(then_block) || chain.contains(else_block) || then_block == else_block {
        return None;
    }
    let mut aliases = BTreeMap::new();
    let mut locals = BTreeSet::new();
    for &id in &chain {
        if !operations::supported(module, &body.blocks[id], false) {
            return None;
        }
        for statement in &body.blocks[id].statements {
            let (local, value) = definition(statement)?;
            if !locals.insert(local) {
                return None;
            }
            aliases.insert(local, resolve(value, &aliases).clone());
        }
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
    let mut entries = incoming[index(header)].iter().copied().filter(|&id| {
        matches!(body.blocks[id].terminator, mir::Terminator::Goto(next) if next == header)
            && body.blocks[id].statements.iter().any(|statement| {
                definition(statement).is_some_and(|(local, value)| {
                    local == iv && integer(value).is_some_and(|value| value >= 0)
                })
            })
    });
    let preheader = entries.next()?;
    if entries.next().is_some() {
        return None;
    }
    let invariants = Invariants::new(function, header, *then_block, *else_block);
    let bound = match &rhs.kind {
        mir::ExprKind::IntegerLiteral(_) => rhs.clone(),
        mir::ExprKind::Local(local) if invariants.contains(*local) => rhs.clone(),
        mir::ExprKind::ArrayLen {
            array_type,
            operand,
        } => {
            let operand = resolve(operand, &aliases);
            let array = invariants.array(operand)?;
            if array.array_type != *array_type {
                return None;
            }
            array.expression()
        }
        _ => return None,
    };
    Some(Header {
        chain,
        inner: *then_block,
        exit: *else_block,
        preheader,
        index: iv,
        kind,
        bound,
        locals,
        invariants,
    })
}
