use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn emit_enum_instruction(
        &mut self,
        instruction: &Instruction,
    ) -> Result<(), CodegenError> {
        let context = self.context;
        let builder = self.builder;
        let function = self.function;
        match instruction {
            Instruction::EnumWrap {
                out,
                enum_id,
                variant,
                fields,
            } => {
                let def = &self.enums[*enum_id];
                let name = format!("t{}", out.into_raw().into_u32());
                let result: BasicValueEnum = match &def.repr {
                    EnumRepr::Niche { payload_variant } => {
                        if *variant == *payload_variant {
                            // The payload is the bare pointer itself.
                            self.value(fields[0])?
                        } else {
                            // The payload-less variant is the null pointer.
                            basic_ty(
                                context,
                                self.structs,
                                self.enums,
                                self.managed_address_space,
                                &function.temps[*out].ty,
                            )?
                            .into_pointer_type()
                            .const_null()
                            .into()
                        }
                    }
                    EnumRepr::Tagged {
                        variants,
                        size,
                        align,
                    } => {
                        // The full value is zero before tag/payload writes,
                        // so every inactive ref-bearing slot is safe for
                        // unconditional GC scanning.
                        let ty = tagged_ty(
                            context,
                            self.managed_address_space,
                            *size,
                            *align,
                            &def.scan,
                        )?;
                        let slot = self.entry_alloca(ty.into(), "enum_wrap")?;
                        builder.build_store(slot, ty.const_zero()).map_err(|e| {
                            CodegenError(format!(
                                "enum_wrap zero @{symbol}: {e}",
                                symbol = function.symbol
                            ))
                        })?;
                        let tag_ptr = self.tag_ptr(slot, ty)?;
                        builder
                            .build_store(
                                tag_ptr,
                                context.i64_type().const_int(*variant as u64, false),
                            )
                            .map_err(|e| {
                                CodegenError(format!(
                                    "enum_wrap tag @{symbol}: {e}",
                                    symbol = function.symbol
                                ))
                            })?;
                        let variant_repr = &variants[*variant as usize];
                        for (value, field) in fields.iter().zip(&variant_repr.fields) {
                            let field_ptr = self.enum_field_ptr(slot, field.offset, "field_ptr")?;
                            builder
                                .build_store(field_ptr, self.value(*value)?)
                                .map_err(|e| {
                                    CodegenError(format!(
                                        "enum_wrap field @{symbol}: {e}",
                                        symbol = function.symbol
                                    ))
                                })?;
                        }
                        builder.build_load(ty, slot, &name).map_err(|e| {
                            CodegenError(format!(
                                "enum_wrap @{symbol}: {e}",
                                symbol = function.symbol
                            ))
                        })?
                    }
                };
                self.temps.insert(*out, result);
            }
            Instruction::EnumTag {
                out,
                enum_id,
                operand,
            } => {
                let def = &self.enums[*enum_id];
                let operand = self.value(*operand)?;
                let name = format!("t{}", out.into_raw().into_u32());
                let result: BasicValueEnum = match &def.repr {
                    // Niche: the tag value is the variant index — null ↔
                    // the payload-less variant, non-null ↔ the payload
                    // variant.
                    EnumRepr::Niche { payload_variant } => {
                        let operand = operand.into_pointer_value();
                        let non_null = builder
                            .build_int_compare(
                                IntPredicate::NE,
                                operand,
                                operand.get_type().const_null(),
                                "non_null",
                            )
                            .map_err(|e| {
                                CodegenError(format!(
                                    "enum_tag @{symbol}: {e}",
                                    symbol = function.symbol
                                ))
                            })?;
                        let i64_ty = context.i64_type();
                        builder
                            .build_select(
                                non_null,
                                i64_ty.const_int(*payload_variant as u64, false),
                                i64_ty.const_int((1 - *payload_variant) as u64, false),
                                &name,
                            )
                            .map_err(|e| {
                                CodegenError(format!(
                                    "enum_tag @{symbol}: {e}",
                                    symbol = function.symbol
                                ))
                            })?
                    }
                    EnumRepr::Tagged { .. } => builder
                        .build_extract_value(operand.into_struct_value(), 0, &name)
                        .map_err(|e| {
                            CodegenError(format!(
                                "enum_tag @{symbol}: {e}",
                                symbol = function.symbol
                            ))
                        })?,
                };
                self.temps.insert(*out, result);
            }
            Instruction::EnumField {
                out,
                enum_id,
                variant,
                index,
                operand,
            } => {
                let def = &self.enums[*enum_id];
                let operand = self.value(*operand)?;
                let name = format!("t{}", out.into_raw().into_u32());
                let result: BasicValueEnum = match &def.repr {
                    // Niche: the payload is the pointer itself; lir-lower
                    // only emits this on paths where the tag is known.
                    EnumRepr::Niche { .. } => operand,
                    EnumRepr::Tagged {
                        variants,
                        size,
                        align,
                    } => {
                        // Reverse of EnumWrap: spill the aggregate into an
                        // entry-block alloca, then load the field out of
                        // the payload area.
                        let ty = tagged_ty(
                            context,
                            self.managed_address_space,
                            *size,
                            *align,
                            &def.scan,
                        )?;
                        let slot = self.entry_alloca(ty.into(), "enum_field")?;
                        builder.build_store(slot, operand).map_err(|e| {
                            CodegenError(format!(
                                "enum_field @{symbol}: {e}",
                                symbol = function.symbol
                            ))
                        })?;
                        let variant_repr = &variants[*variant as usize];
                        let field = &variant_repr.fields[*index as usize];
                        let field_ptr = self.enum_field_ptr(slot, field.offset, "field_ptr")?;
                        let field_ty = basic_ty(
                            context,
                            self.structs,
                            self.enums,
                            self.managed_address_space,
                            &field.ty,
                        )?;
                        builder
                            .build_load(field_ty, field_ptr, &name)
                            .map_err(|e| {
                                CodegenError(format!(
                                    "enum_field @{symbol}: {e}",
                                    symbol = function.symbol
                                ))
                            })?
                    }
                };
                self.temps.insert(*out, result);
            }
            _ => unreachable!("instruction dispatcher routes only enum instructions"),
        }
        Ok(())
    }
}
