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
        match operation {
            Op::TryGet { key } => self.emit_plain_call(
                leaf(Leaf::ContextTryGet),
                vec![lir::Value::ContextKeyCell(*key)],
            ),
            Op::Push { key, value } => {
                let lowered = self.lower_expr(value)?;
                let value = self.reference_object(lowered, &value.ty);
                let task = self.emit_plain_call(leaf(Leaf::ContextCurrent), vec![])?;
                let descriptor = self.context_descriptor(mir::ContextStorageRole::Node);
                let root = self.emit_plain_call(
                    managed(Managed::ContextPush),
                    vec![lir::Value::ContextKeyCell(*key), value, descriptor],
                )?;
                Ok(self.make_aggregate(ty, vec![task, root]))
            }
            Op::Restore { mark } => {
                let mark = self.lower_expr(mark)?;
                let task = self.context_field(mark, 0);
                let root = self.context_field(mark, 1);
                self.emit_plain_call(leaf(Leaf::ContextRestore), vec![task, root])
            }
            Op::Current => self.emit_plain_call(leaf(Leaf::ContextCurrent), vec![]),
            Op::Snapshot => self.context_snapshot(),
            Op::Fork { root } => {
                let root = self.lower_expr(root)?;
                let storage = self.context_storage(mir::ContextStorageRole::Task);
                let (fields, size, _) = context_object_shape(self.context, storage);
                let task = self.emit_plain_call(
                    managed(Managed::Alloc),
                    vec![
                        self.td_ref(&mir::Type::Context(storage)),
                        lir::Value::MachineScalar(lir::MachineScalarValue::ByteSize(size)),
                    ],
                )?;
                // Keep the snapshot live through allocation so the root plan
                // restores it before this heap write if allocation moves it.
                self.store_at_offset(task, fields[0].offset, root, lir::MANAGED_PTR);
                Ok(task)
            }
            Op::Enter { task } => {
                let task = self.lower_expr(task)?;
                let previous = self.emit_plain_call(leaf(Leaf::ContextEnter), vec![task])?;
                Ok(self.make_aggregate(ty, vec![previous]))
            }
            Op::Leave { guard } => {
                let guard = self.lower_expr(guard)?;
                let previous = self.context_field(guard, 0);
                self.emit_plain_call(leaf(Leaf::ContextLeave), vec![previous])
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
            Op::UnwrapBinding { binding } => {
                let object = self.lower_expr(binding)?;
                self.restore_reference_value(object, ty)
            }
            Op::EnsureRoot => {
                let descriptor = self.context_descriptor(mir::ContextStorageRole::Task);
                self.emit_plain_call(managed(Managed::ContextEnsureRoot), vec![descriptor])
            }
        }
    }

    fn context_snapshot(&mut self) -> StorageResult<lir::Value> {
        let task = self.emit_plain_call(
            LoweredCallDestination::no_gc_runtime(lir::NoGcRuntimeFunction::ContextCurrent),
            vec![],
        )?;
        let root = self.new_hidden_local(lir::MANAGED_PTR)?;
        let null = lir::Value::NullPointer(lir::PointerKind::Managed);
        self.push(lir::Instruction::Store {
            local: root,
            value: null,
        });
        let non_null = self.new_temp(lir::LirType::I1);
        self.push(lir::Instruction::BinOp {
            out: non_null,
            op: lir::BinOp::Ne,
            lhs: task,
            rhs: null,
        });
        let present = self.new_block("context_present");
        let done = self.new_block("context_snapshot");
        self.seal(lir::Terminator::CondBr {
            cond: lir::Value::Temp(non_null),
            then_block: present,
            else_block: done,
        });
        self.enter(present);
        let storage = self.context_storage(mir::ContextStorageRole::Task);
        let (fields, _, _) = context_object_shape(self.context, storage);
        let value = self.load_at_offset(task, fields[0].offset, lir::MANAGED_PTR);
        self.push(lir::Instruction::Store {
            local: root,
            value: lir::Value::Temp(value),
        });
        self.seal(lir::Terminator::Br(done));
        self.enter(done);
        Ok(lir::Value::Local(root))
    }

    fn context_descriptor(&self, role: mir::ContextStorageRole) -> lir::Value {
        self.td_ref(&mir::Type::Context(self.context_storage(role)))
    }

    fn context_storage(&self, role: mir::ContextStorageRole) -> mir::ContextStorageType {
        self.module
            .meta
            .generated_exact_types
            .iter()
            .find_map(|entry| match entry.location() {
                mir::GeneratedExactTypeLocation::Context(storage) if storage.role == role => {
                    Some(storage)
                }
                _ => None,
            })
            .expect("the MIR input retains its core context support types")
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
