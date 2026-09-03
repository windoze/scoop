use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn emit_array_instruction(
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
                let live = self.materialize_statepoint_live(live, *safepoint)?;
                // `{ ptr td, i64 gc_word, i64 size, [n x elem] }`
                // (runtime spec 2.5; the 16-byte header is M9):
                // allocate align_up(24, element_align) + n * stride
                // bytes, store the size at offset 16, then store each
                // element in order. The rounded data offset is visible
                // for over-aligned C-layout elements.
                let (array_metadata, td) = self.array_type(*array_type);
                let element_ty = basic_ty(
                    context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    &array_metadata.element,
                )?;
                let stride = array_metadata.element_size;
                let data_offset = array_data_offset(array_metadata.element_align);
                let td = td.as_pointer_value();
                let total = data_offset + elements.len() as u64 * stride;
                let array = self.managed_alloc_value(
                    td,
                    context.i64_type().const_int(total, false),
                    *safepoint,
                    live,
                )?;
                let size_ptr = self.byte_gep(array, 16, "size_ptr")?;
                builder
                    .build_store(
                        size_ptr,
                        context.i64_type().const_int(elements.len() as u64, false),
                    )
                    .map_err(|e| {
                        CodegenError(format!(
                            "array size @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                for (index, element_value) in elements.iter().enumerate() {
                    let element_ptr = self.element_ptr(
                        array,
                        element_ty,
                        context.i64_type().const_int(index as u64, false),
                        "element_ptr",
                    )?;
                    builder
                        .build_store(element_ptr, self.value(*element_value)?)
                        .map_err(|e| {
                            CodegenError(format!(
                                "array element @{symbol}: {e}",
                                symbol = function.symbol
                            ))
                        })?;
                }
                self.temps.insert(*out, array.into());
            }
            Instruction::ArrayAssembly {
                out,
                parts,
                array_type,
                safepoint,
                live,
            } => {
                let (array_metadata, td) = self.array_type(*array_type);
                let element_layout = array_metadata.element.clone();
                let stride = array_metadata.element_size;
                let element_align = array_metadata.element_align;
                let element_ty = basic_ty(
                    context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    &element_layout,
                )?;
                let i64_ty = context.i64_type();
                let mut total = i64_ty.const_int(
                    parts
                        .iter()
                        .filter(|part| matches!(part, scoop_lir::ArrayAssemblyPart::Element(_)))
                        .count() as u64,
                    false,
                );
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
                                symbol = function.symbol
                            ))
                        })?
                        .into_int_value();
                    let next_total = builder
                        .build_int_add(total, size, "assembly_total_size")
                        .map_err(|error| {
                            CodegenError(format!(
                                "sum array assembly size @{symbol}: {error}",
                                symbol = function.symbol
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
                                symbol = function.symbol
                            ))
                        })?;
                    self.array_size_check(overflow, &format!("assembly.size.sum.ok.{part_index}"))?;
                    total = next_total;
                }

                let data_offset = array_data_offset(element_align);
                if let Some(maximum_elements) = (u64::MAX - data_offset).checked_div(stride) {
                    let overflow = builder
                        .build_int_compare(
                            IntPredicate::UGT,
                            total,
                            i64_ty.const_int(maximum_elements, false),
                            "assembly_bytes_overflow",
                        )
                        .map_err(|error| {
                            CodegenError(format!(
                                "check array assembly allocation size @{symbol}: {error}",
                                symbol = function.symbol
                            ))
                        })?;
                    self.array_size_check(overflow, "assembly.size.bytes.ok")?;
                }
                let total_bytes = builder
                    .build_int_mul(
                        total,
                        i64_ty.const_int(stride, false),
                        "assembly_element_bytes",
                    )
                    .and_then(|bytes| {
                        builder.build_int_add(
                            bytes,
                            i64_ty.const_int(data_offset, false),
                            "assembly_total_bytes",
                        )
                    })
                    .map_err(|error| {
                        CodegenError(format!(
                            "compute array assembly allocation @{symbol}: {error}",
                            symbol = function.symbol
                        ))
                    })?;
                let live = self.materialize_statepoint_live(live, *safepoint)?;
                let array =
                    self.managed_alloc_value(td.as_pointer_value(), total_bytes, *safepoint, live)?;
                let size_ptr = self.byte_gep(array, 16, "assembly_size_ptr")?;
                builder.build_store(size_ptr, total).map_err(|error| {
                    CodegenError(format!(
                        "store array assembly size @{symbol}: {error}",
                        symbol = function.symbol
                    ))
                })?;

                let mut destination_index = i64_ty.const_zero();
                for (part_index, part) in parts.iter().enumerate() {
                    match part {
                        scoop_lir::ArrayAssemblyPart::Element(value) => {
                            let destination = self.element_ptr(
                                array,
                                element_ty,
                                destination_index,
                                "assembly_element_ptr",
                            )?;
                            builder
                                .build_store(destination, self.value(*value)?)
                                .map_err(|error| {
                                    CodegenError(format!(
                                        "store array assembly element @{symbol}: {error}",
                                        symbol = function.symbol
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
                                        symbol = function.symbol
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
                                        symbol = function.symbol
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
                                        symbol = function.symbol
                                    ))
                                })?;
                            builder.position_at_end(condition);
                            let index = builder.build_phi(i64_ty, "assembly_copy_index").map_err(
                                |error| {
                                    CodegenError(format!(
                                        "build array assembly copy index @{symbol}: {error}",
                                        symbol = function.symbol
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
                                        symbol = function.symbol
                                    ))
                                })?;
                            builder
                                .build_conditional_branch(has_next, body, done)
                                .map_err(|error| {
                                    CodegenError(format!(
                                        "branch array assembly copy @{symbol}: {error}",
                                        symbol = function.symbol
                                    ))
                                })?;
                            builder.position_at_end(body);
                            let source_element = self.element_ptr(
                                source,
                                element_ty,
                                index_value,
                                "assembly_source_element_ptr",
                            )?;
                            let copied = builder
                                .build_load(element_ty, source_element, "assembly_copied_element")
                                .map_err(|error| {
                                    CodegenError(format!(
                                        "load array assembly element @{symbol}: {error}",
                                        symbol = function.symbol
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
                                        symbol = function.symbol
                                    ))
                                })?;
                            let destination = self.element_ptr(
                                array,
                                element_ty,
                                destination,
                                "assembly_copy_destination_ptr",
                            )?;
                            builder.build_store(destination, copied).map_err(|error| {
                                CodegenError(format!(
                                    "store array assembly copied element @{symbol}: {error}",
                                    symbol = function.symbol
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
                                        symbol = function.symbol
                                    ))
                                })?;
                            builder
                                .build_unconditional_branch(condition)
                                .map_err(|error| {
                                    CodegenError(format!(
                                        "continue array assembly copy @{symbol}: {error}",
                                        symbol = function.symbol
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
                                        symbol = function.symbol
                                    ))
                                })?;
                        }
                    }
                }
                self.temps.insert(*out, array.into());
            }
            Instruction::ArrayLen { out, operand, .. } => {
                let array = self.value(*operand)?.into_pointer_value();
                let size_ptr = self.byte_gep(array, 16, "size_ptr")?;
                let name = format!("t{}", out.into_raw().into_u32());
                let size = builder
                    .build_load(context.i64_type(), size_ptr, &name)
                    .map_err(|e| {
                        CodegenError(format!(
                            "array_len @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                self.temps.insert(*out, size);
            }
            Instruction::ArrayGet {
                out,
                array,
                index,
                array_type,
            } => {
                let (array_metadata, _) = self.array_type(*array_type);
                let element_ty = basic_ty(
                    context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    &array_metadata.element,
                )?;
                let array = self.value(*array)?.into_pointer_value();
                let index = self.value(*index)?.into_int_value();
                self.bounds_check(array, index)?;
                let element_ptr = self.element_ptr(array, element_ty, index, "element_ptr")?;
                let name = format!("t{}", out.into_raw().into_u32());
                let value = builder
                    .build_load(element_ty, element_ptr, &name)
                    .map_err(|e| {
                        CodegenError(format!(
                            "array_get @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                self.temps.insert(*out, value);
            }
            Instruction::ArraySet {
                array,
                index,
                value,
                array_type,
            } => {
                let (array_metadata, _) = self.array_type(*array_type);
                let element_ty = basic_ty(
                    context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    &array_metadata.element,
                )?;
                let array = self.value(*array)?.into_pointer_value();
                let index = self.value(*index)?.into_int_value();
                self.bounds_check(array, index)?;
                let element_ptr = self.element_ptr(array, element_ty, index, "element_ptr")?;
                builder
                    .build_store(element_ptr, self.value(*value)?)
                    .map_err(|e| {
                        CodegenError(format!(
                            "array_set @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                // M9 write barrier: mark the stored-to address's card
                // (array element stores are heap stores too).
                self.card_mark(element_ptr)?;
            }
            Instruction::ArrayClone {
                out,
                operand,
                array_type,
                safepoint,
                live,
            } => {
                let live = self.materialize_statepoint_live(live, *safepoint)?;
                let operand = self.statepoint_value(*operand, &live)?;
                // The target descriptor is explicit: converting Array<T> to
                // MutableArray<T> (or back) changes nominal runtime identity.
                let (array_metadata, target_td) = self.array_type(*array_type);
                let stride = array_metadata.element_size;
                let data_offset = array_data_offset(array_metadata.element_align);
                let clone = self.runtime_fn(
                    scoop_lir::ARRAY_CLONE_SYMBOL,
                    managed_ptr_ty(context, self.managed_address_space).fn_type(
                        &[
                            managed_ptr_ty(context, self.managed_address_space).into(),
                            ptr_ty(context).into(),
                            context.i64_type().into(),
                            context.i64_type().into(),
                        ],
                        false,
                    ),
                );
                let name = format!("t{}", out.into_raw().into_u32());
                let call = builder
                    .build_call(
                        clone,
                        &[
                            operand.into(),
                            target_td.as_pointer_value().into(),
                            context.i64_type().const_int(stride, false).into(),
                            context.i64_type().const_int(data_offset, false).into(),
                        ],
                        &name,
                    )
                    .map_err(|e| {
                        CodegenError(format!(
                            "array_clone @{symbol}: {e}",
                            symbol = function.symbol
                        ))
                    })?;
                self.apply_safepoint_id(call, *safepoint);
                let result = call.try_as_basic_value().basic().ok_or_else(|| {
                    CodegenError(format!(
                        "call @{} produced no value",
                        scoop_lir::ARRAY_CLONE_SYMBOL
                    ))
                })?;
                self.restore_statepoint_live(live, *safepoint)?;
                self.temps.insert(*out, result);
            }
            _ => unreachable!("instruction dispatcher routes only array instructions"),
        }
        Ok(())
    }
}
