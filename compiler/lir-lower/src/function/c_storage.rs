use super::*;

impl FunctionLowerer<'_> {
    pub(super) fn c_storage_type(&self, ty: &mir::Type) -> lir::LirType {
        c_ffi_type(self.module, self.structs, self.enums, ty).storage_type()
    }

    fn has_uint64_c_projection(&self, ty: &mir::Type) -> bool {
        matches!(ty, mir::Type::Struct(id) if matches!(
            self.module.structs[*id].representation,
            mir::StructRepresentation::Declared {
                c_abi: mir::StructCAbi::UInt64Field { .. },
                ..
            }
        ))
    }

    pub(super) fn project_c_value(&mut self, ty: &mir::Type, value: lir::Value) -> lir::Value {
        if !self.has_uint64_c_projection(ty) {
            return value;
        }
        // MIR has validated that the projection names the sole ULong field.
        let out = self.new_temp(lir::LirType::I64);
        self.push(lir::Instruction::ExtractValue {
            out,
            aggregate: value,
            index: 0,
        });
        lir::Value::Temp(out)
    }

    pub(super) fn restore_c_value(&mut self, ty: &mir::Type, value: lir::Value) -> lir::Value {
        if self.has_uint64_c_projection(ty) {
            self.make_aggregate(ty, vec![value])
        } else {
            value
        }
    }
}
