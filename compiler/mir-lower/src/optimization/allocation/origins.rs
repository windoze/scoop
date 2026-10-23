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
    pub kind: AllocationKind,
}

pub(super) enum AllocationKind {
    Box(mir::Expr),
    Closure {
        class: mir::ClosureClassId,
        captures: Vec<mir::ClosureCaptureInit>,
    },
    Class(mir::ClassId),
}

impl AllocationKind {
    fn from_expression(module: &mir::Module, expr: &mir::Expr) -> Option<Self> {
        match &expr.kind {
            mir::ExprKind::Box(payload) => Some(Self::Box((**payload).clone())),
            mir::ExprKind::ClosureAlloc { class, captures } => Some(Self::Closure {
                class: *class,
                captures: captures.clone(),
            }),
            mir::ExprKind::ClassAlloc { class_id }
                if module.classes[*class_id].release_policy == mir::ReleasePolicy::None
                    && matches!(
                        module.classes[*class_id].representation,
                        mir::ClassRepresentation::Declared(_)
                    ) =>
            {
                Some(Self::Class(*class_id))
            }
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Projection {
    BoxPayload,
    Capture(u32),
    Field(u32),
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
    pub fn new(module: &mir::Module, function: &mir::Function, control: &ControlFlow) -> Self {
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
                    mir::StatementKind::ValDecl { init, .. }
                    | mir::StatementKind::Assign { value: init, .. } => {
                        if (matches!(statement.kind, mir::StatementKind::ValDecl { .. })
                            || matches!(init.kind, mir::ExprKind::ClassAlloc { .. }))
                            && let Some(kind) = AllocationKind::from_expression(module, init)
                        {
                            let allocation = sources.allocations.alloc(Allocation {
                                site: Site {
                                    block: block_id,
                                    statement: statement_index,
                                },
                                local: destination,
                                kind,
                            });
                            sources.locals[index(destination)]
                                .candidates
                                .insert(allocation);
                            continue;
                        }
                        Some(init)
                    }
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

pub(in crate::optimization) fn local_alias(mut expr: &mir::Expr) -> Option<mir::LocalId> {
    loop {
        match &expr.kind {
            mir::ExprKind::Local(local) => return Some(*local),
            mir::ExprKind::Retype { operand, .. } => expr = operand,
            _ => return None,
        }
    }
}

pub(in crate::optimization) fn defined_local(statement: &mir::Statement) -> Option<mir::LocalId> {
    match statement.kind {
        mir::StatementKind::ValDecl { local, .. }
        | mir::StatementKind::Assign { local, .. }
        | mir::StatementKind::Call(mir::CallEffect::Value {
            destination: local, ..
        }) => Some(local),
        _ => None,
    }
}

pub(in crate::optimization) fn statement_alias(statement: &mir::Statement) -> Option<mir::LocalId> {
    match &statement.kind {
        mir::StatementKind::ValDecl { init: value, .. }
        | mir::StatementKind::Assign { value, .. }
        | mir::StatementKind::Expr(value) => local_alias(value),
        _ => None,
    }
}

pub(in crate::optimization) fn is_class_field(
    module: &mir::Module,
    class: mir::ClassId,
    object: &mir::Expr,
    field: u32,
    ty: &mir::Type,
) -> bool {
    let mir::Type::Class(view) = object.ty else {
        return false;
    };
    let mir::ClassRepresentation::Declared(declared) = &module.classes[class].representation else {
        return false;
    };
    if !declared
        .fields
        .get(field as usize)
        .is_some_and(|f| f.ty == *ty)
    {
        return false;
    }
    let mut base = Some(class);
    while let Some(id) = base {
        if id == view {
            return true;
        }
        base = module.classes[id].base_class();
    }
    false
}
