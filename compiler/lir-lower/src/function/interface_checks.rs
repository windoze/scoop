//! Fuse an interface guard with the views reached through its successful edge.

use std::collections::{HashMap, HashSet};

use super::*;

mod flow;
mod guards;

type Source = (mir::LocalId, mir::InterfaceId);
type Facts = HashMap<Source, mir::BlockId>;

#[derive(Clone, Copy)]
struct Check {
    source: Source,
    success: mir::BlockId,
    location: Location,
}

#[derive(Clone, Copy)]
pub(super) enum Location {
    Statement(usize),
    Terminator,
}

#[derive(Clone, Copy)]
pub(super) struct PlannedCheck {
    pub location: Location,
    pub entry: lir::LocalId,
}

#[derive(Default)]
pub(super) struct Plan {
    pub checks: HashMap<mir::BlockId, PlannedCheck>,
    pub incoming: HashMap<mir::BlockId, HashMap<Source, lir::LocalId>>,
}

impl Plan {
    pub fn new(
        module: &mir::Module,
        body: &mir::Body,
        addressed: &HashSet<mir::LocalId>,
        lowerer: &mut FunctionLowerer<'_>,
    ) -> StorageResult<Self> {
        let checks = body
            .blocks
            .iter()
            .filter_map(|(id, block)| guards::check(block, addressed).map(|check| (id, check)))
            .collect::<HashMap<_, _>>();
        if checks.is_empty() {
            return Ok(Self::default());
        }
        let mut incoming = flow::incoming(body, &checks);
        let mut used = HashSet::new();
        for (id, block) in body.blocks.iter() {
            let Some(mut state) = incoming.get(&id).cloned() else {
                continue;
            };
            for statement in &block.statements {
                mir::visit_statement_exprs(statement, &mut |expression| {
                    use_view(module, expression, &state, &mut used);
                });
                flow::written(statement, &mut state);
            }
            mir::visit_terminator_exprs(&block.terminator, &mut |expression| {
                use_view(module, expression, &state, &mut used);
            });
        }
        let mut storage = HashMap::new();
        // Keep local allocation deterministic even when CFG block order is arbitrary.
        for (id, _) in body.blocks.iter().filter(|(id, _)| used.contains(id)) {
            storage.insert(id, lowerer.new_hidden_local(lir::METADATA_PTR)?);
        }
        let mut entries = HashMap::new();
        for (id, facts) in &mut incoming {
            facts.retain(|_, check| used.contains(check));
            if !facts.is_empty() {
                entries.insert(
                    *id,
                    facts
                        .iter()
                        .map(|(key, check)| (*key, storage[check]))
                        .collect(),
                );
            }
        }
        Ok(Self {
            checks: storage
                .into_iter()
                .map(|(id, entry)| {
                    (
                        id,
                        PlannedCheck {
                            location: checks[&id].location,
                            entry,
                        },
                    )
                })
                .collect(),
            incoming: entries,
        })
    }
}

fn use_view(
    module: &mir::Module,
    expression: &mir::Expr,
    facts: &Facts,
    used: &mut HashSet<mir::BlockId>,
) {
    if let mir::Type::Interface(interface) = expression.ty
        && !module.interfaces[interface].methods.is_empty()
        && let mir::ExprKind::Retype { operand, .. } = &expression.kind
        && operand.ty != expression.ty
        && let Some(local) = interface_tables::source_local(operand)
        && let Some(check) = facts.get(&(local, interface))
    {
        used.insert(*check);
    }
}

fn condition(mut expression: &mir::Expr) -> Option<(&mir::Expr, mir::InterfaceId, bool)> {
    let mut negated = false;
    while let mir::ExprKind::Unary {
        op: mir::UnOp::BoolNot,
        operand,
    } = &expression.kind
    {
        negated = !negated;
        expression = operand;
    }
    if let mir::ExprKind::IsInstance { operand, check_ty } = &expression.kind
        && let mir::Type::Interface(interface) = **check_ty
    {
        Some((operand, interface, negated))
    } else {
        None
    }
}

impl FunctionLowerer<'_> {
    pub(super) fn checked_condition(
        &mut self,
        condition_expr: &mir::Expr,
        slot: lir::LocalId,
    ) -> StorageResult<lir::Value> {
        let (operand, interface, negated) =
            condition(condition_expr).expect("planned interface guard");
        let value = self.lower_expr(operand)?;
        let object = self.reference_object(value, &operand.ty);
        let descriptor = self.load_at_offset(
            object,
            self.context.object_type_descriptor_offset(),
            lir::METADATA_PTR,
        );
        let entry = self.emit_plain_call(
            LoweredCallDestination::no_gc_runtime(lir::NoGcRuntimeFunction::ITableFind),
            vec![
                lir::Value::Temp(descriptor),
                self.td_ref(&mir::Type::Interface(interface)),
            ],
        )?;
        self.push(lir::Instruction::Store {
            local: slot,
            value: entry,
        });
        let result = self.new_temp(lir::LirType::I1);
        self.push(lir::Instruction::BinOp {
            out: result,
            op: if negated {
                lir::BinOp::Eq
            } else {
                lir::BinOp::Ne
            },
            lhs: entry,
            rhs: lir::Value::NullPointer(lir::PointerKind::Metadata),
        });
        Ok(lir::Value::Temp(result))
    }
}
