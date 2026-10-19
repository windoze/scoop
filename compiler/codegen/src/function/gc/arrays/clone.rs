use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(in crate::function) fn emit_array_clone(
        &mut self,
        instruction: &Instruction,
    ) -> Result<(), CodegenError> {
        let context = self.context;
        let builder = self.builder;
        let function = self.function;
        match instruction {
            Instruction::ArrayClone {
                source_type,
                out,
                operand,
                array_type,
                safepoint,
                live,
            } => {
                let (array_metadata, target_td) = self.array_type(*array_type);
                let element = array_metadata.element.clone();
                let layout = array_metadata.layout.clone();
                let (source_metadata, _) = self.array_type(*source_type);
                if source_metadata.element_exact != array_metadata.element_exact
                    || source_metadata.element != array_metadata.element
                    || source_metadata.layout != array_metadata.layout
                {
                    return Err(CodegenError(
                        "array clone source and target element storage disagree".to_string(),
                    ));
                }
                let operand_ty = function.value_ty(self.globals_arena, *operand);
                if function.temps[*out].ty != scoop_lir::MANAGED_PTR
                    || operand_ty != scoop_lir::MANAGED_PTR
                    || machine_types::contains_machine_scalar(self.structs, self.enums, &element)
                {
                    return Err(CodegenError(format!(
                        "array_clone @{} requires a machine-scalar-free managed array -> managed array",
                        function.symbol()
                    )));
                }
                // A valid source of the same layout already bounds the count and extent.
                let source = self.value(*operand)?.into_pointer_value();
                let count_ptr = self.byte_gep(source, 16, "clone.source_count_ptr")?;
                let count = builder
                    .build_load(context.i64_type(), count_ptr, "clone.count")
                    .map_err(clone_error)?
                    .into_int_value();
                let bytes = self.array_allocation_size(&layout, count)?;
                let safepoint = self.safepoint_id(*safepoint);
                let live = self.materialize_statepoint_live(live, safepoint)?;
                let copy =
                    self.managed_alloc_value(target_td.as_pointer_value(), bytes, safepoint, live)?;
                let count_ptr = self.byte_gep(copy, 16, "clone.count_ptr")?;
                builder.build_store(count_ptr, count).map_err(clone_error)?;
                if let scoop_lir::ArrayElementStorageKindV1::Inline { stride, .. } =
                    layout.storage().kind()
                {
                    // Allocation can move the source. Form both payload addresses only
                    // after restoring its canonical root, on either allocation path.
                    let source = self.value(*operand)?.into_pointer_value();
                    let offset = layout.instance().inline_offset();
                    let source = self.byte_gep(source, offset, "clone.source")?;
                    let destination = self.byte_gep(copy, offset, "clone.destination")?;
                    let payload_bytes = builder
                        .build_int_mul(
                            count,
                            context.i64_type().const_int(stride.get(), false),
                            "clone.payload_bytes",
                        )
                        .map_err(clone_error)?;
                    let alignment = layout.instance().inline_alignment() as u32;
                    builder
                        .build_memcpy(destination, alignment, source, alignment, payload_bytes)
                        .map_err(clone_error)?;
                    self.heap_array_barrier(copy, &layout, bytes)?;
                }
                self.temps.insert(*out, copy.into());
            }
            _ => unreachable!("array instruction dispatch is exhaustive"),
        }
        Ok(())
    }
}

fn clone_error(error: inkwell::builder::BuilderError) -> CodegenError {
    CodegenError(format!("array clone emission: {error}"))
}
