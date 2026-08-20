use super::*;

impl FunctionLowerer<'_> {
    pub(super) fn lower_pointer_offset(
        &mut self,
        offset: &mir::Expr,
        subtract: bool,
    ) -> StorageResult<lir::Value> {
        let valid = match offset.ty {
            mir::Type::Integer(mir::IntegerKind::SIGNED_64) => true,
            mir::Type::MachineScalar(mir::MachineScalarKind::PointerElementOffset) => !subtract,
            _ => false,
        };
        assert!(
            valid,
            "pointer offset requires Long or an additive PointerElementOffset"
        );
        self.lower_expr(offset)
    }

    pub(super) fn pointer_storage(
        &mut self,
        pointee: &mir::Type,
    ) -> StorageResult<abi::ValueStorage> {
        let storage = self.value_type(pointee);
        abi::classify_storage(self.context, storage, self.structs, self.enums)
    }

    pub(in crate::function) fn logical_zst_value(
        &mut self,
        ty: &mir::Type,
        representation: lir::AbiZst,
    ) -> lir::Value {
        let out = self.new_temp(representation.storage_type().clone());
        let value =
            lir::LogicalZstValue::new(exact_type_record(self.module, ty).id(), representation);
        self.push(lir::Instruction::MakeZstValue { out, value });
        lir::Value::Temp(out)
    }

    pub(in crate::function) fn offset_pointer(
        &mut self,
        pointer: lir::Value,
        pointee: &mir::Type,
        offset: lir::Value,
        subtract: bool,
    ) -> StorageResult<lir::Value> {
        match self.pointer_storage(pointee)? {
            abi::ValueStorage::ZeroSized(_) => Ok(pointer),
            abi::ValueStorage::NonZero(pointee) => {
                let out = self.new_temp(lir::RAW_PTR);
                self.push(lir::Instruction::PtrOffset {
                    out,
                    pointer,
                    element_offset: offset,
                    element_size: pointee.layout().size(),
                    subtract,
                });
                Ok(lir::Value::Temp(out))
            }
        }
    }
}
