use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn incoming_direct_parts(
        &self,
        parts: &scoop_lir::AbiDirectParts,
        first: usize,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        let ty = basic_ty(
            self.context,
            self.structs,
            self.enums,
            self.managed_address_space,
            parts.value().storage_type(),
        )?
        .into_struct_type();
        let mut value = ty.const_zero();
        for (index, _) in parts.parts().iter().enumerate() {
            let parameter = u32::try_from(first + index)
                .ok()
                .and_then(|index| self.llvm_function.get_nth_param(index))
                .ok_or_else(|| CodegenError("direct part parameter is out of range".into()))?;
            value = self
                .builder
                .build_insert_value(value, parameter, index as u32, "parameter_part")
                .map_err(|e| CodegenError(format!("assemble direct parameter: {e}")))?
                .into_struct_value();
        }
        Ok(value.into())
    }

    pub(super) fn is_interface_storage(&self, ty: &LirType) -> bool {
        match ty {
            LirType::Interface => true,
            LirType::Enum(id) => matches!(
                self.enums[*id].repr,
                EnumRepr::Niche {
                    kind: scoop_lir::NullNicheKind::Interface,
                    ..
                }
            ),
            _ => false,
        }
    }

    /// Reconstruct a relocated interface from its canonical storage. A
    /// volatile object load prevents aggregate forwarding from resurrecting
    /// a pre-statepoint SSA value; immutable metadata needs no relocation.
    pub(super) fn load_stored_value(
        &self,
        ty: &LirType,
        storage: PointerValue<'ctx>,
        name: &str,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        if self.is_interface_storage(ty) {
            let object = self
                .builder
                .build_load(
                    managed_ptr_ty(self.context, self.managed_address_space),
                    storage,
                    "interface_object",
                )
                .map_err(|e| CodegenError(format!("load interface object: {e}")))?;
            object
                .as_instruction_value()
                .expect("a load is an instruction")
                .set_volatile(true)
                .map_err(|e| CodegenError(format!("volatile interface object: {e}")))?;
            return self.interface_with_object(storage, object);
        }
        let ty = basic_ty(
            self.context,
            self.structs,
            self.enums,
            self.managed_address_space,
            ty,
        )?;
        self.builder
            .build_load(ty, storage, name)
            .map_err(|e| CodegenError(format!("load {name}: {e}")))
    }

    pub(super) fn interface_with_object(
        &self,
        storage: PointerValue<'ctx>,
        object: BasicValueEnum<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        let ty = basic_ty(
            self.context,
            self.structs,
            self.enums,
            self.managed_address_space,
            &LirType::Interface,
        )?
        .into_struct_type();
        let table_address = self
            .builder
            .build_struct_gep(ty, storage, 1, "interface_table_address")
            .map_err(|e| CodegenError(format!("address interface table: {e}")))?;
        let table = self
            .builder
            .build_load(ptr_ty(self.context), table_address, "interface_table")
            .map_err(|e| CodegenError(format!("load interface table: {e}")))?;
        let value = self
            .builder
            .build_insert_value(ty.const_zero(), object, 0, "interface_object_part")
            .map_err(|e| CodegenError(format!("assemble interface object: {e}")))?;
        self.builder
            .build_insert_value(value, table, 1, "interface_value")
            .map(|value| value.into_struct_value().into())
            .map_err(|e| CodegenError(format!("assemble interface table: {e}")))
    }
}
