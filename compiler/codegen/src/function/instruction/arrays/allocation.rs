use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn emit_array_allocation(
        &mut self,
        instruction: &Instruction,
    ) -> Result<(), CodegenError> {
        let context = self.context;
        let builder = self.builder;
        let function = self.function;
        match instruction {
            Instruction::ArrayAlloc {
                out,
                elements,
                array_type,
                safepoint,
                live,
            } => {
                let (array_metadata, td) = self.array_type(*array_type);
                let element_lir_ty = array_metadata.element.clone();
                let layout = array_metadata.layout.clone();
                if function.temps[*out].ty != scoop_lir::MANAGED_PTR
                    || validation::contains_machine_scalar(
                        self.structs,
                        self.enums,
                        &element_lir_ty,
                    )
                {
                    return Err(CodegenError(format!(
                        "array_alloc @{} has an invalid result or internal machine-scalar element type",
                        function.symbol()
                    )));
                }
                for (index, element_value) in elements.iter().enumerate() {
                    let actual = function.value_ty(self.globals_arena, *element_value);
                    if actual != element_lir_ty {
                        return Err(CodegenError(format!(
                            "array_alloc @{} element {} has type {}, expected {}",
                            function.symbol(),
                            index,
                            actual.dump(),
                            element_lir_ty.dump()
                        )));
                    }
                }
                let safepoint = self.safepoint_id(*safepoint);
                let live = self.materialize_statepoint_live(live, safepoint)?;
                // LIR provides the complete variable instance extent and storage branch.
                let td = td.as_pointer_value();
                let element_count = u64::try_from(elements.len()).map_err(|_| {
                    CodegenError(format!(
                        "array_alloc @{} element count does not fit the runtime size word",
                        function.symbol()
                    ))
                })?;
                let total = layout.allocation_size(element_count).ok_or_else(|| {
                    CodegenError(format!(
                        "array allocation @{} exceeds count or target storage limits",
                        function.symbol()
                    ))
                })?;
                let array = self.managed_alloc_value(
                    td,
                    context.i64_type().const_int(total, false),
                    safepoint,
                    live,
                )?;
                let size_ptr = self.byte_gep(array, 16, "size_ptr")?;
                builder
                    .build_store(size_ptr, context.i64_type().const_int(element_count, false))
                    .map_err(|e| {
                        CodegenError(format!(
                            "array size @{symbol}: {e}",
                            symbol = function.symbol()
                        ))
                    })?;
                for (index, element_value) in elements.iter().enumerate() {
                    if matches!(
                        layout.storage().kind(),
                        scoop_lir::ArrayElementStorageKindV1::ZeroSized { .. }
                    ) {
                        continue;
                    }
                    let element_ptr = self.element_ptr(
                        array,
                        &layout,
                        context.i64_type().const_int(index as u64, false),
                        "element_ptr",
                    )?;
                    builder
                        .build_store(element_ptr, self.value(*element_value)?)
                        .map_err(|e| {
                            CodegenError(format!(
                                "array element @{symbol}: {e}",
                                symbol = function.symbol()
                            ))
                        })?;
                }
                self.heap_array_barrier(
                    array,
                    &layout,
                    context.i64_type().const_int(total, false),
                )?;
                self.temps.insert(*out, array.into());
            }
            _ => unreachable!("array instruction dispatch is exhaustive"),
        }
        Ok(())
    }
}
