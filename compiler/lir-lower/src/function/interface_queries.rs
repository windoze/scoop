//! Lazily reuse exact-interface queries for stable sources inside loops.

use std::collections::{HashMap, HashSet};

use super::*;

mod candidates;

type Source = (mir::LocalId, mir::InterfaceId);

#[derive(Clone, Copy)]
pub(super) struct Query {
    pub entry: lir::LocalId,
    initialized: lir::LocalId,
}

#[derive(Default)]
pub(super) struct QueryCache(HashMap<Source, Query>);

impl QueryCache {
    pub fn get(&self, operand: &mir::Expr, interface: mir::InterfaceId) -> Option<Query> {
        self.source((interface_tables::source_local(operand)?, interface))
    }

    pub fn source(&self, source: Source) -> Option<Query> {
        self.0.get(&source).copied()
    }
}

impl FunctionLowerer<'_> {
    pub(super) fn prepare_interface_queries(
        &mut self,
        function: &mir::Function,
        addressed: &HashSet<mir::LocalId>,
        entry: lir::BlockId,
    ) -> StorageResult<lir::BlockId> {
        let candidates = candidates::select(function, addressed);
        if candidates.is_empty() {
            return Ok(entry);
        }
        // A separate prologue runs only once, even when MIR entry has a backedge.
        let prologue = self.new_block("interface.init");
        self.enter(prologue);
        for source in candidates {
            let query = Query {
                entry: self.new_hidden_local(lir::METADATA_PTR)?,
                initialized: self.new_hidden_local(lir::LirType::I1)?,
            };
            self.push(lir::Instruction::Store {
                local: query.entry,
                value: lir::Value::NullPointer(lir::PointerKind::Metadata),
            });
            self.push(lir::Instruction::Store {
                local: query.initialized,
                value: lir::Value::BoolConst(false),
            });
            self.interface_queries.0.insert(source, query);
        }
        self.seal(lir::Terminator::Br(entry));
        Ok(prologue)
    }

    pub(super) fn interface_entry(
        &mut self,
        operand: &mir::Expr,
        interface: mir::InterfaceId,
    ) -> StorageResult<lir::Value> {
        let Some(query) = self.interface_queries.get(operand, interface) else {
            return self.find_interface_entry(operand, interface);
        };
        let find = self.new_block("interface.query");
        let ready = self.new_block("interface.ready");
        self.seal(lir::Terminator::CondBr {
            cond: lir::Value::Local(query.initialized),
            then_block: ready,
            else_block: find,
        });
        self.enter(find);
        let entry = self.find_interface_entry(operand, interface)?;
        self.push(lir::Instruction::Store {
            local: query.entry,
            value: entry,
        });
        self.push(lir::Instruction::Store {
            local: query.initialized,
            value: lir::Value::BoolConst(true),
        });
        self.seal(lir::Terminator::Br(ready));
        self.enter(ready);
        Ok(lir::Value::Local(query.entry))
    }

    fn find_interface_entry(
        &mut self,
        operand: &mir::Expr,
        interface: mir::InterfaceId,
    ) -> StorageResult<lir::Value> {
        let value = self.lower_expr(operand)?;
        let object = self.reference_object(value, &operand.ty);
        let descriptor = self.load_at_offset(
            object,
            self.context.object_type_descriptor_offset(),
            lir::METADATA_PTR,
        );
        self.emit_plain_call(
            LoweredCallDestination::no_gc_runtime(lir::NoGcRuntimeFunction::ITableFind),
            vec![
                lir::Value::Temp(descriptor),
                self.td_ref(&mir::Type::Interface(interface)),
            ],
        )
    }

    pub(super) fn interface_entry_condition(
        &mut self,
        entry: lir::Value,
        negated: bool,
    ) -> lir::Value {
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
        lir::Value::Temp(result)
    }
}
