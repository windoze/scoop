use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn emit_heap_instruction(
        &mut self,
        instruction: &Instruction,
    ) -> Result<(), CodegenError> {
        let context = self.context;
        let builder = self.builder;
        let function = self.function;
        match instruction {
            Instruction::HeapLoad {
                out,
                object,
                offset,
            } => {
                let object_ty = function.value_ty(self.globals_arena, *object);
                if !matches!(
                    object_ty,
                    LirType::Ptr(PointerKind::Managed | PointerKind::Metadata)
                ) || validation::contains_machine_scalar(
                    self.structs,
                    self.enums,
                    &function.temps[*out].ty,
                ) {
                    return Err(CodegenError(format!(
                        "heap_load @{} requires a managed/metadata object and cannot produce a machine scalar; got object {}",
                        function.symbol,
                        object_ty.dump()
                    )));
                }
                let name = format!("t{}", out.into_raw().into_u32());
                let object = self.value(*object)?.into_pointer_value();
                let field_ptr = self.byte_gep(object, *offset, "field_ptr")?;
                let field_ty = basic_ty(
                    context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    &function.temps[*out].ty,
                )?;
                let element = builder
                    .build_load(field_ty, field_ptr, &name)
                    .map_err(|e| {
                        CodegenError(format!(
                            "heap load @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                self.temps.insert(*out, element);
            }
            Instruction::MachineHeapLoad {
                out,
                kind,
                object,
                offset,
            } => {
                let expected = LirType::MachineScalar(*kind);
                if !kind.is_atomic_state()
                    || function.temps[*out].ty != expected
                    || function.value_ty(self.globals_arena, *object) != scoop_lir::MANAGED_PTR
                    || *offset < 16
                    || *offset % 8 != 0
                {
                    return Err(CodegenError(format!(
                        "machine_heap_load @{} must read one aligned managed field of its declared machine state {:?}",
                        function.symbol, kind
                    )));
                }
                let name = format!("t{}", out.into_raw().into_u32());
                let object = self.value(*object)?.into_pointer_value();
                let field_ptr = self.byte_gep(object, *offset, "machine_field_ptr")?;
                let element = builder
                    .build_load(context.i64_type(), field_ptr, &name)
                    .map_err(|e| {
                        CodegenError(format!(
                            "machine heap load @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                self.temps.insert(*out, element);
            }
            Instruction::AtomicLoad {
                out,
                kind,
                object,
                offset,
            } => {
                if *offset < 16 || *offset % 8 != 0 {
                    return Err(CodegenError(format!(
                        "atomic_load @{symbol}: offset {offset} is not an aligned object field",
                        symbol = function.symbol,
                        offset = *offset
                    )));
                }
                if !kind.is_atomic_state()
                    || function.temps[*out].ty != LirType::MachineScalar(*kind)
                    || function.value_ty(self.globals_arena, *object) != scoop_lir::MANAGED_PTR
                {
                    return Err(CodegenError(format!(
                        "atomic_load @{} must produce its declared 64-bit machine state {:?}",
                        function.symbol, kind
                    )));
                }
                let name = format!("t{}", out.into_raw().into_u32());
                let object = self.value(*object)?.into_pointer_value();
                let field_ptr = self.byte_gep(object, *offset, "atomic_field_ptr")?;
                let field_ty = basic_ty(
                    context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    &function.temps[*out].ty,
                )?;
                let element = builder
                    .build_load(field_ty, field_ptr, &name)
                    .map_err(|e| {
                        CodegenError(format!(
                            "atomic load @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                element
                    .as_instruction_value()
                    .expect("a load is an instruction")
                    .set_atomic_ordering(AtomicOrdering::Acquire)
                    .map_err(|e| {
                        CodegenError(format!(
                            "atomic load ordering @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                self.temps.insert(*out, element);
            }
            Instruction::HeapStore {
                object,
                offset,
                value,
            } => {
                // Object fields begin after the 16-byte header.
                if *offset < 16 {
                    return Err(CodegenError(format!(
                        "heap_store @{symbol}: offset {offset} is inside the object header",
                        symbol = function.symbol,
                        offset = *offset
                    )));
                }
                let object_ty = function.value_ty(self.globals_arena, *object);
                let value_ty = function.value_ty(self.globals_arena, *value);
                if object_ty != scoop_lir::MANAGED_PTR
                    || validation::contains_machine_scalar(self.structs, self.enums, &value_ty)
                {
                    return Err(CodegenError(format!(
                        "heap_store @{} requires a managed object and cannot consume {}, got object {}",
                        function.symbol,
                        value_ty.dump(),
                        object_ty.dump()
                    )));
                }
                let object = self.value(*object)?.into_pointer_value();
                let field_ptr = self.byte_gep(object, *offset, "field_ptr")?;
                builder
                    .build_store(field_ptr, self.value(*value)?)
                    .map_err(|e| {
                        CodegenError(format!(
                            "heap_store @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                // M9 write barrier: mark the stored-to address's card.
                self.card_mark(field_ptr)?;
            }
            Instruction::MachineHeapStore {
                kind,
                object,
                offset,
                value,
            } => {
                let expected = LirType::MachineScalar(*kind);
                if !kind.is_atomic_state()
                    || function.value_ty(self.globals_arena, *value) != expected
                    || function.value_ty(self.globals_arena, *object) != scoop_lir::MANAGED_PTR
                    || *offset < 16
                    || *offset % 8 != 0
                {
                    return Err(CodegenError(format!(
                        "machine_heap_store @{} must write one aligned managed field of its declared machine state {:?}",
                        function.symbol, kind
                    )));
                }
                let object = self.value(*object)?.into_pointer_value();
                let field_ptr = self.byte_gep(object, *offset, "machine_field_ptr")?;
                builder
                    .build_store(field_ptr, self.value(*value)?)
                    .map_err(|e| {
                        CodegenError(format!(
                            "machine heap store @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                self.card_mark(field_ptr)?;
            }
            Instruction::AtomicStore {
                kind,
                object,
                offset,
                value,
            } => {
                if *offset < 16 || *offset % 8 != 0 {
                    return Err(CodegenError(format!(
                        "atomic_store @{symbol}: offset {offset} is not an aligned object field",
                        symbol = function.symbol,
                        offset = *offset
                    )));
                }
                if !kind.is_atomic_state()
                    || function.value_ty(self.globals_arena, *value)
                        != LirType::MachineScalar(*kind)
                    || function.value_ty(self.globals_arena, *object) != scoop_lir::MANAGED_PTR
                {
                    return Err(CodegenError(format!(
                        "atomic_store @{} must consume its declared 64-bit machine state {:?}",
                        function.symbol, kind
                    )));
                }
                let object = self.value(*object)?.into_pointer_value();
                let field_ptr = self.byte_gep(object, *offset, "atomic_field_ptr")?;
                let store = builder
                    .build_store(field_ptr, self.value(*value)?)
                    .map_err(|e| {
                        CodegenError(format!(
                            "atomic store @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                store
                    .set_atomic_ordering(AtomicOrdering::Release)
                    .map_err(|e| {
                        CodegenError(format!(
                            "atomic store ordering @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
            }
            Instruction::AtomicCompareExchange {
                out,
                kind,
                object,
                offset,
                expected,
                replacement,
            } => {
                if *offset < 16 || *offset % 8 != 0 {
                    return Err(CodegenError(format!(
                        "atomic_cmpxchg @{symbol}: offset {offset} is not an aligned object field",
                        symbol = function.symbol,
                        offset = *offset
                    )));
                }
                let expected_ty = LirType::MachineScalar(*kind);
                if !kind.is_atomic_state()
                    || function.temps[*out].ty != expected_ty
                    || function.value_ty(self.globals_arena, *expected) != expected_ty
                    || function.value_ty(self.globals_arena, *replacement) != expected_ty
                    || function.value_ty(self.globals_arena, *object) != scoop_lir::MANAGED_PTR
                {
                    return Err(CodegenError(format!(
                        "atomic_cmpxchg @{} must use one declared 64-bit machine state {:?}",
                        function.symbol, kind
                    )));
                }
                let object = self.value(*object)?.into_pointer_value();
                let field_ptr = self.byte_gep(object, *offset, "atomic_field_ptr")?;
                let pair = builder
                    .build_cmpxchg(
                        field_ptr,
                        self.value(*expected)?.into_int_value(),
                        self.value(*replacement)?.into_int_value(),
                        AtomicOrdering::AcquireRelease,
                        AtomicOrdering::Acquire,
                    )
                    .map_err(|e| {
                        CodegenError(format!(
                            "atomic cmpxchg @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                let old = builder
                    .build_extract_value(pair, 0, &format!("t{}", out.into_raw().into_u32()))
                    .map_err(|e| {
                        CodegenError(format!(
                            "atomic cmpxchg result @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                self.temps.insert(*out, old);
            }
            _ => unreachable!("instruction dispatcher routes only heap and atomic instructions"),
        }
        Ok(())
    }
}
