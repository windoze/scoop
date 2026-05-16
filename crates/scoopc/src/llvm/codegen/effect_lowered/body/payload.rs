//! Boundary-result and payload transport: stores boundary results into frame slots, injects resume payloads into bindings, boxes/unboxes effect transport composite values, and encodes/decodes effect transport tag/payload pairs.

use super::*;

impl<'cg, 'a, 'ctx> CallableEmitter<'cg, 'a, 'ctx> {
    pub(super) fn frame_field_type(
        &self,
        field_index: u32,
    ) -> Result<BasicTypeEnum<'ctx>, LlvmEmitError> {
        self.frame_layout
            .llvm_ty()
            .get_field_type_at_index(field_index)
            .ok_or_else(|| frontend_error(format!("frame layout 缺少 field index {field_index}")))
    }

    pub(super) fn frame_field_ptr(
        &mut self,
        field_index: u32,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let frame_ptr = self.current_frame_ptr()?;
        self.codegen
            .builder
            .build_struct_gep(self.frame_layout.llvm_ty(), frame_ptr, field_index, name)
            .map_err(Into::into)
    }

    pub(super) fn store_boundary_result(
        &mut self,
        boundary_id: BoundaryId,
        payload: Option<BasicValueEnum<'ctx>>,
        resume_state: StateId,
    ) -> Result<(), LlvmEmitError> {
        let binding = self
            .callable
            .frame_schema()
            .resume_payload_binding(boundary_id)
            .ok_or_else(|| {
                frontend_error(format!(
                    "boundary bd{} 缺少 resumed local/home binding",
                    boundary_id.as_u32()
                ))
            })?;
        if binding.resume_state() != resume_state {
            return Err(frontend_error(format!(
                "boundary bd{} resume state 漂移：boundary=st{} binding=st{}",
                boundary_id.as_u32(),
                resume_state.as_u32(),
                binding.resume_state().as_u32()
            )));
        }
        let _ = self
            .abi
            .resume_payload_binding_layout(self.abi_step_schema, binding)?;
        self.store_payload_to_binding(binding, payload)
    }

    pub(super) fn inject_resume_payload(
        &mut self,
        binding: LateLoweredResumePayloadBinding,
        resume_tuple_ty: TypeId,
        payload: Option<BasicValueEnum<'ctx>>,
    ) -> Result<(), LlvmEmitError> {
        self.store_resume_payload_to_binding(&binding, resume_tuple_ty, payload)?;
        Ok(())
    }

    pub(super) fn store_resume_payload_to_binding(
        &mut self,
        binding: &LateLoweredResumePayloadBinding,
        resume_tuple_ty: TypeId,
        payload: Option<BasicValueEnum<'ctx>>,
    ) -> Result<(), LlvmEmitError> {
        if let Some(raw) = payload {
            if self.is_task_transport_tuple_ty(resume_tuple_ty)? {
                let resume_cg = self
                    .codegen
                    .cg_ty_of_mir_type(self.source_types, resume_tuple_ty)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "task transport resume payload type",
                        at: self.mir_fun.span.into(),
                    })?;
                let slot = self.codegen.mir_local_slot(
                    self.mir_fun.span,
                    &self.slots,
                    binding.consumer_local(),
                )?;
                if slot.cg_ty == resume_cg {
                    let value =
                        self.codegen
                            .cg_value_from_loaded(self.mir_fun.span, slot.cg_ty, raw)?;
                    self.codegen.store_local_value(
                        self.mir_fun.span,
                        slot.ptr,
                        slot.cg_ty,
                        value,
                    )?;
                } else {
                    let transport =
                        self.codegen
                            .cg_value_from_loaded(self.mir_fun.span, resume_cg, raw)?;
                    let transport = self
                        .codegen
                        .split_task_transport_tuple_value(self.mir_fun.span, transport)?;
                    let decoded = self.codegen.decode_effect_transport_value(
                        self.mir_fun.span,
                        transport.word,
                        transport.gc_ref,
                        slot.cg_ty,
                    )?;
                    self.codegen.store_local_value(
                        self.mir_fun.span,
                        slot.ptr,
                        slot.cg_ty,
                        decoded,
                    )?;
                }
            } else {
                let _ =
                    self.store_loaded_raw_local(self.mir_fun.span, binding.consumer_local(), raw)?;
            }
        }
        if let Some(frame_slot) = binding.consumer_frame_slot() {
            self.store_local_to_frame_slot(binding.consumer_local(), frame_slot)?;
        }
        Ok(())
    }

    pub(super) fn store_payload_to_binding(
        &mut self,
        binding: &LateLoweredResumePayloadBinding,
        payload: Option<BasicValueEnum<'ctx>>,
    ) -> Result<(), LlvmEmitError> {
        if let Some(raw) = payload {
            let _ =
                self.store_loaded_raw_local(self.mir_fun.span, binding.consumer_local(), raw)?;
        }
        if let Some(frame_slot) = binding.consumer_frame_slot() {
            self.store_local_to_frame_slot(binding.consumer_local(), frame_slot)?;
        }
        Ok(())
    }

    pub(super) fn effect_transport_box_layout(
        &mut self,
        source_ty: TypeId,
        cg_ty: CgTy,
    ) -> Result<(StructType<'ctx>, String), LlvmEmitError> {
        let payload_ty = self.codegen.llvm_basic_type_of(self.mir_fun.span, cg_ty)?;
        let (type_name, layout_anchor_name) = stable_naming::effect_transport_box_names(
            self.source_types,
            self.codegen.stable_type_param_resolver(),
            source_ty,
        )?;
        let struct_ty = self
            .codegen
            .context
            .get_struct_type(&type_name)
            .unwrap_or_else(|| self.codegen.context.opaque_struct_type(&type_name));
        if struct_ty.is_opaque() {
            struct_ty.set_body(
                &[self.codegen.llvm_gc_object_header_type().into(), payload_ty],
                false,
            );
        }
        Ok((struct_ty, layout_anchor_name))
    }

    pub(super) fn box_effect_transport_composite_value(
        &mut self,
        source_ty: TypeId,
        value: CgValue<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        if value.value.is_none() {
            return Err(frontend_error(format!(
                "effect transport composite t{} 缺少 runtime value",
                source_ty.as_u32()
            )));
        }
        let (box_ty, layout_anchor_name) = self.effect_transport_box_layout(source_ty, value.ty)?;
        let deferred = self
            .codegen
            .defer_gc_sensitive_cg_value(self.mir_fun.span, name, value)?;
        let box_ptr =
            self.codegen
                .alloc_gc_struct(self.mir_fun.span, box_ty, &layout_anchor_name, name)?;
        let box_root_slot = self.capture_gc_pointer_root_slot(box_ptr, &format!("{name}_root"))?;
        let box_ptr =
            self.reload_gc_pointer_from_root_slot(box_root_slot, &format!("{name}_root"))?;
        let payload_ptr = self.codegen.builder.build_struct_gep(
            box_ty,
            box_ptr,
            1,
            &format!("{name}_payload_gep"),
        )?;
        let materialized = self
            .codegen
            .materialize_deferred_cg_value(self.mir_fun.span, &format!("{name}_reload"), deferred)?
            .value
            .ok_or_else(|| {
                frontend_error(format!(
                    "effect transport composite `{name}` reload 缺少 runtime value"
                ))
            })?;
        self.codegen.store_gc_aware_value(
            self.mir_fun.span,
            payload_ptr,
            materialized,
            &format!("{name}_payload"),
        )?;
        let box_ptr =
            self.reload_gc_pointer_from_root_slot(box_root_slot, &format!("{name}_root"))?;
        self.clear_root_gc_slot(box_root_slot, &format!("{name}_root_clear"))?;
        self.codegen.cast_ptr(
            box_ptr,
            self.codegen.llvm_gc_i8_ptr_type(),
            &format!("{name}_gc_ref"),
        )
    }

    pub(super) fn load_effect_transport_composite_value(
        &mut self,
        source_ty: TypeId,
        target_cg: CgTy,
        gc_ref: PointerValue<'ctx>,
        name: &str,
    ) -> Result<BasicValueEnum<'ctx>, LlvmEmitError> {
        let (box_ty, _) = self.effect_transport_box_layout(source_ty, target_cg)?;
        let box_ptr = self.codegen.cast_ptr(
            gc_ref,
            self.codegen.llvm_ptr_type(self.codegen.gc_address_space()),
            &format!("{name}_box_ptr"),
        )?;
        let payload_ptr = self.codegen.builder.build_struct_gep(
            box_ty,
            box_ptr,
            1,
            &format!("{name}_payload_gep"),
        )?;
        Ok(self.codegen.builder.build_load(
            self.codegen
                .llvm_basic_type_of(self.mir_fun.span, target_cg)?,
            payload_ptr,
            &format!("{name}_payload"),
        )?)
    }

    pub(super) fn encode_effect_transport_parts(
        &mut self,
        source_ty: TypeId,
        payload: Option<BasicValueEnum<'ctx>>,
        name: &str,
    ) -> Result<ValueTransportParts<'ctx>, LlvmEmitError> {
        let layout = self.abi.source_value_layout(source_ty)?;
        if layout.abi().is_elided() {
            return Ok(ValueTransportParts {
                word: self.codegen.context.i64_type().const_zero(),
                gc_ref: self.codegen.llvm_gc_i8_ptr_type().const_null(),
            });
        }
        let Some(raw) = payload else {
            return Err(frontend_error(format!(
                "effect transport t{} 需要 non-elided payload",
                source_ty.as_u32()
            )));
        };
        let target_cg = self
            .codegen
            .cg_ty_of_mir_type(self.source_types, source_ty)
            .ok_or_else(|| {
                frontend_error(format!(
                    "effect transport t{} (`{}`) 缺少 codegen type",
                    source_ty.as_u32(),
                    self.source_types.display(source_ty)
                ))
            })?;
        let value = self
            .codegen
            .cg_value_from_loaded(self.mir_fun.span, target_cg, raw)?;
        match target_cg {
            CgTy::Unit | CgTy::Never => Ok(ValueTransportParts {
                word: self.codegen.context.i64_type().const_zero(),
                gc_ref: self.codegen.llvm_gc_i8_ptr_type().const_null(),
            }),
            CgTy::Bool | CgTy::Float32 | CgTy::Float64 | CgTy::Int(_) => {
                let word = self.codegen.coerce_u64_word(self.mir_fun.span, value)?;
                Ok(ValueTransportParts {
                    word,
                    gc_ref: self.codegen.llvm_gc_i8_ptr_type().const_null(),
                })
            }
            CgTy::Ref => Ok(ValueTransportParts {
                word: self.codegen.context.i64_type().const_zero(),
                gc_ref: value
                    .value
                    .ok_or_else(|| frontend_error("effect transport ref 缺少值".to_string()))?
                    .into_pointer_value(),
            }),
            CgTy::String => Ok(ValueTransportParts {
                word: self.codegen.context.i64_type().const_zero(),
                gc_ref: self.codegen.builder.build_pointer_cast(
                    value
                        .value
                        .ok_or_else(|| {
                            frontend_error("effect transport string 缺少值".to_string())
                        })?
                        .into_pointer_value(),
                    self.codegen.llvm_gc_i8_ptr_type(),
                    &format!("{name}_string_ref"),
                )?,
            }),
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => Ok(ValueTransportParts {
                word: self.codegen.context.i64_type().const_zero(),
                gc_ref: self.box_effect_transport_composite_value(source_ty, value, name)?,
            }),
        }
    }

    pub(super) fn decode_effect_transport_parts(
        &mut self,
        source_ty: TypeId,
        transport: ValueTransportParts<'ctx>,
        name: &str,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        let layout = self.abi.source_value_layout(source_ty)?;
        if layout.abi().is_elided() {
            return Ok(None);
        }
        let target_cg = self
            .codegen
            .cg_ty_of_mir_type(self.source_types, source_ty)
            .ok_or_else(|| {
                frontend_error(format!(
                    "effect transport t{} (`{}`) 缺少 codegen type",
                    source_ty.as_u32(),
                    self.source_types.display(source_ty)
                ))
            })?;
        match target_cg {
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => self
                .load_effect_transport_composite_value(source_ty, target_cg, transport.gc_ref, name)
                .map(Some),
            _ => Ok(self
                .codegen
                .decode_effect_transport_value(
                    self.mir_fun.span,
                    transport.word,
                    transport.gc_ref,
                    target_cg,
                )?
                .value),
        }
    }

    pub(super) fn lower_completion_payload(
        &mut self,
        payload_source: &LateLoweredCompletionPayloadSource,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        match payload_source {
            LateLoweredCompletionPayloadSource::Unit { .. } => Ok(None),
            LateLoweredCompletionPayloadSource::Operand(source) => {
                if self.source_ty_is_unit(source.source_ty()) {
                    return Ok(None);
                }
                let value = self.lower_operand_source(source)?;
                if value.value.is_none() {
                    return Err(frontend_error(format!(
                        "completion payload source {:?} lowered to no runtime value",
                        source
                    )));
                }
                Ok(value.value)
            }
        }
    }

    pub(super) fn lower_completion_payload_as(
        &mut self,
        payload_source: &LateLoweredCompletionPayloadSource,
        target_ty: TypeId,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        let expected = self
            .codegen
            .cg_ty_of_mir_type(self.source_types, target_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "completion payload target type",
                at: self.mir_fun.span.into(),
            })?;
        if expected == CgTy::Unit {
            return Ok(None);
        }
        match payload_source {
            LateLoweredCompletionPayloadSource::Unit { .. } => Ok(None),
            LateLoweredCompletionPayloadSource::Operand(source) => {
                let value = self.lower_operand_source(source)?;
                let value = self.codegen.coerce_value(
                    source.span().unwrap_or(self.mir_fun.span),
                    value,
                    expected,
                )?;
                if value.value.is_none() {
                    return Err(frontend_error(format!(
                        "completion payload source {:?} coerced to no runtime value",
                        source
                    )));
                }
                Ok(value.value)
            }
        }
    }
}
