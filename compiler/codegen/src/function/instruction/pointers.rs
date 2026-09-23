use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn emit_raw_pointer_instruction(
        &mut self,
        instruction: &Instruction,
    ) -> Result<(), CodegenError> {
        let context = self.context;
        let builder = self.builder;
        let function = self.function;
        match instruction {
            Instruction::RawLoad {
                out,
                pointer,
                pointee,
            } => {
                let pointer_ty = function.value_ty(self.globals_arena, *pointer);
                let out_ty = &function.temps[*out].ty;
                if pointer_ty != scoop_lir::RAW_PTR
                    || validation::contains_machine_scalar(self.structs, self.enums, out_ty)
                {
                    return Err(CodegenError(format!(
                        "raw_load @{} requires a raw pointer and cannot produce {}, got pointer {}",
                        function.symbol(),
                        out_ty.dump(),
                        pointer_ty.dump()
                    )));
                }
                let pointer = self.value(*pointer)?.into_pointer_value();
                let ty = basic_ty(
                    context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    &function.temps[*out].ty,
                )?;
                let value = builder
                    .build_load(ty, pointer, "raw_load")
                    .map_err(|e| CodegenError(format!("raw load @{}: {e}", function.symbol())))?;
                value
                    .as_instruction_value()
                    .expect("a non-constant load is an instruction")
                    .set_alignment(pointee.layout().alignment().get() as u32)
                    .map_err(|e| {
                        CodegenError(format!("raw load alignment @{}: {e}", function.symbol()))
                    })?;
                self.temps.insert(*out, value);
            }
            Instruction::RawStore {
                pointer,
                value,
                pointee,
            } => {
                let pointer_ty = function.value_ty(self.globals_arena, *pointer);
                let value_ty = function.value_ty(self.globals_arena, *value);
                if pointer_ty != scoop_lir::RAW_PTR
                    || validation::contains_machine_scalar(self.structs, self.enums, &value_ty)
                {
                    return Err(CodegenError(format!(
                        "raw_store @{} requires a raw pointer and cannot consume {}, got pointer {}",
                        function.symbol(),
                        value_ty.dump(),
                        pointer_ty.dump()
                    )));
                }
                let pointer = self.value(*pointer)?.into_pointer_value();
                let store = builder
                    .build_store(pointer, self.value(*value)?)
                    .map_err(|e| CodegenError(format!("raw store @{}: {e}", function.symbol())))?;
                store
                    .set_alignment(pointee.layout().alignment().get() as u32)
                    .map_err(|e| {
                        CodegenError(format!("raw store alignment @{}: {e}", function.symbol()))
                    })?;
            }
            Instruction::PtrOffset {
                out,
                pointer,
                element_offset,
                element_size,
                subtract,
            } => {
                let pointer_ty = function.value_ty(self.globals_arena, *pointer);
                let out_ty = &function.temps[*out].ty;
                if pointer_ty != scoop_lir::RAW_PTR || *out_ty != scoop_lir::RAW_PTR {
                    return Err(CodegenError(format!(
                        "ptr_offset @{} requires ptr<raw> -> ptr<raw>, got {} -> {}",
                        function.symbol(),
                        pointer_ty.dump(),
                        out_ty.dump()
                    )));
                }
                let pointer = self.value(*pointer)?.into_pointer_value();
                let offset_ty = function.value_ty(self.globals_arena, *element_offset);
                let valid_offset = match offset_ty {
                    LirType::I64 => true,
                    LirType::MachineScalar(MachineScalarKind::PointerElementOffset) => !subtract,
                    _ => false,
                };
                if !valid_offset {
                    return Err(CodegenError(format!(
                        "ptr_offset @{} has invalid element offset type {}",
                        function.symbol(),
                        offset_ty.dump()
                    )));
                }
                let mut bytes = self.value(*element_offset)?.into_int_value();
                if element_size.get() != 1 {
                    bytes = builder
                        .build_int_mul(
                            bytes,
                            context.i64_type().const_int(element_size.get(), false),
                            "raw_offset_bytes",
                        )
                        .map_err(|e| {
                            CodegenError(format!(
                                "raw pointer offset scale @{}: {e}",
                                function.symbol()
                            ))
                        })?;
                }
                if *subtract {
                    bytes = builder
                        .build_int_neg(bytes, "raw_offset_subtract")
                        .map_err(|e| {
                            CodegenError(format!(
                                "raw pointer offset negate @{}: {e}",
                                function.symbol()
                            ))
                        })?;
                }
                // SAFETY: source semantics make raw pointer arithmetic unsafe;
                // validity of the resulting address remains the caller's duty.
                let result = unsafe {
                    builder.build_gep(context.i8_type(), pointer, &[bytes], "raw_offset")
                }
                .map_err(|e| CodegenError(format!("raw gep @{}: {e}", function.symbol())))?;
                self.temps.insert(*out, result.into());
            }
            _ => unreachable!("raw pointer dispatch is exhaustive"),
        }
        Ok(())
    }
}
