use super::*;

impl FunctionLowerer<'_> {
    pub(super) fn lower_atomic_new(
        &mut self,
        ty: &mir::Type,
        initial: &mir::Expr,
    ) -> StorageResult<lir::Value> {
        let mir::Type::Class(class) = ty else {
            unreachable!("an atomic allocation has its actual class type")
        };
        let mir::ClassRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::Atomic(storage)) =
            &self.module.classes[*class].representation
        else {
            unreachable!("an atomic allocation names an intrinsic atomic class")
        };
        let kind = storage.kind();
        let layout = lir::atomic_object_layout(self.context.target_profile(), kind)?;
        let value = self.lower_expr(initial)?;
        let object = self.emit_plain_call(
            LoweredCallDestination::managed_runtime(lir::ManagedRuntimeFunction::Alloc),
            vec![
                lir::METADATA_PTR,
                lir::LirType::MachineScalar(lir::MachineScalarKind::ByteSize),
            ],
            lir::MANAGED_PTR,
            vec![
                self.td_ref(ty),
                lir::Value::MachineScalar(lir::MachineScalarValue::ByteSize(layout.size)),
            ],
        )?;
        self.push(lir::Instruction::AtomicStore {
            location: lir::AtomicLocation {
                object,
                offset: layout.value.offset,
                kind,
            },
            value,
            order: lir::AtomicStoreOrder::Relaxed,
        });
        Ok(object)
    }
}
