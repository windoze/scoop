//! Immortal compile-time constants for strings and liftable MIR aggregates.

#![allow(dead_code)]

use super::*;

const SCOOP_GC_FLAG_IMMORTAL: u64 = 0x8000_0000;
const SCOOP_GC_MARK_IMMORTAL: u64 = 0xffff_ffff;

struct ImmortalConstValue<'ctx> {
    raw: BasicValueEnum<'ctx>,
    key: String,
}

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn get_or_create_immortal_string_global(
        &mut self,
        span: crate::span::Span,
        bytes: &[u8],
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        let data_gv = self.get_or_create_global_bytes(bytes);
        let data_name = llvm_global_name(data_gv);
        let suffix = data_name
            .strip_prefix("__scoop_str_data_")
            .unwrap_or(data_name.as_str());
        let global_name = format!("__scoop_str_lit_{suffix}");

        if let Some(existing) = self.module.get_global(&global_name) {
            existing.set_constant(true);
            existing.set_unnamed_addr(true);
            return Ok(existing);
        }

        let scoop_str_ty = self.llvm_scoop_string_type();
        let header_ty = self.llvm_gc_object_header_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let i64_ty = self.context.i64_type();
        let i32_ty = self.context.i32_type();

        let type_desc = self.get_or_create_string_type_desc_global(span)?;
        let type_desc_i8 = type_desc.as_pointer_value().const_cast(i8_ptr_ty);
        let obj_size = self.target_data.get_store_size(&scoop_str_ty);
        let header_values: [BasicValueEnum<'ctx>; 5] = [
            i8_ptr_ty.const_null().into(),
            type_desc_i8.into(),
            i64_ty.const_int(obj_size, false).into(),
            i32_ty.const_int(SCOOP_GC_FLAG_IMMORTAL, false).into(),
            i32_ty.const_int(SCOOP_GC_MARK_IMMORTAL, false).into(),
        ];
        let header = header_ty.const_named_struct(&header_values);

        let data_ptr = if bytes.is_empty() {
            i8_ptr_ty.const_null()
        } else {
            data_gv.as_pointer_value().const_cast(i8_ptr_ty)
        };
        let values: [BasicValueEnum<'ctx>; 3] = [
            header.into(),
            i64_ty.const_int(bytes.len() as u64, false).into(),
            data_ptr.into(),
        ];
        let init = scoop_str_ty.const_named_struct(&values);

        let gv = self
            .module
            .add_global(scoop_str_ty, Some(self.gc_address_space()), &global_name);
        gv.set_initializer(&init);
        gv.set_constant(true);
        gv.set_unnamed_addr(true);
        gv.set_linkage(Linkage::Internal);
        Ok(gv)
    }

    pub(in crate::llvm::codegen) fn try_emit_immortal_tuple(
        &mut self,
        span: crate::span::Span,
        mir_types: &TypeStore,
        elements: &[crate::mir::Operand],
        transport: &crate::mir::AggregateTransportMetadata,
        target_cg: CgTy,
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        if elements
            .iter()
            .any(|element| !matches!(element, crate::mir::Operand::Const(_)))
            || self
                .immortal_aggregate_codegen_ty(mir_types, transport)?
                .is_none()
        {
            return Ok(None);
        }

        let CgTy::Tuple(tuple_ty) = target_cg else {
            return Ok(None);
        };
        let TypeKind::Value(ValueTypeKind::Tuple(element_tys)) = self.types.kind(tuple_ty.inner())
        else {
            panic!("try_emit_immortal_tuple: MIR verifier accepted tuple target without schema");
        };
        if element_tys.len() != elements.len() || transport.fields.len() != elements.len() {
            return Ok(None);
        }

        let mut init_values = Vec::with_capacity(elements.len());
        let mut key_parts = Vec::with_capacity(elements.len());
        for (idx, (element, elem_ty)) in elements.iter().zip(element_tys.iter()).enumerate() {
            let crate::mir::Operand::Const(const_value) = element else {
                return Ok(None);
            };
            let elem_cg = self.try_cg_ty_of_type_id(*elem_ty).unwrap_or_else(|| {
                panic!("try_emit_immortal_tuple: MIR verifier accepted unsupported tuple element")
            });
            let Some(value) = self.try_emit_immortal_const_operand(span, const_value, elem_cg)?
            else {
                return Ok(None);
            };
            init_values.push(value.raw);
            key_parts.push(format!("{idx}:{}", value.key));
        }

        let llvm_tuple_ty = self.llvm_tuple_type(span, tuple_ty)?;
        let init = llvm_tuple_ty.const_named_struct(&init_values);
        self.load_immortal_aggregate_global(
            target_cg,
            llvm_tuple_ty.into(),
            init.into(),
            transport.aggregate_ty,
            key_parts.as_slice(),
        )
        .map(Some)
    }

    pub(in crate::llvm::codegen) fn try_emit_immortal_struct(
        &mut self,
        span: crate::span::Span,
        mir_types: &TypeStore,
        fields: &[crate::mir::StructLitField],
        transport: &crate::mir::AggregateTransportMetadata,
        target_cg: CgTy,
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        if fields
            .iter()
            .any(|field| !matches!(field.value, crate::mir::Operand::Const(_)))
            || self
                .immortal_aggregate_codegen_ty(mir_types, transport)?
                .is_none()
        {
            return Ok(None);
        }

        let aggregate_ty = self
            .equivalent_codegen_type_id(mir_types, transport.aggregate_ty)
            .unwrap_or_else(|| {
                panic!("try_emit_immortal_struct: MIR verifier accepted aggregate TypeStore drift")
            });
        if self
            .scalar_layout_struct_field(aggregate_ty, target_cg)?
            .is_some()
        {
            return Ok(None);
        }

        let CgTy::Struct(struct_ty) = target_cg else {
            return Ok(None);
        };
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(struct_ty.inner())
        else {
            panic!("try_emit_immortal_struct: MIR verifier accepted struct target without schema");
        };
        let layout_key = self.nominal_layout_key(nominal);
        let layout = self.struct_layouts.get(&layout_key).unwrap_or_else(|| {
            panic!("try_emit_immortal_struct: MIR verifier accepted struct without layout")
        });
        if layout.fields.len() != fields.len() || transport.fields.len() != fields.len() {
            return Ok(None);
        }

        let llvm_struct_ty = self.llvm_struct_type(span, struct_ty)?;
        let mut init_values: Vec<BasicValueEnum<'ctx>> = llvm_struct_ty
            .get_field_types()
            .into_iter()
            .map(|ty| self.zero_initializer_for_basic_type(ty))
            .collect();
        let mut key_parts = Vec::with_capacity(fields.len());

        for (idx, layout_field) in layout.fields.iter().enumerate() {
            let Some(init) = fields.iter().find(|field| field.name == layout_field.name) else {
                return Ok(None);
            };
            let crate::mir::Operand::Const(const_value) = &init.value else {
                return Ok(None);
            };
            let field_cg = self.cg_ty_of_layout_field(
                init.span,
                layout_field.ty,
                layout_field.ty_fqn.as_deref(),
            )?;
            let Some(value) =
                self.try_emit_immortal_const_operand(init.span, const_value, field_cg)?
            else {
                return Ok(None);
            };
            let llvm_idx = self
                .shared_caches
                .pack_field_indices
                .borrow()
                .get(&layout_key)
                .map_or(idx as u32, |indices| indices[idx]);
            let Some(slot) = init_values.get_mut(llvm_idx as usize) else {
                return Ok(None);
            };
            *slot = value.raw;
            key_parts.push(format!("{}:{}", layout_field.name, value.key));
        }

        let init = llvm_struct_ty.const_named_struct(&init_values);
        self.load_immortal_aggregate_global(
            target_cg,
            llvm_struct_ty.into(),
            init.into(),
            transport.aggregate_ty,
            key_parts.as_slice(),
        )
        .map(Some)
    }

    fn immortal_aggregate_codegen_ty(
        &mut self,
        mir_types: &TypeStore,
        transport: &crate::mir::AggregateTransportMetadata,
    ) -> Result<Option<TypeId>, LlvmEmitError> {
        if !transport.is_tuple_or_struct() || !transport.fields_have_no_boxing() {
            return Ok(None);
        }
        let Some(aggregate_ty) = self.equivalent_codegen_type_id(mir_types, transport.aggregate_ty)
        else {
            return Ok(None);
        };
        match self.types.kind(aggregate_ty) {
            TypeKind::Value(_) => Ok(Some(aggregate_ty)),
            TypeKind::Ref(_) => {
                let mut immutability = self.type_immutability();
                Ok(immutability
                    .is_immutable(aggregate_ty)
                    .then_some(aggregate_ty))
            }
            TypeKind::Param(_) | TypeKind::StarProjection(_) => Ok(None),
        }
    }

    fn try_emit_immortal_const_operand(
        &mut self,
        span: crate::span::Span,
        value: &crate::mir::ConstValue,
        expected: CgTy,
    ) -> Result<Option<ImmortalConstValue<'ctx>>, LlvmEmitError> {
        let raw = match (value, expected) {
            (crate::mir::ConstValue::Bool(v), CgTy::Bool) => ImmortalConstValue {
                raw: self.context.bool_type().const_int(*v as u64, false).into(),
                key: format!("bool:{v}"),
            },
            (crate::mir::ConstValue::Char, CgTy::Int(int_ty))
                if int_ty.bits == 32 && !int_ty.signed =>
            {
                let text = self.current_source_slice(span)?;
                let parsed = crate::syntax::char_literal::parse_char_literal(text)
                    .unwrap_or_else(|_| {
                        panic!(
                            "try_emit_immortal_const_operand: parser/typecheck accepted invalid char literal"
                        )
                    });
                ImmortalConstValue {
                    raw: self
                        .context
                        .i32_type()
                        .const_int(parsed as u64, false)
                        .into(),
                    key: format!("char:{:x}", parsed as u32),
                }
            }
            (crate::mir::ConstValue::Unit, CgTy::Unit) => ImmortalConstValue {
                raw: self.context.i8_type().const_zero().into(),
                key: "unit".to_string(),
            },
            (crate::mir::ConstValue::Int, CgTy::Int(int_ty)) => {
                let bits = match self.int_literal_bits_from_source_span_if_present(span, int_ty)? {
                    Some(bits) => bits,
                    None => self.int_literal_bits_for_ty(span, int_ty)?,
                };
                ImmortalConstValue {
                    raw: self.int_type(int_ty).const_int(bits, false).into(),
                    key: format!("int{}:{}:{bits}", int_ty.bits, int_ty.signed),
                }
            }
            (crate::mir::ConstValue::SynthInt(v), CgTy::Int(int_ty)) => ImmortalConstValue {
                raw: self
                    .int_type(int_ty)
                    .const_int(*v as u64, int_ty.signed)
                    .into(),
                key: format!("int{}:{}:{v}", int_ty.bits, int_ty.signed),
            },
            (crate::mir::ConstValue::Float64, CgTy::Float64) => {
                let parsed = crate::syntax::float_literal::parse_float_literal(
                    self.current_source_slice(span)?,
                );
                ImmortalConstValue {
                    raw: self.context.f64_type().const_float(parsed.value).into(),
                    key: format!("f64:{:x}", parsed.value.to_bits()),
                }
            }
            (crate::mir::ConstValue::Float32, CgTy::Float32) => {
                let parsed = crate::syntax::float_literal::parse_float_literal(
                    self.current_source_slice(span)?,
                );
                ImmortalConstValue {
                    raw: self.context.f32_type().const_float(parsed.value).into(),
                    key: format!("f32:{:x}", (parsed.value as f32).to_bits()),
                }
            }
            (crate::mir::ConstValue::String, CgTy::String) => {
                let bytes = self.parse_current_string_literal_bytes(span)?;
                self.immortal_string_const_value(span, &bytes)?
            }
            (crate::mir::ConstValue::SynthString(text), CgTy::String) => {
                self.immortal_string_const_value(span, text.as_bytes())?
            }
            _ => return Ok(None),
        };
        Ok(Some(raw))
    }

    fn immortal_string_const_value(
        &mut self,
        span: crate::span::Span,
        bytes: &[u8],
    ) -> Result<ImmortalConstValue<'ctx>, LlvmEmitError> {
        let gv = self.get_or_create_immortal_string_global(span, bytes)?;
        Ok(ImmortalConstValue {
            raw: gv.as_pointer_value().into(),
            key: format!("string:{}", llvm_global_name(gv)),
        })
    }

    fn load_immortal_aggregate_global(
        &mut self,
        target_cg: CgTy,
        llvm_ty: BasicTypeEnum<'ctx>,
        init: BasicValueEnum<'ctx>,
        aggregate_ty: TypeId,
        key_parts: &[String],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let key = format!(
            "{}|{}",
            self.types.display(aggregate_ty),
            key_parts.join("|")
        );
        let hash = super::alloca::string_byte_data_hash(key.as_bytes());
        let name = format!("__scoop_immortal_agg_{hash}");
        let gv = if let Some(existing) = self.module.get_global(&name) {
            existing.set_constant(true);
            existing.set_unnamed_addr(true);
            existing
        } else {
            let gv = self.module.add_global(llvm_ty, None, &name);
            gv.set_initializer(&init);
            gv.set_constant(true);
            gv.set_unnamed_addr(true);
            gv.set_linkage(Linkage::Internal);
            if let CgTy::Struct(struct_ty) = target_cg
                && let Some(aligned) = self.struct_clayout(struct_ty).and_then(|c| c.aligned)
            {
                gv.set_alignment(aligned);
            }
            gv
        };
        let loaded =
            self.builder
                .build_load(llvm_ty, gv.as_pointer_value(), "immortal_aggregate_load")?;
        Ok(CgValue {
            ty: target_cg,
            value: Some(loaded),
        })
    }
}

fn llvm_global_name(global: GlobalValue<'_>) -> String {
    global.get_name().to_string_lossy().into_owned()
}
