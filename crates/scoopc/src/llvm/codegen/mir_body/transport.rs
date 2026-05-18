//! MIR value-transport and composite-value boxing lowering.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_mir_value_transport(
        &mut self,
        span: crate::span::Span,
        value: &crate::mir::Operand,
        transport: &crate::mir::ValueTransportMetadata,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        target_cg: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let body_fqn = self.current_codegen_body_fqn();
        let Some(boxing) = transport.boxing.as_ref() else {
            return Err(super::composite_transport::composite_transport_gate_error(
                &body_fqn,
                span,
                "PIPELINE_GAPS §4.1",
                "composite value erasure must publish boxing intent before LLVM lowering",
            ));
        };
        if !matches!(
            boxing.reason,
            crate::mir::MirBoxingReason::AnyErasure | crate::mir::MirBoxingReason::RefErasure
        ) || boxing.source_ty != transport.source_ty
        {
            return Err(super::composite_transport::composite_transport_gate_error(
                &body_fqn,
                span,
                "PIPELINE_GAPS §4.1",
                "composite value erasure boxing intent must remain aligned with the published source transport",
            ));
        }
        let source_ty = self
            .equivalent_codegen_type_id(mir_types, transport.source_ty)
            .ok_or_else(|| {
                super::composite_transport::composite_transport_gate_error(
                    &body_fqn,
                    span,
                    "PIPELINE_GAPS §4.1",
                    "composite value erasure source type must stay materialized before LLVM lowering",
                )
            })?;
        let source_cg = self.cg_ty_of(source_ty).ok_or_else(|| {
            super::composite_transport::composite_transport_gate_error(
                &body_fqn,
                span,
                "PIPELINE_GAPS §4.1",
                "composite value erasure source layout must stay queryable before LLVM lowering",
            )
        })?;

        if matches!(
            mir_types.kind(transport.source_ty),
            TypeKind::Value(ValueTypeKind::Nothing)
        ) {
            return self.default_value(span, target_cg);
        }

        if target_cg == CgTy::String {
            let source = self.codegen_mir_operand_expected(span, value, slots, Some(source_cg))?;
            let source = self.coerce_value(span, source, source_cg)?;
            if source_cg == CgTy::String {
                return self.coerce_value(span, source, CgTy::String);
            }
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR transport to String requires ordinary ToString lowering",
                at: span.into(),
            });
        }
        if target_cg != CgTy::Ref {
            return Err(super::composite_transport::composite_transport_gate_error(
                &body_fqn,
                span,
                "PIPELINE_GAPS §4.1",
                "composite value erasure must lower to Ref or String after descriptor publication",
            ));
        }
        let _descriptor = self.get_or_create_value_composite_transport_descriptor_global(
            &body_fqn, span, mir_types, transport,
        )?;

        match source_cg {
            CgTy::Tuple(_) | CgTy::Struct(_) => self.codegen_mir_composite_value_box(
                span, value, source_ty, source_cg, body, mir_types, slots,
            ),
            CgTy::Enum(_) if transport.kind == crate::mir::MirTransportKind::EnumPayload => self
                .codegen_mir_composite_value_box(
                    span, value, source_ty, source_cg, body, mir_types, slots,
                ),
            CgTy::Unit | CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref | CgTy::Enum(_) => {
                let source =
                    self.codegen_mir_operand_expected(span, value, slots, Some(source_cg))?;
                let source = self.coerce_value(span, source, source_cg)?;
                self.coerce_value(span, source, CgTy::Ref)
            }
            CgTy::Float64 | CgTy::Float32 | CgTy::Never => {
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "value erasure transport source kind",
                    at: span.into(),
                })
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_mir_composite_value_box(
        &mut self,
        span: crate::span::Span,
        value: &crate::mir::Operand,
        source_ty: TypeId,
        source_cg: CgTy,
        _body: &crate::mir::Body,
        _mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let source = self.codegen_mir_operand_expected(span, value, slots, Some(source_cg))?;
        let source = self.coerce_value(span, source, source_cg)?;
        let deferred_source =
            self.defer_gc_sensitive_cg_value(span, "mir_value_box_source", source)?;

        let box_obj_ty = self.mir_value_box_object_type(span, source_ty, source_cg)?;
        let obj_size_bytes = self.target_data.get_store_size(&box_obj_ty);
        let size_v = self.context.i64_type().const_int(obj_size_bytes, false);
        let box_desc =
            self.get_or_create_mir_value_box_type_desc_global(span, source_ty, box_obj_ty)?;
        let box_desc_i8 = self.builder.build_pointer_cast(
            box_desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "mir_value_box_desc_i8",
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call = self.build_call_preserving_gc_local_roots(
            span,
            rt_alloc,
            &[box_desc_i8.into(), size_v.into()],
            "rt_alloc_mir_value_box",
        )?;
        let raw = self.expect_basic_value(call, "scoop_alloc_typed MIR value box allocation");
        let obj_i8 = self.expect_pointer_value(raw, "scoop_alloc_typed MIR value box allocation");

        let obj_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let obj_ptr =
            self.builder
                .build_pointer_cast(obj_i8, obj_ptr_ty, "mir_value_box_obj_ptr")?;
        let deferred_obj = self.defer_gc_ref_pointer(span, "mir_value_box_obj_root", obj_ptr)?;
        let obj_ptr = self.reload_deferred_gc_ref_without_clearing(
            span,
            "mir_value_box_obj_reload",
            &deferred_obj,
        )?;
        let payload_gep =
            self.builder
                .build_struct_gep(box_obj_ty, obj_ptr, 1, "mir_value_box_payload_gep")?;
        let payload = self.materialize_deferred_cg_value(
            span,
            "mir_value_box_source_reload",
            deferred_source,
        )?;
        let _ = self.store_local_value(span, payload_gep, source_cg, payload)?;
        let obj_i8 = self.reload_deferred_gc_ref_without_clearing(
            span,
            "mir_value_box_return",
            &deferred_obj,
        )?;
        Ok(CgValue {
            ty: CgTy::Ref,
            value: Some(obj_i8.into()),
        })
    }

    pub(in crate::llvm::codegen) fn current_codegen_body_fqn(&self) -> String {
        self.function_cx
            .current_callable_fqn
            .clone()
            .unwrap_or_else(|| "<unknown>".to_string())
    }

    pub(in crate::llvm::codegen) fn codegen_mir_type_metadata_literal(
        &mut self,
        span: crate::span::Span,
        metadata: &crate::mir::TypeMetadataLiteral,
        mir_types: &TypeStore,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match metadata.kind {
            crate::mir::TypeMetadataLiteralKind::TypeNameString => {
                let type_name = metadata
                    .source_fqn
                    .clone()
                    .unwrap_or_else(|| mir_types.display(metadata.source_ty).to_string());
                self.codegen_string_literal_from_text(span, &type_name)
            }
        }
    }

    pub(in crate::llvm::codegen) fn codegen_platform_literal(
        &mut self,
        span: crate::span::Span,
        target_cg: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let CgTy::Struct(struct_ty) = target_cg else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "getPlatform intrinsic result type",
                at: span.into(),
            });
        };
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(struct_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "getPlatform intrinsic nominal Platform type",
                at: span.into(),
            });
        };
        if nominal.fqn != "scoop.core.Platform" {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "getPlatform intrinsic Platform target",
                at: span.into(),
            });
        }

        let layout_key = self.nominal_layout_key(nominal);
        let layout =
            self.struct_layouts
                .get(&layout_key)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "getPlatform intrinsic Platform layout",
                    at: span.into(),
                })?;
        let llvm_struct_ty = self.llvm_struct_type(span, struct_ty)?;
        let (arch, vendor, os, env) = decompose_target_triple(&self.host.triple);
        let field_values = [
            ("triple", self.host.triple.as_str()),
            ("arch", arch.as_str()),
            ("vendor", vendor.as_str()),
            ("os", os.as_str()),
            ("env", env.as_str()),
        ];

        let mut deferred_fields: Vec<(u32, String, DeferredCgValue<'ctx>)> =
            Vec::with_capacity(layout.fields.len());
        for (idx, layout_field) in layout.fields.iter().enumerate() {
            let (_, text) = field_values
                .iter()
                .find(|(name, _)| *name == layout_field.name)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "getPlatform intrinsic Platform field",
                    at: span.into(),
                })?;
            let field_cg =
                self.cg_ty_of_layout_field(span, layout_field.ty, layout_field.ty_fqn.as_deref())?;
            let value = self.codegen_string_literal_from_text(span, text)?;
            let value = if value.ty != field_cg {
                self.coerce_value(span, value, field_cg)?
            } else {
                value
            };
            let deferred = self.defer_gc_sensitive_cg_value(
                span,
                &format!("get_platform_field_{idx}"),
                value,
            )?;
            let llvm_idx = self
                .shared_caches
                .pack_field_indices
                .borrow()
                .get(&layout_key)
                .map_or(idx as u32, |indices| indices[idx]);
            deferred_fields.push((llvm_idx, layout_field.name.clone(), deferred));
        }

        let mut agg: AggregateValueEnum<'ctx> = llvm_struct_ty.get_undef().into();
        for (idx, (llvm_idx, field_name, deferred)) in deferred_fields.into_iter().enumerate() {
            let materialized = self.materialize_deferred_cg_value(
                span,
                &format!("get_platform_field_reload_{idx}"),
                deferred,
            )?;
            let raw = materialized
                .value
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "getPlatform intrinsic field value",
                    at: span.into(),
                })?;
            agg = self.builder.build_insert_value(
                agg,
                raw,
                llvm_idx,
                &format!("get_platform_insert_{field_name}"),
            )?;
        }

        Ok(CgValue {
            ty: target_cg,
            value: Some(agg.as_basic_value_enum()),
        })
    }
}
