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
                variant,
                fields,
            } => {
                let enum_id = variant.definition();
                let variant_index = variant.index();
                let def = &self.enums[enum_id];
                let name = format!("t{}", out.into_raw().into_u32());
                let result: BasicValueEnum = match &def.repr {
                    EnumRepr::Niche {
                        payload_variant, ..
                    } => {
                        if variant_index == *payload_variant {
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
                            .const_zero()
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
                                symbol = function.symbol()
                            ))
                        })?;
                        let tag_ptr = self.tag_ptr(slot, ty)?;
                        builder
                            .build_store(
                                tag_ptr,
                                context.i64_type().const_int(variant_index as u64, false),
                            )
                            .map_err(|e| {
                                CodegenError(format!(
                                    "enum_wrap tag @{symbol}: {e}",
                                    symbol = function.symbol()
                                ))
                            })?;
                        let variant_repr = &variants[variant_index as usize];
                        for (value, field) in fields.iter().zip(&variant_repr.fields) {
                            let field_ptr = self.enum_field_ptr(slot, field.offset, "field_ptr")?;
                            builder
                                .build_store(field_ptr, self.value(*value)?)
                                .map_err(|e| {
                                    CodegenError(format!(
                                        "enum_wrap field @{symbol}: {e}",
                                        symbol = function.symbol()
                                    ))
                                })?;
                        }
                        builder.build_load(ty, slot, &name).map_err(|e| {
                            CodegenError(format!(
                                "enum_wrap @{symbol}: {e}",
                                symbol = function.symbol()
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
                if function.temps[*out].ty != LirType::MachineScalar(MachineScalarKind::EnumTag) {
                    return Err(CodegenError(format!(
                        "enum_tag @{} must produce machine<enum-tag>",
                        function.symbol()
                    )));
                }
                let operand_ty = function.value_ty(self.globals_arena, *operand);
                if operand_ty != LirType::Enum(*enum_id) {
                    return Err(CodegenError(format!(
                        "enum_tag @{} expects e{}, got {}",
                        function.symbol(),
                        enum_id.into_raw(),
                        operand_ty.dump()
                    )));
                }
                let def = &self.enums[*enum_id];
                let operand = self.value(*operand)?;
                let name = format!("t{}", out.into_raw().into_u32());
                let result: BasicValueEnum = match &def.repr {
                    // Niche: the tag value is the variant index — null ↔
                    // the payload-less variant, non-null ↔ the payload
                    // variant.
                    EnumRepr::Niche {
                        kind,
                        payload_variant,
                    } => {
                        let operand = if *kind == scoop_lir::NullNicheKind::Interface {
                            builder
                                .build_extract_value(operand.into_struct_value(), 0, "niche_object")
                                .map_err(|e| CodegenError(format!("interface niche object: {e}")))?
                        } else {
                            operand
                        };
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
                                    symbol = function.symbol()
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
                                    symbol = function.symbol()
                                ))
                            })?
                    }
                    EnumRepr::Tagged { .. } => builder
                        .build_extract_value(operand.into_struct_value(), 0, &name)
                        .map_err(|e| {
                            CodegenError(format!(
                                "enum_tag @{symbol}: {e}",
                                symbol = function.symbol()
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
                let operand_ty = function.value_ty(self.globals_arena, *operand);
                if operand_ty != LirType::Enum(*enum_id) {
                    return Err(CodegenError(format!(
                        "enum_field @{} expects e{}, got {}",
                        function.symbol(),
                        enum_id.into_raw(),
                        operand_ty.dump()
                    )));
                }
                let expected_ty = match &def.repr {
                    EnumRepr::Niche {
                        kind,
                        payload_variant,
                    } if variant == payload_variant && *index == 0 => {
                        let out_ty = &function.temps[*out].ty;
                        let expected = kind.storage_type();
                        if out_ty != &expected {
                            return Err(CodegenError(format!(
                                "enum_field @{} niche payload produces {}, expected {}",
                                function.symbol(),
                                out_ty.dump(),
                                expected.dump(),
                            )));
                        }
                        None
                    }
                    EnumRepr::Niche { .. } => {
                        return Err(CodegenError(format!(
                            "enum_field @{} has an invalid niche variant or field index",
                            function.symbol()
                        )));
                    }
                    EnumRepr::Tagged { variants, .. } => {
                        let expected = variants
                            .get(*variant as usize)
                            .and_then(|variant| variant.fields.get(*index as usize))
                            .ok_or_else(|| {
                                CodegenError(format!(
                                    "enum_field @{} has an invalid variant or field index",
                                    function.symbol()
                                ))
                            })?;
                        Some(&expected.ty)
                    }
                };
                if let Some(expected_ty) = expected_ty
                    && &function.temps[*out].ty != expected_ty
                {
                    return Err(CodegenError(format!(
                        "enum_field @{} produces {}, expected {}",
                        function.symbol(),
                        function.temps[*out].ty.dump(),
                        expected_ty.dump()
                    )));
                }
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
                                symbol = function.symbol()
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
                                    symbol = function.symbol()
                                ))
                            })?
                    }
                };
                self.temps.insert(*out, result);
            }
            Instruction::VariantTest {
                out,
                operand,
                variant,
            } => {
                if !self.enums.contains_variant(*variant) {
                    return Err(CodegenError(format!(
                        "variant_test @{} carries a variant reference that is invalid for this module",
                        function.symbol()
                    )));
                }
                if function.temps[*out].ty != LirType::I1 {
                    return Err(CodegenError(format!(
                        "variant_test @{} must produce i1",
                        function.symbol()
                    )));
                }
                let enum_id = variant.definition();
                let operand_ty = function.value_ty(self.globals_arena, *operand);
                if operand_ty != LirType::Enum(enum_id) {
                    return Err(CodegenError(format!(
                        "variant_test @{} expects e{}, got {}",
                        function.symbol(),
                        enum_id.into_raw(),
                        operand_ty.dump()
                    )));
                }
                let def = &self.enums[enum_id];
                let operand = self.value(*operand)?;
                let name = format!("t{}", out.into_raw().into_u32());
                let result: BasicValueEnum = match &def.repr {
                    EnumRepr::Niche {
                        kind,
                        payload_variant,
                    } => {
                        let operand = if *kind == scoop_lir::NullNicheKind::Interface {
                            builder
                                .build_extract_value(operand.into_struct_value(), 0, "niche_object")
                                .map_err(|e| CodegenError(format!("interface niche object: {e}")))?
                        } else {
                            operand
                        };
                        let pointer = operand.into_pointer_value();
                        let predicate = if variant.index() == *payload_variant {
                            IntPredicate::NE
                        } else {
                            IntPredicate::EQ
                        };
                        builder
                            .build_int_compare(
                                predicate,
                                pointer,
                                pointer.get_type().const_null(),
                                &name,
                            )
                            .map_err(|error| {
                                CodegenError(format!(
                                    "variant_test @{symbol}: {error}",
                                    symbol = function.symbol()
                                ))
                            })?
                            .into()
                    }
                    EnumRepr::Tagged { .. } => {
                        let tag = builder
                            .build_extract_value(operand.into_struct_value(), 0, "variant_tag")
                            .map_err(|error| {
                                CodegenError(format!(
                                    "variant_test tag @{symbol}: {error}",
                                    symbol = function.symbol()
                                ))
                            })?
                            .into_int_value();
                        builder
                            .build_int_compare(
                                IntPredicate::EQ,
                                tag,
                                context.i64_type().const_int(variant.index() as u64, false),
                                &name,
                            )
                            .map_err(|error| {
                                CodegenError(format!(
                                    "variant_test @{symbol}: {error}",
                                    symbol = function.symbol()
                                ))
                            })?
                            .into()
                    }
                };
                self.temps.insert(*out, result);
            }
            Instruction::VariantPayloadProject {
                out,
                operand,
                field,
            } => {
                if !self.enums.contains_variant_field(*field) {
                    return Err(CodegenError(format!(
                        "variant_payload_project @{} carries a variant field reference that is invalid for this module",
                        function.symbol()
                    )));
                }
                let variant = field.variant();
                let enum_id = variant.definition();
                let operand_ty = function.value_ty(self.globals_arena, *operand);
                if operand_ty != LirType::Enum(enum_id) {
                    return Err(CodegenError(format!(
                        "variant_payload_project @{} expects e{}, got {}",
                        function.symbol(),
                        enum_id.into_raw(),
                        operand_ty.dump()
                    )));
                }
                let expected_ty = self.enums.variant_field_type(*field).ok_or_else(|| {
                    CodegenError(format!(
                        "variant_payload_project @{} has an invalid field {} for e{} v{}",
                        function.symbol(),
                        field.index(),
                        enum_id.into_raw(),
                        variant.index()
                    ))
                })?;
                if function.temps[*out].ty != expected_ty {
                    return Err(CodegenError(format!(
                        "variant_payload_project @{} produces {}, expected {}",
                        function.symbol(),
                        function.temps[*out].ty.dump(),
                        expected_ty.dump()
                    )));
                }

                let def = &self.enums[enum_id];
                let operand = self.value(*operand)?;
                let name = format!("t{}", out.into_raw().into_u32());
                let result: BasicValueEnum = match &def.repr {
                    // A niche payload is the provenance-preserving carrier
                    // itself. `variant_field_type` above proves that this is
                    // exactly field zero of the payload variant.
                    EnumRepr::Niche { .. } => operand,
                    EnumRepr::Tagged {
                        variants,
                        size,
                        align,
                    } => {
                        let ty = tagged_ty(
                            context,
                            self.managed_address_space,
                            *size,
                            *align,
                            &def.scan,
                        )?;
                        let slot = self.entry_alloca(ty.into(), "variant_payload_project")?;
                        builder.build_store(slot, operand).map_err(|error| {
                            CodegenError(format!(
                                "variant_payload_project spill @{symbol}: {error}",
                                symbol = function.symbol()
                            ))
                        })?;
                        let field =
                            &variants[variant.index() as usize].fields[field.index() as usize];
                        let field_ptr =
                            self.enum_field_ptr(slot, field.offset, "variant_payload_field_ptr")?;
                        let field_ty = basic_ty(
                            context,
                            self.structs,
                            self.enums,
                            self.managed_address_space,
                            &field.ty,
                        )?;
                        builder
                            .build_load(field_ty, field_ptr, &name)
                            .map_err(|error| {
                                CodegenError(format!(
                                    "variant_payload_project @{symbol}: {error}",
                                    symbol = function.symbol()
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
