use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn emit_array_assembly(
        &mut self,
        instruction: &Instruction,
    ) -> Result<(), CodegenError> {
        let context = self.context;
        let builder = self.builder;
        let function = self.function;
        match instruction {
            Instruction::ArrayAssembly {
                out,
                parts,
                overflow_message,
                array_type,
                safepoint,
                live,
            } => {
                let (array_metadata, td) = self.array_type(*array_type);
                if function.temps[*out].ty != scoop_lir::MANAGED_PTR
                    || validation::contains_machine_scalar(
                        self.structs,
                        self.enums,
                        &array_metadata.element,
                    )
                {
                    return Err(CodegenError(format!(
                        "array_assembly @{} has an invalid result or internal machine-scalar element type",
                        function.symbol()
                    )));
                }
                for (index, part) in parts.iter().enumerate() {
                    match part {
                        scoop_lir::ArrayAssemblyPart::Element(value) => {
                            let actual = function.value_ty(self.globals_arena, *value);
                            if actual != array_metadata.element {
                                return Err(CodegenError(format!(
                                    "array_assembly @{} element part {} has type {}, expected {}",
                                    function.symbol(),
                                    index,
                                    actual.dump(),
                                    array_metadata.element.dump()
                                )));
                            }
                        }
                        scoop_lir::ArrayAssemblyPart::CopyArray(value)
                            if function.value_ty(self.globals_arena, *value)
                                != scoop_lir::MANAGED_PTR =>
                        {
                            return Err(CodegenError(format!(
                                "array_assembly @{} copy part {} is not a managed array",
                                function.symbol(),
                                index
                            )));
                        }
                        scoop_lir::ArrayAssemblyPart::CopyArray(_) => {}
                    }
                }
                let element_layout = array_metadata.element.clone();
                let layout = array_metadata.layout.clone();
                let element_ty = basic_ty(
                    context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    &element_layout,
                )?;
                let i64_ty = context.i64_type();
                let element_count = u64::try_from(
                    parts
                        .iter()
                        .filter(|part| matches!(part, scoop_lir::ArrayAssemblyPart::Element(_)))
                        .count(),
                )
                .map_err(|_| CodegenError("array assembly element count exceeds u64".into()))?;
                if element_count > i64::MAX as u64 {
                    return Err(CodegenError(
                        "array assembly element count exceeds INT64_MAX".into(),
                    ));
                }
                let mut total = i64_ty.const_int(element_count, false);
                for (part_index, part) in parts.iter().enumerate() {
                    let scoop_lir::ArrayAssemblyPart::CopyArray(source) = part else {
                        continue;
                    };
                    let source = self.value(*source)?.into_pointer_value();
                    let size_ptr = self.byte_gep(source, 16, "assembly_source_size_ptr")?;
                    let size = builder
                        .build_load(i64_ty, size_ptr, "assembly_source_size")
                        .map_err(|error| {
                            CodegenError(format!(
                                "load array assembly source size @{symbol}: {error}",
                                symbol = function.symbol()
                            ))
                        })?
                        .into_int_value();
                    let next_total = builder
                        .build_int_add(total, size, "assembly_total_size")
                        .map_err(|error| {
                            CodegenError(format!(
                                "sum array assembly size @{symbol}: {error}",
                                symbol = function.symbol()
                            ))
                        })?;
                    let overflow = builder
                        .build_int_compare(
                            IntPredicate::ULT,
                            next_total,
                            total,
                            "assembly_total_overflow",
                        )
                        .map_err(|error| {
                            CodegenError(format!(
                                "check array assembly size sum @{symbol}: {error}",
                                symbol = function.symbol()
                            ))
                        })?;
                    self.array_size_check(
                        overflow,
                        *overflow_message,
                        &format!("assembly.size.sum.ok.{part_index}"),
                    )?;
                    let exceeds_long_max = builder
                        .build_int_compare(
                            IntPredicate::UGT,
                            next_total,
                            i64_ty.const_int(i64::MAX as u64, false),
                            "assembly_long_size_overflow",
                        )
                        .map_err(|error| {
                            CodegenError(format!(
                                "check array assembly Long size limit @{symbol}: {error}",
                                symbol = function.symbol()
                            ))
                        })?;
                    self.array_size_check(
                        exceeds_long_max,
                        *overflow_message,
                        &format!("assembly.size.long.ok.{part_index}"),
                    )?;
                    total = next_total;
                }

                let total_bytes =
                    self.checked_array_allocation_size(&layout, total, *overflow_message)?;
                let safepoint = self.safepoint_id(*safepoint);
                let live = self.materialize_statepoint_live(live, safepoint)?;
                let array =
                    self.managed_alloc_value(td.as_pointer_value(), total_bytes, safepoint, live)?;
                let size_ptr = self.byte_gep(array, 16, "assembly_size_ptr")?;
                builder.build_store(size_ptr, total).map_err(|error| {
                    CodegenError(format!(
                        "store array assembly size @{symbol}: {error}",
                        symbol = function.symbol()
                    ))
                })?;

                let mut destination_index = i64_ty.const_zero();
                for (part_index, part) in parts.iter().enumerate() {
                    if matches!(
                        layout.storage().kind(),
                        scoop_lir::ArrayElementStorageKindV1::ZeroSized { .. }
                    ) {
                        continue;
                    }
                    match part {
                        scoop_lir::ArrayAssemblyPart::Element(value) => {
                            let destination = self.element_ptr(
                                array,
                                &layout,
                                destination_index,
                                "assembly_element_ptr",
                            )?;
                            builder
                                .build_store(destination, self.value(*value)?)
                                .map_err(|error| {
                                    CodegenError(format!(
                                        "store array assembly element @{symbol}: {error}",
                                        symbol = function.symbol()
                                    ))
                                })?;
                            destination_index = builder
                                .build_int_add(
                                    destination_index,
                                    i64_ty.const_int(1, false),
                                    "assembly_next_destination",
                                )
                                .map_err(|error| {
                                    CodegenError(format!(
                                        "advance array assembly destination @{symbol}: {error}",
                                        symbol = function.symbol()
                                    ))
                                })?;
                        }
                        scoop_lir::ArrayAssemblyPart::CopyArray(source) => {
                            let source = self.value(*source)?.into_pointer_value();
                            let size_ptr = self.byte_gep(source, 16, "assembly_copy_size_ptr")?;
                            let size = builder
                                .build_load(i64_ty, size_ptr, "assembly_copy_size")
                                .map_err(|error| {
                                    CodegenError(format!(
                                        "load array assembly copy size @{symbol}: {error}",
                                        symbol = function.symbol()
                                    ))
                                })?
                                .into_int_value();
                            let preheader = builder.get_insert_block().ok_or_else(|| {
                                CodegenError("builder has no insertion block".to_string())
                            })?;
                            let condition = context.append_basic_block(
                                self.llvm_function,
                                &format!("assembly.copy.cond.{part_index}"),
                            );
                            let body = context.append_basic_block(
                                self.llvm_function,
                                &format!("assembly.copy.body.{part_index}"),
                            );
                            let done = context.append_basic_block(
                                self.llvm_function,
                                &format!("assembly.copy.done.{part_index}"),
                            );
                            builder
                                .build_unconditional_branch(condition)
                                .map_err(|error| {
                                    CodegenError(format!(
                                        "enter array assembly copy @{symbol}: {error}",
                                        symbol = function.symbol()
                                    ))
                                })?;
                            builder.position_at_end(condition);
                            let index = builder.build_phi(i64_ty, "assembly_copy_index").map_err(
                                |error| {
                                    CodegenError(format!(
                                        "build array assembly copy index @{symbol}: {error}",
                                        symbol = function.symbol()
                                    ))
                                },
                            )?;
                            index.add_incoming(&[(&i64_ty.const_zero(), preheader)]);
                            let index_value = index.as_basic_value().into_int_value();
                            let has_next = builder
                                .build_int_compare(
                                    IntPredicate::ULT,
                                    index_value,
                                    size,
                                    "assembly_copy_has_next",
                                )
                                .map_err(|error| {
                                    CodegenError(format!(
                                        "check array assembly copy @{symbol}: {error}",
                                        symbol = function.symbol()
                                    ))
                                })?;
                            builder
                                .build_conditional_branch(has_next, body, done)
                                .map_err(|error| {
                                    CodegenError(format!(
                                        "branch array assembly copy @{symbol}: {error}",
                                        symbol = function.symbol()
                                    ))
                                })?;
                            builder.position_at_end(body);
                            let source_element = self.element_ptr(
                                source,
                                &layout,
                                index_value,
                                "assembly_source_element_ptr",
                            )?;
                            let copied = builder
                                .build_load(element_ty, source_element, "assembly_copied_element")
                                .map_err(|error| {
                                    CodegenError(format!(
                                        "load array assembly element @{symbol}: {error}",
                                        symbol = function.symbol()
                                    ))
                                })?;
                            let destination = builder
                                .build_int_add(
                                    destination_index,
                                    index_value,
                                    "assembly_copy_destination_index",
                                )
                                .map_err(|error| {
                                    CodegenError(format!(
                                        "index array assembly destination @{symbol}: {error}",
                                        symbol = function.symbol()
                                    ))
                                })?;
                            let destination = self.element_ptr(
                                array,
                                &layout,
                                destination,
                                "assembly_copy_destination_ptr",
                            )?;
                            builder.build_store(destination, copied).map_err(|error| {
                                CodegenError(format!(
                                    "store array assembly copied element @{symbol}: {error}",
                                    symbol = function.symbol()
                                ))
                            })?;
                            let next = builder
                                .build_int_add(
                                    index_value,
                                    i64_ty.const_int(1, false),
                                    "assembly_copy_next",
                                )
                                .map_err(|error| {
                                    CodegenError(format!(
                                        "advance array assembly copy @{symbol}: {error}",
                                        symbol = function.symbol()
                                    ))
                                })?;
                            builder
                                .build_unconditional_branch(condition)
                                .map_err(|error| {
                                    CodegenError(format!(
                                        "continue array assembly copy @{symbol}: {error}",
                                        symbol = function.symbol()
                                    ))
                                })?;
                            index.add_incoming(&[(&next, body)]);
                            builder.position_at_end(done);
                            destination_index = builder
                                .build_int_add(
                                    destination_index,
                                    size,
                                    "assembly_after_copy_destination",
                                )
                                .map_err(|error| {
                                    CodegenError(format!(
                                        "advance copied array destination @{symbol}: {error}",
                                        symbol = function.symbol()
                                    ))
                                })?;
                        }
                    }
                }
                self.temps.insert(*out, array.into());
            }
            _ => unreachable!("array instruction dispatch is exhaustive"),
        }
        Ok(())
    }
}
