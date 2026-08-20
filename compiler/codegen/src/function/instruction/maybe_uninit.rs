use super::*;
use scoop_lir::{LocalId, MaybeUninitOperation as Operation, StructDefId};

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn emit_maybe_uninit(
        &mut self,
        out: LocalId,
        wrapper: StructDefId,
        operation: &Operation<Value>,
    ) -> Result<(), CodegenError> {
        let definition = &self.structs[wrapper];
        let StructRepresentation::Intrinsic(scoop_lir::IntrinsicTypeRepresentation::MaybeUninit {
            value: payload,
            ..
        }) = &definition.representation
        else {
            return Err(CodegenError(
                "MaybeUninit operation requires an intrinsic wrapper".into(),
            ));
        };
        let wrapper_ty = LirType::Struct(wrapper);
        let result_ty = match operation {
            Operation::AssumeInit(_) => payload.as_ref(),
            _ => &wrapper_ty,
        };
        if self.function.locals[out].ty() != result_ty {
            return Err(CodegenError(
                "MaybeUninit operation result type mismatch".into(),
            ));
        }
        if let Some(operand) = operation.operand() {
            let expected = match operation {
                Operation::Initialized(_) => payload.as_ref(),
                _ => &wrapper_ty,
            };
            if self.function.value_ty(self.globals_arena, *operand) != *expected {
                return Err(CodegenError(
                    "MaybeUninit operation operand type mismatch".into(),
                ));
            }
        }
        if definition.size == 0 {
            return Ok(());
        }
        let storage_ty = basic_ty(
            self.context,
            self.structs,
            self.enums,
            self.managed_address_space,
            &wrapper_ty,
        )?;
        let storage = self.local_pointer(out)?;
        match operation {
            Operation::Initialized(value) => {
                self.builder
                    .build_store(storage, storage_ty.const_zero())
                    .map_err(|e| CodegenError(format!("zero MaybeUninit storage: {e}")))?;
                if let Value::Local(local) = value {
                    self.builder
                        .build_memcpy(
                            storage,
                            definition.align as u32,
                            self.local_pointer(*local)?,
                            definition.align as u32,
                            self.context.i64_type().const_int(definition.size, false),
                        )
                        .map_err(|e| CodegenError(format!("copy MaybeUninit payload: {e}")))?;
                } else {
                    self.store_coercion_source(payload, self.value(*value)?, storage)?;
                }
            }
            Operation::AssumeInit(value) => {
                self.builder
                    .build_store(storage, self.value(*value)?)
                    .map_err(|e| CodegenError(format!("copy MaybeUninit wrapper: {e}")))?;
            }
            Operation::Uninit => {
                self.builder
                    .build_store(storage, storage_ty.const_zero())
                    .map_err(|e| CodegenError(format!("zero MaybeUninit storage: {e}")))?;
            }
        }
        Ok(())
    }
}
