//! MIR value-transport and composite-value boxing lowering.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_mir_value_transport(
        &mut self,
        span: crate::span::Span,
        value: &mir_source::Operand,
        transport: &mir_source::ValueTransportMetadata,
        body: &mir_source::Body,
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
            mir_source::MirBoxingReason::AnyErasure | mir_source::MirBoxingReason::RefErasure
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
        let source_cg = self.try_cg_ty_of_type_id(source_ty).ok_or_else(|| {
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
            panic!(
                "codegen_mir_value_transport: MIR transport to String reached LLVM without ordinary ToString lowering"
            );
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
            CgTy::Enum(_) if transport.kind == mir_source::MirTransportKind::EnumPayload => self
                .codegen_mir_composite_value_box(
                    span, value, source_ty, source_cg, body, mir_types, slots,
                ),
            CgTy::Float64 | CgTy::Float32 => self.codegen_mir_composite_value_box(
                span, value, source_ty, source_cg, body, mir_types, slots,
            ),
            CgTy::Unit | CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref | CgTy::Enum(_) => {
                let source =
                    self.codegen_mir_operand_expected(span, value, slots, Some(source_cg))?;
                let source = self.coerce_value(span, source, source_cg)?;
                self.coerce_value(span, source, CgTy::Ref)
            }
            CgTy::Never => {
                let source =
                    self.codegen_mir_operand_expected(span, value, slots, Some(source_cg))?;
                self.coerce_value(span, source, CgTy::Ref)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_lir_value_transport(
        &mut self,
        span: crate::span::Span,
        value: &LirOperand,
        transport: &mir_source::ValueTransportMetadata,
        _body: &LirExecutableBody,
        source_types: &TypeStore,
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
            mir_source::MirBoxingReason::AnyErasure | mir_source::MirBoxingReason::RefErasure
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
            .equivalent_codegen_type_id(source_types, transport.source_ty)
            .ok_or_else(|| {
                super::composite_transport::composite_transport_gate_error(
                    &body_fqn,
                    span,
                    "PIPELINE_GAPS §4.1",
                    "composite value erasure source type must stay materialized before LLVM lowering",
                )
            })?;
        let source_cg = self.try_cg_ty_of_type_id(source_ty).ok_or_else(|| {
            super::composite_transport::composite_transport_gate_error(
                &body_fqn,
                span,
                "PIPELINE_GAPS §4.1",
                "composite value erasure source layout must stay queryable before LLVM lowering",
            )
        })?;

        if matches!(
            source_types.kind(transport.source_ty),
            TypeKind::Value(ValueTypeKind::Nothing)
        ) {
            return self.default_value(span, target_cg);
        }

        if target_cg == CgTy::String {
            let source = self.codegen_lir_operand_expected(span, value, slots, Some(source_cg))?;
            let source = self.coerce_value(span, source, source_cg)?;
            if source_cg == CgTy::String {
                return self.coerce_value(span, source, CgTy::String);
            }
            panic!(
                "codegen_lir_value_transport: LIR transport to String reached LLVM without ordinary ToString lowering"
            );
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
            &body_fqn,
            span,
            source_types,
            transport,
        )?;

        match source_cg {
            CgTy::Tuple(_) | CgTy::Struct(_) => {
                self.codegen_lir_composite_value_box(span, value, source_ty, source_cg, slots)
            }
            CgTy::Enum(_) if transport.kind == mir_source::MirTransportKind::EnumPayload => {
                self.codegen_lir_composite_value_box(span, value, source_ty, source_cg, slots)
            }
            CgTy::Float64 | CgTy::Float32 => {
                self.codegen_lir_composite_value_box(span, value, source_ty, source_cg, slots)
            }
            CgTy::Unit | CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref | CgTy::Enum(_) => {
                let source =
                    self.codegen_lir_operand_expected(span, value, slots, Some(source_cg))?;
                let source = self.coerce_value(span, source, source_cg)?;
                self.coerce_value(span, source, CgTy::Ref)
            }
            CgTy::Never => {
                let source =
                    self.codegen_lir_operand_expected(span, value, slots, Some(source_cg))?;
                self.coerce_value(span, source, CgTy::Ref)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_mir_composite_value_box(
        &mut self,
        span: crate::span::Span,
        value: &mir_source::Operand,
        source_ty: TypeId,
        source_cg: CgTy,
        _body: &mir_source::Body,
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

    pub(in crate::llvm::codegen) fn codegen_lir_composite_value_box(
        &mut self,
        span: crate::span::Span,
        value: &LirOperand,
        source_ty: TypeId,
        source_cg: CgTy,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let source = self.codegen_lir_operand_expected(span, value, slots, Some(source_cg))?;
        let source = self.coerce_value(span, source, source_cg)?;
        let deferred_source =
            self.defer_gc_sensitive_cg_value(span, "lir_value_box_source", source)?;

        let box_obj_ty = self.mir_value_box_object_type(span, source_ty, source_cg)?;
        let obj_size_bytes = self.target_data.get_store_size(&box_obj_ty);
        let size_v = self.context.i64_type().const_int(obj_size_bytes, false);
        let box_desc =
            self.get_or_create_mir_value_box_type_desc_global(span, source_ty, box_obj_ty)?;
        let box_desc_i8 = self.builder.build_pointer_cast(
            box_desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "lir_value_box_desc_i8",
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call = self.build_call_preserving_gc_local_roots(
            span,
            rt_alloc,
            &[box_desc_i8.into(), size_v.into()],
            "rt_alloc_lir_value_box",
        )?;
        let raw = self.expect_basic_value(call, "scoop_alloc_typed LIR value box allocation");
        let obj_i8 = self.expect_pointer_value(raw, "scoop_alloc_typed LIR value box allocation");

        let obj_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let obj_ptr =
            self.builder
                .build_pointer_cast(obj_i8, obj_ptr_ty, "lir_value_box_obj_ptr")?;
        let deferred_obj = self.defer_gc_ref_pointer(span, "lir_value_box_obj_root", obj_ptr)?;
        let obj_ptr = self.reload_deferred_gc_ref_without_clearing(
            span,
            "lir_value_box_obj_reload",
            &deferred_obj,
        )?;
        let payload_gep =
            self.builder
                .build_struct_gep(box_obj_ty, obj_ptr, 1, "lir_value_box_payload_gep")?;
        let payload = self.materialize_deferred_cg_value(
            span,
            "lir_value_box_source_reload",
            deferred_source,
        )?;
        let _ = self.store_local_value(span, payload_gep, source_cg, payload)?;
        let obj_i8 = self.reload_deferred_gc_ref_without_clearing(
            span,
            "lir_value_box_return",
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
        metadata: &mir_source::TypeMetadataLiteral,
        mir_types: &TypeStore,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match metadata.kind {
            mir_source::TypeMetadataLiteralKind::TypeNameString => {
                // Type metadata strings are immutable metadata values, so they can share the
                // ordinary immortal String pool and remain pointer-stable across repeated reads.
                let type_name = metadata
                    .source_fqn
                    .clone()
                    .unwrap_or_else(|| mir_types.display(metadata.source_ty).to_string());
                self.codegen_string_literal_from_text(span, &type_name)
            }
        }
    }

    pub(in crate::llvm::codegen) fn codegen_lir_type_metadata_literal(
        &mut self,
        span: crate::span::Span,
        metadata: &crate::effect_lowered::LirTypeMetadataLiteral,
        source_types: &TypeStore,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match metadata.kind {
            mir_source::TypeMetadataLiteralKind::TypeNameString => {
                let type_name = metadata
                    .source_nominal
                    .as_ref()
                    .map(|key| key.as_str().to_string())
                    .unwrap_or_else(|| source_types.display(metadata.source_ty).to_string());
                self.codegen_string_literal_from_text(span, &type_name)
            }
        }
    }
}
