use std::collections::{BTreeMap, BTreeSet};

use super::mir;
use crate::optimization::{
    allocation::ControlFlow,
    graph::{block_edges, index},
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct Length {
    pub array_type: mir::ClassId,
    pub array: mir::LocalId,
}

impl Length {
    pub fn expression(self) -> mir::Expr {
        mir::Expr::new(
            mir::Type::Integer(mir::IntegerKind::SIGNED_64),
            mir::ExprKind::ArrayLen {
                array_type: self.array_type,
                operand: Box::new(mir::Expr::local(
                    self.array,
                    mir::Type::Class(self.array_type),
                )),
            },
        )
    }
}

pub(super) struct Invariants {
    available: BTreeSet<mir::LocalId>,
    aliases: BTreeMap<mir::LocalId, mir::Expr>,
    control: ControlFlow,
}

impl Invariants {
    pub fn new(
        function: &mir::Function,
        header: mir::BlockId,
        inner: mir::BlockId,
        exit: mir::BlockId,
    ) -> Self {
        let body = &function.body;
        let mut writes = vec![0_u32; body.locals.len()];
        let mut addressed = BTreeSet::new();
        for parameter in &function.params {
            writes[index(parameter.local)] += 1;
        }
        for (_, block) in body.blocks.iter() {
            for statement in &block.statements {
                if let Some(local) = defined_local(statement) {
                    writes[index(local)] = writes[index(local)].saturating_add(1);
                }
            }
            mir::visit_block_exprs(block, &mut |expr| {
                if let mir::ExprKind::AddressOf { local, .. } = expr.kind {
                    addressed.insert(local);
                }
            });
        }
        let stable = |local| writes[index(local)] == 1 && !addressed.contains(&local);
        let mut available = function
            .params
            .iter()
            .map(|parameter| parameter.local)
            .filter(|local| stable(*local))
            .collect::<BTreeSet<_>>();
        let edges = block_edges(body);
        let mut region = vec![false; edges.len()];
        let mut pending = vec![index(inner)];
        while let Some(block) = pending.pop() {
            if block == index(header)
                || block == index(exit)
                || std::mem::replace(&mut region[block], true)
            {
                continue;
            }
            pending.extend(&edges[block]);
        }
        let control = ControlFlow::new(body);
        let mut aliases = BTreeMap::new();
        for (id, block) in body.blocks.iter() {
            if id == header
                || region[index(id)]
                || !control.block_dominates(index(id), index(header))
            {
                continue;
            }
            for statement in &block.statements {
                if let Some(local) = defined_local(statement)
                    && stable(local)
                {
                    available.insert(local);
                    if let Some((_, value)) = definition(statement) {
                        aliases.insert(local, value.clone());
                    }
                }
            }
        }
        Self {
            available,
            aliases,
            control,
        }
    }

    pub fn dominates(&self, definition: mir::BlockId, block: mir::BlockId) -> bool {
        self.control
            .block_dominates(index(definition), index(block))
    }

    pub fn contains(&self, local: mir::LocalId) -> bool {
        self.available.contains(&local)
    }

    pub fn resolve<'a>(&'a self, mut expression: &'a mir::Expr) -> &'a mir::Expr {
        let mut seen = BTreeSet::new();
        while let mir::ExprKind::Local(local) = expression.kind {
            if !self.contains(local) || !seen.insert(local) {
                break;
            }
            let Some(value) = self.aliases.get(&local) else {
                break;
            };
            match &value.kind {
                mir::ExprKind::Local(other) if self.contains(*other) => expression = value,
                mir::ExprKind::IntegerLiteral(_) | mir::ExprKind::ArrayLen { .. } => {
                    return value;
                }
                _ => break,
            }
        }
        expression
    }

    pub fn array(&self, expression: &mir::Expr) -> Option<Length> {
        let expression = self.resolve(expression);
        let mir::ExprKind::Local(array) = expression.kind else {
            return None;
        };
        let mir::Type::Class(array_type) = expression.ty else {
            return None;
        };
        self.contains(array).then_some(Length { array_type, array })
    }

    pub fn length(&self, expression: &mir::Expr) -> Option<Length> {
        let mir::ExprKind::ArrayLen { operand, .. } = &self.resolve(expression).kind else {
            return None;
        };
        self.array(operand)
    }
}

pub(super) fn resolve<'a>(
    expression: &'a mir::Expr,
    aliases: &'a BTreeMap<mir::LocalId, mir::Expr>,
) -> &'a mir::Expr {
    if let mir::ExprKind::Local(local) = expression.kind {
        aliases.get(&local).unwrap_or(expression)
    } else {
        expression
    }
}

pub(super) fn integer(expression: &mir::Expr) -> Option<i128> {
    match expression.kind {
        mir::ExprKind::IntegerLiteral(value) => Some(value.mathematical_value()),
        _ => None,
    }
}

pub(super) fn definition(statement: &mir::Statement) -> Option<(mir::LocalId, &mir::Expr)> {
    match &statement.kind {
        mir::StatementKind::ValDecl { local, init } => Some((*local, init)),
        mir::StatementKind::Assign { local, value } => Some((*local, value)),
        _ => None,
    }
}

pub(super) fn defined_local(statement: &mir::Statement) -> Option<mir::LocalId> {
    definition(statement)
        .map(|(local, _)| local)
        .or(match statement.kind {
            mir::StatementKind::Call(mir::CallEffect::Value { destination, .. }) => {
                Some(destination)
            }
            _ => None,
        })
}
