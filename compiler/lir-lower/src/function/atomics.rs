use super::*;

impl FunctionLowerer<'_> {
    pub(super) fn lower_atomic_operation(
        &mut self,
        atomic: &mir::AtomicExpression<mir::Expr>,
    ) -> StorageResult<lir::Value> {
        let atomic = atomic.try_map(|operand| self.lower_expr(operand))?;
        let layout = lir::atomic_object_layout(self.context.target_profile(), atomic.kind)?;
        let location = lir::AtomicLocation {
            object: atomic.object,
            offset: layout.value.offset,
            kind: atomic.kind,
        };
        let out = match atomic.operation {
            mir::AtomicOperation::Load { order } => {
                let out = self.new_temp(location.value_type());
                self.push(lir::Instruction::AtomicLoad {
                    out,
                    location,
                    order,
                });
                out
            }
            mir::AtomicOperation::Store { value, order } => {
                self.push(lir::Instruction::AtomicStore {
                    location,
                    value,
                    order,
                });
                return Ok(self.unit_value());
            }
            mir::AtomicOperation::Rmw {
                value,
                operation,
                order,
            } => {
                let out = self.new_temp(location.value_type());
                self.push(lir::Instruction::AtomicRmw {
                    out,
                    location,
                    value,
                    operation,
                    order,
                });
                out
            }
            mir::AtomicOperation::CompareExchange {
                expected,
                value,
                order,
                result,
            } => {
                let ty = match result {
                    lir::AtomicCompareExchangeResult::Success => lir::LirType::I1,
                    lir::AtomicCompareExchangeResult::ObservedValue => location.value_type(),
                };
                let out = self.new_temp(ty);
                self.push(lir::Instruction::AtomicCmpXchg {
                    out,
                    location,
                    expected,
                    replacement: value,
                    order,
                    result,
                });
                out
            }
        };
        Ok(lir::Value::Temp(out))
    }

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
