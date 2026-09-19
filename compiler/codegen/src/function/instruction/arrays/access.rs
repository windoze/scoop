use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn emit_array_access(
        &mut self,
        instruction: &Instruction,
    ) -> Result<(), CodegenError> {
        let context = self.context;
        let builder = self.builder;
        let function = self.function;
        match instruction {
            Instruction::ArrayLen { out, operand, .. } => {
                if function.temps[*out].ty != LirType::I64
                    || function.value_ty(self.globals_arena, *operand) != scoop_lir::MANAGED_PTR
                {
                    return Err(CodegenError(format!(
                        "array_len @{} requires ptr<managed> -> i64",
                        function.symbol()
                    )));
                }
                let array = self.value(*operand)?.into_pointer_value();
                let size_ptr = self.byte_gep(array, 16, "size_ptr")?;
                let name = format!("t{}", out.into_raw().into_u32());
                let size = builder
                    .build_load(context.i64_type(), size_ptr, &name)
                    .map_err(|e| {
                        CodegenError(format!(
                            "array_len @{symbol}: {e}",
                            symbol = function.symbol()
                        ))
                    })?;
                self.temps.insert(*out, size);
            }
            Instruction::ArrayGet {
                out,
                array,
                index,
                array_type,
            } => {
                let (array_metadata, _) = self.array_type(*array_type);
                let layout = array_metadata.layout.clone();
                let actual_out = &function.temps[*out].ty;
                let array_ty = function.value_ty(self.globals_arena, *array);
                let index_ty = function.value_ty(self.globals_arena, *index);
                if actual_out != &array_metadata.element
                    || array_ty != scoop_lir::MANAGED_PTR
                    || index_ty != LirType::I64
                {
                    return Err(CodegenError(format!(
                        "array_get @{} requires ptr<managed>[i64] -> {}, got {}[{}] -> {}",
                        function.symbol(),
                        array_metadata.element.dump(),
                        array_ty.dump(),
                        index_ty.dump(),
                        actual_out.dump()
                    )));
                }
                let element_ty = basic_ty(
                    context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    &array_metadata.element,
                )?;
                let array = self.value(*array)?.into_pointer_value();
                let index = self.value(*index)?.into_int_value();
                self.bounds_check(array, index)?;
                if matches!(
                    layout.storage(),
                    scoop_lir::ArrayElementStorageV1::ZeroSized { .. }
                ) {
                    self.temps.insert(*out, element_ty.const_zero());
                    return Ok(());
                }
                let element_ptr = self.element_ptr(array, &layout, index, "element_ptr")?;
                let name = format!("t{}", out.into_raw().into_u32());
                let value = builder
                    .build_load(element_ty, element_ptr, &name)
                    .map_err(|e| {
                        CodegenError(format!(
                            "array_get @{symbol}: {e}",
                            symbol = function.symbol()
                        ))
                    })?;
                self.temps.insert(*out, value);
            }
            Instruction::ArraySet {
                array,
                index,
                value,
                array_type,
            } => {
                let (array_metadata, _) = self.array_type(*array_type);
                let layout = array_metadata.layout.clone();
                let array_ty = function.value_ty(self.globals_arena, *array);
                let index_ty = function.value_ty(self.globals_arena, *index);
                let value_ty = function.value_ty(self.globals_arena, *value);
                if array_ty != scoop_lir::MANAGED_PTR
                    || index_ty != LirType::I64
                    || value_ty != array_metadata.element
                {
                    return Err(CodegenError(format!(
                        "array_set @{} requires ptr<managed>[i64] = {}, got {}[{}] = {}",
                        function.symbol(),
                        array_metadata.element.dump(),
                        array_ty.dump(),
                        index_ty.dump(),
                        value_ty.dump()
                    )));
                }
                let array = self.value(*array)?.into_pointer_value();
                let index = self.value(*index)?.into_int_value();
                self.bounds_check(array, index)?;
                if matches!(
                    layout.storage(),
                    scoop_lir::ArrayElementStorageV1::ZeroSized { .. }
                ) {
                    return Ok(());
                }
                let element_ptr = self.element_ptr(array, &layout, index, "element_ptr")?;
                builder
                    .build_store(element_ptr, self.value(*value)?)
                    .map_err(|e| {
                        CodegenError(format!(
                            "array_set @{symbol}: {e}",
                            symbol = function.symbol()
                        ))
                    })?;
                // M9 write barrier: mark the stored-to address's card
                // (array element stores are heap stores too).
                self.card_mark(element_ptr)?;
            }
            _ => unreachable!("array instruction dispatch is exhaustive"),
        }
        Ok(())
    }
}
