use std::collections::{BTreeMap, BTreeSet};

use super::{CHUNK, Loop, mir};
use crate::optimization::graph::{block_edges, index};

pub(super) fn counted(function: &mir::Function, header: mir::BlockId) -> Option<Loop> {
    let body = &function.body;
    let head = &body.blocks[header];
    let mir::Terminator::Branch {
        cond,
        then_block: inner,
        else_block: exit,
    } = &head.terminator
    else {
        return None;
    };
    if header == *inner || inner == exit || head.unwind.is_some() {
        return None;
    }
    let inner = *inner;
    let block = &body.blocks[inner];
    if !matches!(block.terminator, mir::Terminator::Goto(target) if target == header)
        || block.unwind.is_some()
        || !pure(head)
        || !pure(block)
    {
        return None;
    }
    let edges = block_edges(body);
    let predecessors = |target| {
        edges
            .iter()
            .enumerate()
            .filter_map(|(from, edges)| edges.contains(&index(target)).then_some(from))
            .collect::<Vec<_>>()
    };
    if predecessors(inner) != [index(header)] || body.entry == header || body.entry == inner {
        return None;
    }
    let incoming = predecessors(header);
    if incoming.len() != 2 {
        return None;
    }
    let preheader = body
        .blocks
        .iter()
        .find_map(|(id, _)| (id != inner && incoming.contains(&index(id))).then_some(id))?;
    if !matches!(body.blocks[preheader].terminator, mir::Terminator::Goto(target) if target == header)
    {
        return None;
    }
    let mut aliases = BTreeMap::new();
    let mut header_locals = BTreeSet::new();
    for statement in &head.statements {
        let mir::StatementKind::ValDecl { local, init } = &statement.kind else {
            return None;
        };
        header_locals.insert(*local);
        aliases.insert(*local, resolve(init, &aliases).clone());
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
    let mut valid = true;
    let mut writes = Vec::new();
    for (id, candidate) in body.blocks.iter() {
        mir::visit_block_exprs(candidate, &mut |expr| {
            if matches!(expr.kind, mir::ExprKind::AddressOf { local, .. } if local == iv || Some(local) == bound_local)
                || (id != header
                    && matches!(expr.kind, mir::ExprKind::Local(local) if header_locals.contains(&local)))
            {
                valid = false;
            }
        });
        for statement in &candidate.statements {
            if let Some((local, _)) = definition(statement) {
                if local == iv {
                    writes.push((id, statement));
                }
                if Some(local) == bound_local {
                    valid = false;
                }
            } else if matches!(&statement.kind, mir::StatementKind::Call(mir::CallEffect::Value { destination, .. }) if *destination == iv || Some(*destination) == bound_local)
            {
                valid = false;
            }
        }
    }
    if !valid || writes.len() != 2 {
        return None;
    }
    let initial = writes
        .iter()
        .find_map(|(id, statement)| (*id == preheader).then_some(*statement))?;
    let (_, init) = definition(initial)?;
    // The independently entered preheader assigns the initial nonnegative value.
    let start = integer(init)?;
    if start < 0 || integer(rhs).is_some_and(|end| end - start <= i128::from(CHUNK)) {
        return None;
    }
    let bound = rhs.clone();
    let update = writes
        .iter()
        .find_map(|(id, statement)| (*id == inner).then_some(*statement))?;
    aliases.clear();
    for statement in &block.statements {
        let Some((local, value)) = definition(statement) else {
            continue;
        };
        if std::ptr::eq(statement, update) {
            let mir::ExprKind::IntegerBinary {
                operation,
                lhs,
                rhs,
            } = &value.kind
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
        if matches!(statement.kind, mir::StatementKind::ValDecl { .. }) {
            aliases.insert(local, resolve(value, &aliases).clone());
        } else {
            // Mutable aliases require snapshots; this narrow recognizer does
            // not infer an index update through an earlier mutable assignment.
            aliases.remove(&local);
        }
    }
    Some(Loop {
        header,
        body: inner,
        index: iv,
        bound,
        kind,
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

fn pure(block: &mir::BasicBlock) -> bool {
    if block.statements.iter().any(|statement| {
        !matches!(
            statement.kind,
            mir::StatementKind::ValDecl { .. }
                | mir::StatementKind::Assign { .. }
                | mir::StatementKind::Expr(_)
        )
    }) {
        return false;
    }
    let mut pure = true;
    mir::visit_block_exprs(block, &mut |expr| {
        pure &= matches!(
            expr.kind,
            mir::ExprKind::IntegerLiteral(_)
                | mir::ExprKind::BoolLiteral(_)
                | mir::ExprKind::UnitLiteral
                | mir::ExprKind::Local(_)
                | mir::ExprKind::IntegerUnary { .. }
                | mir::ExprKind::IntegerBinary { .. }
                | mir::ExprKind::IntegerCompare { .. }
                | mir::ExprKind::IntegerCompareTo { .. }
                | mir::ExprKind::IntegerShift { .. }
                | mir::ExprKind::IntegerConversion { .. }
                | mir::ExprKind::Unary { .. }
                | mir::ExprKind::Binary {
                    op: mir::BinOp::BoolEq | mir::BinOp::BoolNe,
                    ..
                }
        ) && matches!(
            expr.ty,
            mir::Type::Integer(_) | mir::Type::Boolean | mir::Type::Unit
        );
    });
    pure
}
