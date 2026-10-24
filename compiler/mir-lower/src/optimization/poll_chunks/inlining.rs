//! Reuse counted-loop facts to specialize an actual inline, without new guards.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    Loop,
    checks::facts::{State, Value},
    header, linear, mir, operations, recognize,
};

pub(in crate::optimization) struct CallRanges<'a> {
    caller: &'a mir::Function,
    loops: Vec<(header::Header, Loop)>,
}

impl<'a> CallRanges<'a> {
    pub fn new(module: &mir::Module, caller: &'a mir::Function) -> Self {
        let incoming = linear::predecessors(&caller.body);
        let loops = caller
            .body
            .loop_header_polls
            .iter()
            .filter_map(|poll| {
                let head = header::analyze(module, caller, poll.header(), &incoming)?;
                head.invariants.length(&head.bound)?;
                let shape = recognize::shape(caller, poll.header(), &head, &incoming)?;
                Some((head, shape))
            })
            .collect();
        Self { caller, loops }
    }

    pub fn blocks(
        &self,
        module: &mir::Module,
        site: (mir::BlockId, usize),
        call: &mir::Call,
        callee: &mir::Function,
        blocks: &[(mir::BlockId, mir::BasicBlock)],
    ) -> Option<Vec<(mir::BlockId, mir::BasicBlock)>> {
        let (head, shape) = self
            .loops
            .iter()
            .find(|(_, shape)| shape.body_chain.contains(&site.0))?;
        let mut state = State::new(head);
        for &id in &shape.header_chain {
            for statement in &self.caller.body.blocks[id].statements {
                state.statement(statement);
            }
        }
        for &id in &shape.body_chain {
            let statements = &self.caller.body.blocks[id].statements;
            let prefix = if id == site.0 {
                site.1
            } else {
                statements.len()
            };
            for statement in &statements[..prefix] {
                state.statement(statement);
            }
            if id == site.0 {
                break;
            }
        }
        specialize(module, callee, blocks, state.bind(callee, &call.args))
    }
}

fn specialize(
    module: &mir::Module,
    callee: &mir::Function,
    blocks: &[(mir::BlockId, mir::BasicBlock)],
    mut state: State<'_>,
) -> Option<Vec<(mir::BlockId, mir::BasicBlock)>> {
    let source = blocks
        .iter()
        .map(|(id, block)| (*id, block))
        .collect::<BTreeMap<_, _>>();
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    let mut current = callee.body.entry;
    let (mut branches, mut array_access) = (false, false);
    loop {
        if !seen.insert(current) {
            return None;
        }
        let block = source.get(&current)?;
        if block.unwind.is_some() || !operations::supported(module, block, true) {
            return None;
        }
        mir::visit_block_exprs(block, &mut |expr| {
            array_access |= matches!(expr.kind, mir::ExprKind::ArrayGet { .. });
        });
        let mut block = (**block).clone();
        for statement in &mut block.statements {
            array_access |= matches!(statement.kind, mir::StatementKind::ArraySet { .. });
            mir::visit_statement_exprs_mut(statement, &mut |expr| {
                if let Value::Boolean(value) = state.expression(expr) {
                    *expr = mir::Expr::bool(value);
                }
            });
            state.statement(statement);
        }
        let next = match &block.terminator {
            mir::Terminator::Goto(next) => Some(*next),
            mir::Terminator::Branch {
                cond,
                then_block,
                else_block,
            } => {
                let Value::Boolean(value) = state.expression(cond) else {
                    return None;
                };
                let next = if value { *then_block } else { *else_block };
                block.terminator = mir::Terminator::Goto(next);
                branches = true;
                Some(next)
            }
            mir::Terminator::Return { .. } => None,
            _ => return None,
        };
        result.push((current, block));
        if let Some(next) = next {
            current = next;
        } else {
            break;
        }
    }
    (branches && array_access && state.guards.is_empty()).then_some(result)
}
