use std::collections::{HashSet, VecDeque};

use super::mir;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ActualType {
    Unknown,
    Class(mir::ClassId),
    Closure(mir::ClosureClassId),
}

pub(super) type State = Vec<ActualType>;

pub(super) struct Analysis<'a> {
    module: &'a mir::Module,
    function: &'a mir::Function,
    addressed: HashSet<mir::LocalId>,
}

impl<'a> Analysis<'a> {
    pub fn new(module: &'a mir::Module, function: &'a mir::Function) -> Self {
        let mut addressed = HashSet::new();
        for (_, block) in function.body.blocks.iter() {
            mir::visit_block_exprs(block, &mut |expression| {
                if let mir::ExprKind::AddressOf { local, .. } = expression.kind {
                    addressed.insert(local);
                }
            });
        }
        Self {
            module,
            function,
            addressed,
        }
    }

    pub fn solve(&self) -> Vec<Option<State>> {
        let body = &self.function.body;
        let mut incoming = vec![None; body.blocks.len()];
        let mut initial = vec![ActualType::Unknown; body.locals.len()];
        for parameter in &self.function.params {
            self.assign(
                parameter.local,
                self.static_type(&parameter.ty),
                &mut initial,
            );
        }
        incoming[index(body.entry)] = Some(initial);
        let mut pending = VecDeque::from([body.entry]);
        let mut queued = vec![false; body.blocks.len()];
        queued[index(body.entry)] = true;
        while let Some(block_id) = pending.pop_front() {
            queued[index(block_id)] = false;
            let block = &body.blocks[block_id];
            let mut state = incoming[index(block_id)]
                .clone()
                .expect("a queued block is reachable");
            let mut exceptional = state.clone();
            for statement in &block.statements {
                self.statement(statement, &mut state);
                if block.unwind.is_some() {
                    merge(&mut exceptional, &state);
                }
            }
            let mut propagate = |target, state: &State| {
                let changed = match &mut incoming[index(target)] {
                    Some(previous) => merge(previous, state),
                    slot @ None => {
                        *slot = Some(state.clone());
                        true
                    }
                };
                if changed && !queued[index(target)] {
                    queued[index(target)] = true;
                    pending.push_back(target);
                }
            };
            if let Some(unwind) = block.unwind {
                propagate(unwind, &exceptional);
            }
            match &block.terminator {
                mir::Terminator::Goto(target) => propagate(*target, &state),
                mir::Terminator::Branch {
                    cond,
                    then_block,
                    else_block,
                } => match cond.kind {
                    mir::ExprKind::BoolLiteral(true) => propagate(*then_block, &state),
                    mir::ExprKind::BoolLiteral(false) => propagate(*else_block, &state),
                    _ => {
                        propagate(*then_block, &state);
                        propagate(*else_block, &state);
                    }
                },
                mir::Terminator::Throw {
                    unwind: Some(target),
                    ..
                }
                | mir::Terminator::Rethrow {
                    unwind: Some(target),
                } => propagate(*target, &state),
                mir::Terminator::Return { .. }
                | mir::Terminator::Throw { unwind: None, .. }
                | mir::Terminator::Rethrow { unwind: None }
                | mir::Terminator::Resume
                | mir::Terminator::Trap { .. }
                | mir::Terminator::Unreachable => {}
            }
        }
        incoming
    }

    pub fn statement(&self, statement: &mir::Statement, state: &mut State) {
        match &statement.kind {
            mir::StatementKind::ValDecl { local, init: value }
            | mir::StatementKind::Assign { local, value } => {
                self.assign(*local, self.expression(value, state), state);
            }
            mir::StatementKind::Call(mir::CallEffect::Value { destination, .. }) => {
                self.assign(
                    *destination,
                    self.static_type(&self.function.body.locals[*destination].ty),
                    state,
                );
            }
            mir::StatementKind::Expr(_)
            | mir::StatementKind::Call(mir::CallEffect::Unit(_))
            | mir::StatementKind::PublishReleaseReady { .. }
            | mir::StatementKind::GlobalAssign { .. }
            | mir::StatementKind::ArraySet { .. }
            | mir::StatementKind::FieldSet { .. }
            | mir::StatementKind::AtomicFieldStore { .. }
            | mir::StatementKind::Eh(_) => {}
        }
    }

    fn assign(&self, local: mir::LocalId, value: ActualType, state: &mut State) {
        state[local.into_raw().into_u32() as usize] = if self.addressed.contains(&local) {
            ActualType::Unknown
        } else {
            value
        };
    }

    fn static_type(&self, ty: &mir::Type) -> ActualType {
        match ty {
            mir::Type::Class(class)
                if self.module.classes[*class].modifier == mir::ClassModifier::Final =>
            {
                ActualType::Class(*class)
            }
            _ => ActualType::Unknown,
        }
    }

    pub fn expression(&self, expression: &mir::Expr, state: &State) -> ActualType {
        match &expression.kind {
            mir::ExprKind::Local(local) => {
                if self.addressed.contains(local) {
                    ActualType::Unknown
                } else {
                    state[local.into_raw().into_u32() as usize]
                }
            }
            mir::ExprKind::ClassAlloc { class_id } => ActualType::Class(*class_id),
            mir::ExprKind::ClosureAlloc { class, .. } => ActualType::Closure(*class),
            mir::ExprKind::ArrayAllocate { array_type, .. }
            | mir::ExprKind::ArrayLiteral { array_type, .. }
            | mir::ExprKind::ArrayAssembly { array_type, .. } => ActualType::Class(*array_type),
            mir::ExprKind::ArrayClone { target_type, .. } => ActualType::Class(*target_type),
            mir::ExprKind::Retype { operand, .. }
            | mir::ExprKind::Cast {
                operand,
                optional: false,
            } => {
                let actual = self.expression(operand, state);
                if actual == ActualType::Unknown {
                    self.static_type(&expression.ty)
                } else {
                    actual
                }
            }
            mir::ExprKind::Box(value) => self
                .module
                .meta
                .boxed_types
                .iter()
                .find(|boxed| boxed.payload() == &value.ty)
                .map_or(ActualType::Unknown, |boxed| {
                    ActualType::Class(boxed.class())
                }),
            _ => self.static_type(&expression.ty),
        }
    }
}

fn index(block: mir::BlockId) -> usize {
    block.into_raw().into_u32() as usize
}

fn merge(target: &mut State, source: &State) -> bool {
    let mut changed = false;
    for (target, source) in target.iter_mut().zip(source) {
        if *target != *source && *target != ActualType::Unknown {
            *target = ActualType::Unknown;
            changed = true;
        }
    }
    changed
}
