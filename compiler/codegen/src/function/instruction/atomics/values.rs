use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn atomic_address(
        &self,
        location: AtomicLocation,
    ) -> Result<PointerValue<'ctx>, CodegenError> {
        if self.function.value_ty(self.globals_arena, location.object) != scoop_lir::MANAGED_PTR
            || location.offset < 16
            || location.offset % u64::from(location.kind.bytes()) != 0
        {
            return Err(CodegenError(
                "atomic access requires an aligned managed object field".into(),
            ));
        }
        self.byte_gep(
            self.value(location.object)?.into_pointer_value(),
            location.offset,
            "atomic_field",
        )
    }

    pub(super) fn atomic_storage_type(
        &self,
        location: AtomicLocation,
    ) -> Result<BasicTypeEnum<'ctx>, CodegenError> {
        if location.kind == AtomicValueKind::Boolean {
            return Ok(self.context.i8_type().into());
        }
        basic_ty(
            self.context,
            self.structs,
            self.enums,
            self.managed_address_space,
            &location.value_type(),
        )
    }

    pub(super) fn require_atomic_result(
        &self,
        out: TempId,
        expected: LirType,
    ) -> Result<(), CodegenError> {
        if self.function.temps[out].ty != expected {
            return Err(CodegenError(format!(
                "atomic result must have type {}",
                expected.dump()
            )));
        }
        Ok(())
    }

    pub(super) fn atomic_operand(
        &self,
        location: AtomicLocation,
        value: Value,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        if self.function.value_ty(self.globals_arena, value) != location.value_type() {
            return Err(CodegenError(format!(
                "atomic operand must have type {}",
                location.value_type().dump()
            )));
        }
        let value = self.value(value)?;
        if location.kind == AtomicValueKind::Boolean {
            return self
                .builder
                .build_int_z_extend(
                    value.into_int_value(),
                    self.context.i8_type(),
                    "atomic_bool_byte",
                )
                .map(Into::into)
                .map_err(|error| CodegenError(format!("atomic Boolean storage: {error}")));
        }
        Ok(value)
    }

    pub(super) fn atomic_result(
        &self,
        location: AtomicLocation,
        value: BasicValueEnum<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        if location.kind == AtomicValueKind::Boolean {
            return self
                .builder
                .build_int_truncate(
                    value.into_int_value(),
                    self.context.bool_type(),
                    "atomic_bool",
                )
                .map(Into::into)
                .map_err(|error| CodegenError(format!("atomic Boolean result: {error}")));
        }
        Ok(value)
    }
}
