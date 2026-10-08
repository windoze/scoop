use super::*;

impl FunctionLowerer<'_> {
    pub(super) fn lower_data_borrow_operation(
        &mut self,
        operation: &mir::DataBorrowOperation<mir::Expr>,
    ) -> StorageResult<lir::Value> {
        use mir::{BorrowDataSource as Source, DataBorrowOperationKind as Op};
        let object = self.lower_expr(&operation.operand)?;
        let out_ty = match operation.kind {
            Op::PopPinFrame => {
                self.push(lir::Instruction::PopPinFrame { frame: object });
                return Ok(self.unit_value());
            }
            Op::PushPinFrame | Op::DataPointer(_) => lir::RAW_PTR,
            Op::Length(_) => lir::LirType::I64,
        };
        let out = self.new_temp(out_ty);
        let instruction = match operation.kind {
            Op::PushPinFrame => lir::Instruction::PushPinFrame { out, object },
            Op::DataPointer(Source::Array(class)) => lir::Instruction::ArrayDataPointer {
                out,
                object,
                array_type: self.array_type_id(class),
            },
            Op::DataPointer(Source::String) => lir::Instruction::StringDataPointer {
                out,
                object,
                byte_offset: self.context.string_layout().size,
            },
            Op::Length(_) => lir::Instruction::BorrowDataLength {
                out,
                object,
                byte_offset: self.context.object_header_layout().size,
            },
            Op::PopPinFrame => unreachable!("pop has no result"),
        };
        self.push(instruction);
        Ok(lir::Value::Temp(out))
    }
}
