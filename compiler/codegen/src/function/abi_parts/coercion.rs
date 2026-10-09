use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(in crate::function) fn outgoing_parts(
        &self,
        parts: &scoop_lir::AbiDirectParts,
        value: BasicValueEnum<'ctx>,
    ) -> Result<Vec<BasicValueEnum<'ctx>>, CodegenError> {
        if self.is_interface_storage(parts.value().storage_type()) {
            return (0..2)
                .map(|i| {
                    self.builder
                        .build_extract_value(value.into_struct_value(), i, "interface_part")
                        .map_err(|e| CodegenError(format!("extract interface part: {e}")))
                })
                .collect();
        }
        let ty = basic_ty(
            self.context,
            self.structs,
            self.enums,
            self.managed_address_space,
            parts.value().storage_type(),
        )?;
        let storage = self.entry_alloca(ty, "coercion_source")?;
        self.zero_coercion_storage(storage, parts.value().layout().size().get())?;
        self.store_coercion_source(parts.value().storage_type(), value, storage)?;
        parts
            .parts()
            .iter()
            .map(|part| {
                let carrier =
                    abi::carrier_type(self.context, self.managed_address_space, part.carrier());
                let temporary = self.entry_alloca(carrier, "coercion_carrier")?;
                self.zero_coercion_storage(temporary, part.carrier().byte_size())?;
                let source = self.coercion_offset(storage, part.byte_offset())?;
                self.copy_coercion_bytes(temporary, 1, source, part.alignment(), part.extent())?;
                self.builder
                    .build_load(carrier, temporary, "argument_part")
                    .map_err(|e| CodegenError(format!("read ABI carrier: {e}")))
            })
            .collect()
    }

    pub(in crate::function) fn value_from_parts(
        &self,
        parts: &scoop_lir::AbiDirectParts,
        values: &[BasicValueEnum<'ctx>],
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        let ty = basic_ty(
            self.context,
            self.structs,
            self.enums,
            self.managed_address_space,
            parts.value().storage_type(),
        )?;
        if self.is_interface_storage(parts.value().storage_type()) {
            let mut value = ty.into_struct_type().const_zero();
            for (i, part) in values.iter().enumerate() {
                value = self
                    .builder
                    .build_insert_value(value, *part, i as u32, "interface_value")
                    .map_err(|e| CodegenError(format!("assemble interface value: {e}")))?
                    .into_struct_value();
            }
            return Ok(value.into());
        }
        let storage = self.entry_alloca(ty, "coercion_value")?;
        self.zero_coercion_storage(storage, parts.value().layout().size().get())?;
        for (part, value) in parts.parts().iter().zip(values) {
            let temporary = self.entry_alloca(value.get_type(), "coercion_return_part")?;
            self.builder
                .build_store(temporary, *value)
                .map_err(|e| CodegenError(format!("store ABI carrier: {e}")))?;
            let destination = self.coercion_offset(storage, part.byte_offset())?;
            self.copy_coercion_bytes(destination, part.alignment(), temporary, 1, part.extent())?;
        }
        self.builder
            .build_load(ty, storage, "coercion_value")
            .map_err(|e| CodegenError(format!("reconstruct ABI value: {e}")))
    }

    pub(in crate::function) fn encode_direct_result(
        &self,
        plan: &scoop_lir::AbiDirectValue,
        value: BasicValueEnum<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        let scoop_lir::AbiDirectValue::DirectParts(parts) = plan else {
            return Ok(value);
        };
        let values = self.outgoing_parts(parts, value)?;
        if values.len() == 1 {
            return Ok(values[0]);
        }
        let ty = abi::direct_type(
            self.context,
            self.structs,
            self.enums,
            self.managed_address_space,
            plan,
        )?;
        let mut result = ty.into_struct_type().const_zero();
        for (i, value) in values.iter().enumerate() {
            result = self
                .builder
                .build_insert_value(result, *value, i as u32, "return_part")
                .map_err(|e| CodegenError(format!("assemble return carrier: {e}")))?
                .into_struct_value();
        }
        Ok(result.into())
    }

    pub(in crate::function) fn decode_direct_result(
        &self,
        plan: &scoop_lir::AbiDirectValue,
        value: BasicValueEnum<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        let scoop_lir::AbiDirectValue::DirectParts(parts) = plan else {
            return Ok(value);
        };
        if parts.parts().len() == 1 {
            return self.value_from_parts(parts, &[value]);
        }
        let values = (0..parts.parts().len())
            .map(|i| {
                self.builder
                    .build_extract_value(value.into_struct_value(), i as u32, "return_part")
                    .map_err(|e| CodegenError(format!("extract return carrier: {e}")))
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.value_from_parts(parts, &values)
    }

    fn zero_coercion_storage(
        &self,
        pointer: PointerValue<'ctx>,
        size: u64,
    ) -> Result<(), CodegenError> {
        self.builder
            .build_memset(
                pointer,
                1,
                self.context.i8_type().const_zero(),
                self.context.i64_type().const_int(size, false),
            )
            .map_err(|e| CodegenError(format!("zero ABI coercion storage: {e}")))?;
        Ok(())
    }

    fn copy_coercion_bytes(
        &self,
        destination: PointerValue<'ctx>,
        dst_align: u64,
        source: PointerValue<'ctx>,
        src_align: u64,
        size: u64,
    ) -> Result<(), CodegenError> {
        self.builder
            .build_memcpy(
                destination,
                dst_align as u32,
                source,
                src_align as u32,
                self.context.i64_type().const_int(size, false),
            )
            .map_err(|e| CodegenError(format!("copy ABI coercion bytes: {e}")))?;
        Ok(())
    }

    fn coercion_offset(
        &self,
        pointer: PointerValue<'ctx>,
        offset: u64,
    ) -> Result<PointerValue<'ctx>, CodegenError> {
        // SAFETY: ranges were checked against the exact ABI storage layout.
        unsafe {
            self.builder.build_gep(
                self.context.i8_type(),
                pointer,
                &[self.context.i64_type().const_int(offset, false)],
                "coercion_address",
            )
        }
        .map_err(|e| CodegenError(format!("address ABI coercion bytes: {e}")))
    }

    /// Store fields separately so source-level padding stays zero instead of
    /// being overwritten by undef aggregate padding. Tagged payload already
    /// has its complete, zero-initialized byte representation.
    pub(in crate::function) fn store_coercion_source(
        &self,
        ty: &LirType,
        value: BasicValueEnum<'ctx>,
        pointer: PointerValue<'ctx>,
    ) -> Result<(), CodegenError> {
        let fields = match ty {
            LirType::Struct(id)
                if !matches!(
                    self.structs[*id].representation,
                    StructRepresentation::Intrinsic(
                        scoop_lir::IntrinsicTypeRepresentation::MaybeUninit { .. }
                    )
                ) =>
            {
                let definition = &self.structs[*id];
                let mut aggregate = value.into_struct_value();
                if definition.is_c_layout() {
                    aggregate = self
                        .builder
                        .build_extract_value(aggregate, 1, "coercion_payload")
                        .map_err(|e| CodegenError(format!("extract C payload: {e}")))?
                        .into_struct_value();
                }
                (0..definition.field_count())
                    .map(|i| {
                        let index = if definition.is_c_layout() {
                            c_physical_field_index(self.structs, self.enums, definition, i as u32)?
                        } else {
                            i as u32
                        };
                        Ok((
                            definition.field_storage_type(i).expect("field has a type"),
                            definition
                                .field_layout(i)
                                .expect("field has a layout")
                                .offset,
                            self.builder
                                .build_extract_value(aggregate, index, "coercion_field")
                                .map_err(|e| {
                                    CodegenError(format!("extract ABI source field: {e}"))
                                })?,
                        ))
                    })
                    .collect::<Result<Vec<_>, CodegenError>>()?
            }
            LirType::Aggregate(types) => {
                let aggregate = value.into_struct_value();
                types
                    .iter()
                    .enumerate()
                    .map(|(i, ty)| {
                        Ok((
                            ty.clone(),
                            self.target_data
                                .offset_of_element(&aggregate.get_type(), i as u32)
                                .expect("tuple field has an offset"),
                            self.builder
                                .build_extract_value(aggregate, i as u32, "coercion_field")
                                .map_err(|e| CodegenError(format!("extract tuple field: {e}")))?,
                        ))
                    })
                    .collect::<Result<Vec<_>, CodegenError>>()?
            }
            _ => {
                self.builder
                    .build_store(pointer, value)
                    .map_err(|e| CodegenError(format!("store ABI source scalar: {e}")))?
                    .set_alignment(1)
                    .map_err(|e| CodegenError(format!("align ABI source: {e}")))?;
                return Ok(());
            }
        };
        for (ty, offset, value) in fields {
            self.store_coercion_source(&ty, value, self.coercion_offset(pointer, offset)?)?;
        }
        Ok(())
    }
}
