//! Function-local allocation origins. A mixed value keeps every candidate.

use std::collections::{BTreeSet, VecDeque};

use la_arena::{Arena, Idx};

use super::{control::ControlFlow, index, mir};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Site {
    pub block: mir::BlockId,
    pub statement: usize,
}

pub(super) struct Allocation {
    pub site: Site,
    pub local: mir::LocalId,
    pub payload: mir::Expr,
}

pub(super) type AllocationId = Idx<Allocation>;

#[derive(Clone, Default)]
pub(super) struct Origins {
    pub candidates: BTreeSet<AllocationId>,
    pub external: bool,
}

impl Origins {
    pub fn unique(&self) -> Option<AllocationId> {
        if !self.external && self.candidates.len() == 1 {
            self.candidates.first().copied()
        } else {
            None
        }
    }

    fn extend(&mut self, other: &Self) -> bool {
        let previous = (self.candidates.len(), self.external);
        self.candidates.extend(&other.candidates);
        self.external |= other.external;
        previous != (self.candidates.len(), self.external)
    }
}

pub(super) struct Sources {
    pub allocations: Arena<Allocation>,
    pub locals: Vec<Origins>,
}

impl Sources {
    pub fn new(function: &mir::Function, control: &ControlFlow) -> Self {
        let mut sources = Self {
            allocations: Arena::new(),
            locals: vec![Origins::default(); function.body.locals.len()],
        };
        let mut edges = vec![Vec::new(); sources.locals.len()];
        for parameter in &function.params {
            sources.locals[index(parameter.local)].external = true;
        }
        for (block_id, block) in function.body.blocks.iter() {
            if !control.reachable(block_id) {
                continue;
            }
            for (statement_index, statement) in block.statements.iter().enumerate() {
                let Some(destination) = defined_local(statement) else {
                    continue;
                };
                let value = match &statement.kind {
                    mir::StatementKind::ValDecl { init, .. } => {
                        if let mir::ExprKind::Box(payload) = &init.kind {
                            let allocation = sources.allocations.alloc(Allocation {
                                site: Site {
                                    block: block_id,
                                    statement: statement_index,
                                },
                                local: destination,
                                payload: (**payload).clone(),
                            });
                            sources.locals[index(destination)]
                                .candidates
                                .insert(allocation);
                            continue;
                        }
                        Some(init)
                    }
                    mir::StatementKind::Assign { value, .. } => Some(value),
                    _ => None,
                };
                if let Some(source) = value.and_then(local_alias) {
                    edges[index(source)].push(destination);
                } else {
                    sources.locals[index(destination)].external = true;
                }
            }
            mir::visit_block_exprs(block, &mut |expr| {
                if let mir::ExprKind::AddressOf { local, .. } = expr.kind {
                    // Unknown writes through this address also affect later aliases.
                    sources.locals[index(local)].external = true;
                }
            });
        }
        let mut pending = sources
            .locals
            .iter()
            .enumerate()
            .filter_map(|(id, origins)| {
                (origins.external || !origins.candidates.is_empty()).then_some(id)
            })
            .collect::<VecDeque<_>>();
        let mut queued = vec![false; sources.locals.len()];
        for &id in &pending {
            queued[id] = true;
        }
        while let Some(source) = pending.pop_front() {
            queued[source] = false;
            let origins = sources.locals[source].clone();
            for &destination in &edges[source] {
                let destination = index(destination);
                if sources.locals[destination].extend(&origins) && !queued[destination] {
                    queued[destination] = true;
                    pending.push_back(destination);
                }
            }
        }
        sources
    }
}

pub(super) fn local_alias(mut expr: &mir::Expr) -> Option<mir::LocalId> {
    loop {
        match &expr.kind {
            mir::ExprKind::Local(local) => return Some(*local),
            mir::ExprKind::Retype { operand, .. } => expr = operand,
            _ => return None,
        }
    }
}

pub(super) fn defined_local(statement: &mir::Statement) -> Option<mir::LocalId> {
    match statement.kind {
        mir::StatementKind::ValDecl { local, .. }
        | mir::StatementKind::Assign { local, .. }
        | mir::StatementKind::Call(mir::CallEffect::Value {
            destination: local, ..
        }) => Some(local),
        _ => None,
    }
}

pub(super) fn statement_alias(statement: &mir::Statement) -> Option<mir::LocalId> {
    match &statement.kind {
        mir::StatementKind::ValDecl { init: value, .. }
        | mir::StatementKind::Assign { value, .. }
        | mir::StatementKind::Expr(value) => local_alias(value),
        _ => None,
    }
}
