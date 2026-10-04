use super::*;

impl FunctionLowerer<'_> {
    pub(super) fn lower_context_operation(
        &mut self,
        operation: &mir::ContextOperation<mir::Expr>,
        ty: &mir::Type,
    ) -> StorageResult<lir::Value> {
        use lir::{ManagedRuntimeFunction as Managed, NoGcRuntimeFunction as Leaf};
        use mir::ContextOperation as Op;
        let managed = LoweredCallDestination::managed_runtime;
        let leaf = LoweredCallDestination::no_gc_runtime;
        let pointer = lir::MANAGED_PTR;
        match operation {
            Op::TryGet { key } => self.emit_plain_call(
                leaf(Leaf::ContextTryGet),
                vec![lir::RAW_PTR],
                pointer,
                vec![lir::Value::ContextKeyCell(*key)],
            ),
            Op::Push { key, value } => {
                let value = self.lower_expr(value)?;
                let task = self.emit_plain_call(
                    leaf(Leaf::ContextCurrent),
                    vec![],
                    pointer.clone(),
                    vec![],
                )?;
                let descriptor = self.context_descriptor(mir::ContextStorageRole::Node);
                let root = self.emit_plain_call(
                    managed(Managed::ContextPush),
                    vec![lir::RAW_PTR, pointer.clone(), lir::METADATA_PTR],
                    pointer,
                    vec![lir::Value::ContextKeyCell(*key), value, descriptor],
                )?;
                Ok(self.make_aggregate(ty, vec![task, root]))
            }
            Op::Restore { mark } => {
                let mark = self.lower_expr(mark)?;
                let task = self.context_field(mark, 0);
                let root = self.context_field(mark, 1);
                self.emit_plain_call(
                    leaf(Leaf::ContextRestore),
                    vec![pointer.clone(), pointer],
                    lir::LirType::Void,
                    vec![task, root],
                )
            }
            Op::Snapshot => {
                self.emit_plain_call(leaf(Leaf::ContextSnapshot), vec![], pointer, vec![])
            }
            Op::Fork { root } => {
                let root = self.lower_expr(root)?;
                let descriptor = self.context_descriptor(mir::ContextStorageRole::Task);
                self.emit_plain_call(
                    managed(Managed::ContextFork),
                    vec![pointer.clone(), lir::METADATA_PTR],
                    pointer,
                    vec![root, descriptor],
                )
            }
            Op::Enter { task } => {
                let task = self.lower_expr(task)?;
                let previous = self.emit_plain_call(
                    leaf(Leaf::ContextEnter),
                    vec![pointer.clone()],
                    pointer,
                    vec![task],
                )?;
                Ok(self.make_aggregate(ty, vec![previous]))
            }
            Op::Leave { guard } => {
                let guard = self.lower_expr(guard)?;
                let previous = self.context_field(guard, 0);
                self.emit_plain_call(
                    leaf(Leaf::ContextLeave),
                    vec![pointer],
                    lir::LirType::Void,
                    vec![previous],
                )
            }
            Op::IsPresent { binding } => {
                let binding = self.lower_expr(binding)?;
                let out = self.new_temp(lir::LirType::I1);
                self.push(lir::Instruction::BinOp {
                    out,
                    op: lir::BinOp::Ne,
                    lhs: binding,
                    rhs: lir::Value::NullPointer(lir::PointerKind::Managed),
                });
                Ok(lir::Value::Temp(out))
            }
            Op::UnwrapBinding { binding } => self.lower_expr(binding),
            Op::EnsureRoot => {
                let descriptor = self.context_descriptor(mir::ContextStorageRole::Task);
                self.emit_plain_call(
                    managed(Managed::ContextEnsureRoot),
                    vec![lir::METADATA_PTR],
                    pointer,
                    vec![descriptor],
                )
            }
        }
    }

    fn context_descriptor(&self, role: mir::ContextStorageRole) -> lir::Value {
        let storage = self
            .module
            .meta
            .generated_exact_types
            .iter()
            .find_map(|entry| match entry.location() {
                mir::GeneratedExactTypeLocation::Context(storage) if storage.role == role => {
                    Some(storage)
                }
                _ => None,
            })
            .expect("the MIR input retains its core context support types");
        self.td_ref(&mir::Type::Context(storage))
    }

    fn context_field(&mut self, aggregate: lir::Value, index: u32) -> lir::Value {
        let out = self.new_temp(lir::MANAGED_PTR);
        self.push(lir::Instruction::ExtractValue {
            out,
            aggregate,
            index,
        });
        lir::Value::Temp(out)
    }
}
