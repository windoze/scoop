use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(in crate::function) fn heap_value_barrier(
        &self,
        address: PointerValue<'ctx>,
        ty: &LirType,
    ) -> Result<(), CodegenError> {
        let mut offsets = Vec::new();
        self.heap_reference_offsets(ty, 0, &mut offsets)?;
        offsets.sort_unstable();
        let Some(&first) = offsets.first() else {
            return Ok(());
        };
        let address = self.byte_gep(address, first, "barrier_start")?;
        if offsets.len() == 1 {
            return self.card_mark(address);
        }
        let bytes = offsets[offsets.len() - 1] - first + 8;
        self.heap_range_barrier(address, self.context.i64_type().const_int(bytes, false))
    }

    pub(in crate::function) fn heap_array_barrier(
        &self,
        array: PointerValue<'ctx>,
        layout: &scoop_lir::ArrayLayoutV1,
        total_bytes: IntValue<'ctx>,
    ) -> Result<(), CodegenError> {
        if !layout.instance().inline_scan().contains_reference() {
            return Ok(());
        }
        let offset = layout.instance().inline_offset();
        let first = self.byte_gep(array, offset, "barrier_elements")?;
        let bytes = self
            .builder
            .build_int_sub(
                total_bytes,
                self.context.i64_type().const_int(offset, false),
                "barrier_bytes",
            )
            .map_err(|error| CodegenError(format!("array barrier extent: {error}")))?;
        self.heap_range_barrier(first, bytes)
    }

    fn heap_range_barrier(
        &self,
        address: PointerValue<'ctx>,
        bytes: IntValue<'ctx>,
    ) -> Result<(), CodegenError> {
        if bytes.get_zero_extended_constant() == Some(0) {
            return Ok(());
        }
        let barrier = self.gc_leaf_fn(
            scoop_lir::RuntimeAbiSymbolV1::WriteBarrier.logical_symbol(),
            self.context.void_type().fn_type(
                &[
                    managed_ptr_ty(self.context, self.managed_address_space).into(),
                    self.context.i64_type().into(),
                ],
                false,
            ),
        );
        barrier.add_attribute(
            AttributeLoc::Function,
            self.context
                .create_enum_attribute(Attribute::get_named_enum_kind_id("nounwind"), 0),
        );
        self.builder
            .build_call(barrier, &[address.into(), bytes.into()], "")
            .map_err(|error| CodegenError(format!("heap range barrier: {error}")))?;
        Ok(())
    }

    /// Read offsets from the complete value layout. This does not revalidate
    /// the type or make any assumption about the age of the destination.
    fn heap_reference_offsets(
        &self,
        ty: &LirType,
        base: u64,
        offsets: &mut Vec<u64>,
    ) -> Result<(), CodegenError> {
        match ty {
            LirType::Ptr(PointerKind::Managed) | LirType::Interface => offsets.push(base),
            LirType::Struct(id) => {
                if let StructRepresentation::Scoop { fields } = &self.structs[*id].representation {
                    for field in fields {
                        self.heap_reference_offsets(
                            &field.ty,
                            base + field.layout.offset,
                            offsets,
                        )?;
                    }
                } else if let StructRepresentation::Intrinsic(
                    scoop_lir::IntrinsicTypeRepresentation::MaybeUninit { scan, .. },
                ) = &self.structs[*id].representation
                {
                    let start = offsets.len();
                    flatten_ref_scan(scan, offsets);
                    for offset in &mut offsets[start..] {
                        *offset += base;
                    }
                }
            }
            LirType::Enum(id) => {
                let start = offsets.len();
                flatten_ref_scan(&self.enums[*id].scan, offsets);
                for offset in &mut offsets[start..] {
                    *offset += base;
                }
            }
            LirType::Aggregate(fields) => {
                let layout = basic_ty(
                    self.context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    ty,
                )?
                .into_struct_type();
                for (index, field) in fields.iter().enumerate() {
                    let offset = self
                        .target_data
                        .offset_of_element(&layout, index as u32)
                        .expect("the field belongs to this concrete aggregate");
                    self.heap_reference_offsets(field, base + offset, offsets)?;
                }
            }
            LirType::Void
            | LirType::I1
            | LirType::I8
            | LirType::I16
            | LirType::I32
            | LirType::I64
            | LirType::F32
            | LirType::F64
            | LirType::MachineScalar(_)
            | LirType::Ptr(PointerKind::Raw | PointerKind::Code | PointerKind::Metadata)
            | LirType::ExceptionRecord => {}
        }
        Ok(())
    }
}
