use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn emit_data_borrow(
        &mut self,
        instruction: &Instruction,
    ) -> Result<(), CodegenError> {
        match instruction {
            Instruction::PushPinFrame { out, object } => {
                self.check_borrow_types(*out, *object, &scoop_lir::RAW_PTR)?;
                let raw = ptr_ty(self.context);
                let managed = managed_ptr_ty(self.context, self.managed_address_space);
                let frame_ty = self
                    .context
                    .struct_type(&[raw.into(), managed.into()], false);
                let frame = self.entry_alloca(frame_ty.into(), "pin_frame")?;
                let push = self.native_boundary_fn(
                    scoop_lir::RuntimeAbiSymbolV1::PushPinFrame.logical_symbol(),
                    self.context
                        .void_type()
                        .fn_type(&[raw.into(), managed.into()], false),
                );
                self.builder
                    .build_call(push, &[frame.into(), self.value(*object)?.into()], "")
                    .map_err(|error| CodegenError(format!("push pin frame: {error}")))?;
                self.temps.insert(*out, frame.into());
            }
            Instruction::PopPinFrame { frame } => {
                if self.function.value_ty(self.globals_arena, *frame) != scoop_lir::RAW_PTR {
                    return Err(CodegenError("pin frame must be a raw stack pointer".into()));
                }
                let pop = self.native_boundary_fn(
                    scoop_lir::RuntimeAbiSymbolV1::PopPinFrame.logical_symbol(),
                    self.context
                        .void_type()
                        .fn_type(&[ptr_ty(self.context).into()], false),
                );
                self.builder
                    .build_call(pop, &[self.value(*frame)?.into()], "")
                    .map_err(|error| CodegenError(format!("pop pin frame: {error}")))?;
            }
            Instruction::ArrayDataPointer {
                out,
                object,
                array_type,
            } => {
                let (array, _) = self.array_type(*array_type);
                if array.layout.instance().object_scan().contains_reference() {
                    return Err(CodegenError(
                        "array data borrow requires GC-free elements".into(),
                    ));
                }
                self.borrow_data_pointer(*out, *object, array.layout.instance().inline_offset())?;
            }
            Instruction::StringDataPointer {
                out,
                object,
                byte_offset,
            } => {
                self.borrow_data_pointer(*out, *object, *byte_offset)?;
            }
            Instruction::BorrowDataLength {
                out,
                object,
                byte_offset,
            } => {
                self.check_borrow_types(*out, *object, &LirType::I64)?;
                let object = self.value(*object)?.into_pointer_value();
                let address = self.byte_gep(object, *byte_offset, "borrow_length_address")?;
                let value = self
                    .builder
                    .build_load(self.context.i64_type(), address, "borrow_length")
                    .map_err(|error| CodegenError(format!("borrow data length: {error}")))?;
                self.temps.insert(*out, value);
            }
            _ => unreachable!("data borrow dispatch is exhaustive"),
        }
        Ok(())
    }

    fn check_borrow_types(
        &self,
        out: TempId,
        object: Value,
        result: &LirType,
    ) -> Result<(), CodegenError> {
        if &self.function.temps[out].ty != result
            || self.function.value_ty(self.globals_arena, object) != scoop_lir::MANAGED_PTR
        {
            return Err(CodegenError(
                "data borrow requires a managed object and an exact result type".into(),
            ));
        }
        Ok(())
    }

    fn borrow_data_pointer(
        &mut self,
        out: TempId,
        object: Value,
        offset: u64,
    ) -> Result<(), CodegenError> {
        self.check_borrow_types(out, object, &scoop_lir::RAW_PTR)?;
        let object = self.value(object)?.into_pointer_value();
        // The non-inbounds GEP also admits the end address of an empty payload.
        let data = self.byte_gep(object, offset, "borrow_data")?;
        let raw = self
            .builder
            .build_address_space_cast(data, ptr_ty(self.context), "borrow_raw")
            .map_err(|error| CodegenError(format!("borrow data address: {error}")))?;
        if let Some(instruction) = raw.as_instruction_value() {
            mark_typed_managed_pointer_boundary(
                self.context,
                instruction,
                statepoint::TypedManagedPointerBoundary::ScopedDataBorrow,
            )?;
        }
        self.temps.insert(out, raw.into());
        Ok(())
    }
}
