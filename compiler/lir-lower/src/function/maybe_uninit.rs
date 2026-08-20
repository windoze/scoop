use super::*;

impl FunctionLowerer<'_> {
    pub(super) fn lower_maybe_uninit(
        &mut self,
        wrapper: mir::StructId,
        operation: &mir::MaybeUninitOperation<Box<mir::Expr>>,
        result: &mir::Type,
    ) -> StorageResult<lir::Value> {
        let operation = match operation {
            mir::MaybeUninitOperation::Uninit => lir::MaybeUninitOperation::Uninit,
            mir::MaybeUninitOperation::Initialized(value) => {
                lir::MaybeUninitOperation::Initialized(self.lower_expr(value)?)
            }
            mir::MaybeUninitOperation::AssumeInit(value) => {
                lir::MaybeUninitOperation::AssumeInit(self.lower_expr(value)?)
            }
        };
        let ty = self.value_type(result);
        let storage = match abi::classify_storage(self.context, ty, self.structs, self.enums)? {
            abi::ValueStorage::NonZero(value) => lir::LocalStorage::NonZero(value),
            abi::ValueStorage::ZeroSized(representation) => {
                lir::LocalStorage::LogicalZst(lir::LogicalZstValue::new(
                    exact_type_record(self.module, result).id(),
                    representation,
                ))
            }
        };
        self.hidden_count += 1;
        let out = self.locals.alloc(lir::Local::new(
            format!("$maybe.{}", self.hidden_count),
            storage,
        ));
        self.push(lir::Instruction::MaybeUninit {
            out,
            wrapper: struct_def_id(wrapper),
            operation,
        });
        Ok(lir::Value::Local(out))
    }
}
