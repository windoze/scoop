use std::collections::{HashSet, VecDeque};

use super::{
    constants::{self, Constant},
    mir,
};

mod stability;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ActualType {
    Unknown,
    Class(mir::ClassId),
    Closure(mir::ClosureClassId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ValueFacts {
    pub actual: ActualType,
    pub constant: Option<Constant>,
    pub alias: Option<mir::LocalId>,
}

impl ValueFacts {
    pub const UNKNOWN: Self = Self {
        actual: ActualType::Unknown,
        constant: None,
        alias: None,
    };
}

pub(super) type State = Vec<ValueFacts>;

pub(super) struct Analysis<'a> {
    module: &'a mir::Module,
    function: &'a mir::Function,
    addressed: HashSet<mir::LocalId>,
    stable: Vec<bool>,
    initial: State,
    string_class: ActualType,
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
        let mut analysis = Self {
            module,
            function,
            stable: stability::locals(function, &addressed),
            addressed,
            initial: Vec::new(),
            string_class: module
                .classes
                .iter()
                .find_map(|(id, class)| {
                    (class.modifier == mir::ClassModifier::Final
                        && matches!(
                            class.representation,
                            mir::ClassRepresentation::Intrinsic(
                                mir::IntrinsicClassRepresentation::String
                            )
                        ))
                    .then_some(ActualType::Class(id))
                })
                .unwrap_or(ActualType::Unknown),
        };
        let mut initial = vec![ValueFacts::UNKNOWN; function.body.locals.len()];
        for parameter in &function.params {
            analysis.assign(
                parameter.local,
                analysis.static_facts(&parameter.ty),
                &mut initial,
            );
        }
        analysis.initial = initial;
        analysis
    }

    pub fn with_arguments(mut self, arguments: &[ValueFacts]) -> Self {
        for (parameter, argument) in self.function.params.iter().zip(arguments) {
            if !self.addressed.contains(&parameter.local) {
                let mut facts = *argument;
                // Local ids belong to one body; only semantic facts cross a call.
                facts.alias = None;
                if facts.actual == ActualType::Unknown {
                    facts.actual = self.static_type(&parameter.ty);
                }
                self.initial[parameter.local.into_raw().into_u32() as usize] = facts;
            }
        }
        self
    }

    pub fn solve(&self) -> Vec<Option<State>> {
        let body = &self.function.body;
        let mut incoming = vec![None; body.blocks.len()];
        incoming[index(body.entry)] = Some(self.initial.clone());
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
                } => match self.expression(cond, &state).constant {
                    Some(Constant::Boolean(true)) => propagate(*then_block, &state),
                    Some(Constant::Boolean(false)) => propagate(*else_block, &state),
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
            mir::StatementKind::Call(mir::CallEffect::Value { destination, call }) => {
                let mut facts = self.static_facts(&self.function.body.locals[*destination].ty);
                facts.constant = self.call_constant(call, state);
                self.assign(*destination, facts, state);
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

    pub fn call_constant(&self, call: &mir::Call, state: &State) -> Option<Constant> {
        constants::call(self.module, call, |operand| {
            self.expression(operand, state).constant
        })
    }

    fn assign(&self, local: mir::LocalId, value: ValueFacts, state: &mut State) {
        state[local.into_raw().into_u32() as usize] = if self.addressed.contains(&local) {
            ValueFacts::UNKNOWN
        } else {
            value
        };
    }

    fn static_facts(&self, ty: &mir::Type) -> ValueFacts {
        ValueFacts {
            actual: self.static_type(ty),
            constant: None,
            alias: None,
        }
    }

    fn static_type(&self, ty: &mir::Type) -> ActualType {
        match ty {
            mir::Type::String => self.string_class,
            mir::Type::Class(class)
                if self.module.classes[*class].modifier == mir::ClassModifier::Final =>
            {
                ActualType::Class(*class)
            }
            _ => ActualType::Unknown,
        }
    }

    pub fn expression(&self, expression: &mir::Expr, state: &State) -> ValueFacts {
        let actual = match &expression.kind {
            mir::ExprKind::Local(local) => {
                return if self.addressed.contains(local) {
                    ValueFacts::UNKNOWN
                } else {
                    let mut facts = state[local.into_raw().into_u32() as usize];
                    if facts.alias.is_none() && self.stable[local.into_raw().into_u32() as usize] {
                        facts.alias = Some(*local);
                    }
                    facts
                };
            }
            mir::ExprKind::ClassAlloc { class_id } => ActualType::Class(*class_id),
            mir::ExprKind::ClosureAlloc { class, .. } => ActualType::Closure(*class),
            mir::ExprKind::ArrayAllocate { array_type, .. }
            | mir::ExprKind::ArrayLiteral { array_type, .. }
            | mir::ExprKind::ArrayAssembly { array_type, .. } => ActualType::Class(*array_type),
            mir::ExprKind::ArrayClone { target_type, .. } => ActualType::Class(*target_type),
            mir::ExprKind::Retype { operand, .. } => {
                let actual = self.expression(operand, state).actual;
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
        };
        ValueFacts {
            actual,
            alias: None,
            constant: constants::evaluate(expression, |operand| {
                self.expression(operand, state).constant
            }),
        }
    }
}

fn index(block: mir::BlockId) -> usize {
    block.into_raw().into_u32() as usize
}

fn merge(target: &mut State, source: &State) -> bool {
    let mut changed = false;
    for (target, source) in target.iter_mut().zip(source) {
        if target.actual != source.actual && target.actual != ActualType::Unknown {
            target.actual = ActualType::Unknown;
            changed = true;
        }
        if target.constant != source.constant && target.constant.is_some() {
            target.constant = None;
            changed = true;
        }
        if target.alias != source.alias && target.alias.is_some() {
            target.alias = None;
            changed = true;
        }
    }
    changed
}
