//! MIR call argument binding helpers (class-ctor, bound MIR args).

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn codegen_mir_class_ctor_ordered_args(
        &mut self,
        _span: crate::span::Span,
        args: &[crate::mir::CallArg],
        slots: &[MirLocalSlot<'ctx>],
        ctor_params: &[hir::ClassCtorParam],
        kind: &'static str,
    ) -> Result<Vec<CgValue<'ctx>>, LlvmEmitError> {
        if ctor_params.len() != args.len() {
            panic!("codegen_mir_class_ctor_ordered_args: MIR verifier accepted {kind}");
        }

        let mut evaluated_args = Vec::with_capacity(args.len());
        for (idx, (param, arg)) in ctor_params.iter().zip(args).enumerate() {
            if arg.name.is_some() {
                panic!("codegen_mir_class_ctor_ordered_args: MIR verifier accepted {kind}");
            }
            let param_cg = self
                .cg_ty_of(param.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "class ctor param type",
                    at: arg.span.into(),
                })?;
            let value =
                self.codegen_mir_operand_expected(arg.span, &arg.value, slots, Some(param_cg))?;
            let value = self.coerce_value(arg.span, value, param_cg)?;
            let deferred = self.defer_gc_sensitive_cg_value(
                arg.span,
                &format!("class_ctor_ordered_arg_{idx}"),
                value,
            )?;
            evaluated_args.push(self.materialize_deferred_cg_value(
                arg.span,
                &format!("class_ctor_ordered_arg_reload_{idx}"),
                deferred,
            )?);
        }

        Ok(evaluated_args)
    }

    pub(in crate::llvm::codegen) fn codegen_mir_class_ctor_call(
        &mut self,
        span: crate::span::Span,
        class_layout_key: &str,
        ctor: &crate::mir::ClassCtorCallMetadata,
        args: &[crate::mir::CallArg],
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let class = self.class_init_layout(span, class_layout_key)?;
        let selected_ctor = self.selected_mir_class_ctor_from_contract(
            span,
            &class,
            ctor,
            args,
            "class ctor selected/ordered args contract",
        )?;
        let ctor_params: &[hir::ClassCtorParam] = match selected_ctor {
            Some(ctor) => ctor.params.as_slice(),
            None => &[][..],
        };

        let obj_ty = self.llvm_class_object_type(span, &class)?;
        let obj_size_bytes = self.target_data.get_store_size(&obj_ty);
        let size_v = self.context.i64_type().const_int(obj_size_bytes, false);
        let type_desc = self.get_or_create_class_type_desc_global(span, class_layout_key)?;
        let type_desc_i8 = self.builder.build_pointer_cast(
            type_desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "lowered_class_type_desc_i8",
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call = self.build_call_preserving_gc_local_roots(
            span,
            rt_alloc,
            &[type_desc_i8.into(), size_v.into()],
            "rt_alloc_lowered_class",
        )?;
        let raw = self.expect_basic_value(call, "scoop_alloc_typed lowered class allocation");
        let obj_ptr = self.expect_pointer_value(raw, "scoop_alloc_typed lowered class allocation");

        let obj_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let typed_obj =
            self.builder
                .build_pointer_cast(obj_ptr, obj_ptr_ty, "lowered_class_obj_ptr")?;
        let payload_ptr =
            self.builder
                .build_struct_gep(obj_ty, typed_obj, 1, "lowered_class_payload_gep")?;
        let payload_ty = self.llvm_class_payload_type(span, &class)?;
        let payload_size_bytes = self.target_data.get_store_size(&payload_ty);
        if payload_size_bytes > 0 {
            let payload_i8 = self
                .builder
                .build_bit_cast(
                    payload_ptr,
                    self.llvm_gc_i8_ptr_type(),
                    "lowered_class_payload_i8",
                )?
                .into_pointer_value();
            let size_ty = self.llvm_ptr_sized_int_type(None);
            let size_v = size_ty.const_int(payload_size_bytes, false);
            let zero = self.context.i8_type().const_int(0, false);
            let _ = self.builder.build_memset(payload_i8, 1, zero, size_v)?;
        }

        let deferred_obj = self.defer_gc_sensitive_cg_value(
            span,
            "lowered_class_ctor_obj_root",
            CgValue {
                ty: CgTy::Ref,
                value: Some(obj_ptr.into()),
            },
        )?;

        let evaluated_args = self.codegen_mir_class_ctor_ordered_args(
            span,
            args,
            slots,
            ctor_params,
            "class ctor ordered arg eval",
        )?;

        let current_obj = self.reload_deferred_gc_ref_without_clearing(
            span,
            "lowered_class_ctor_obj_before_invoke",
            &deferred_obj,
        )?;

        self.codegen_class_ctor_invoke(
            span,
            span,
            &class,
            selected_ctor,
            evaluated_args.as_slice(),
            current_obj,
        )?;
        self.emit_ordinary_call_effect_propagation_check(span, "lowered_class_ctor_call_effect")?;

        if !self.ordinary_effect_propagation_enabled()
            && let Some(outcome_ptr) = self.function_cx.current_effect_outcome_ptr
        {
            let current_fn = self.expect_current_function("lowered class ctor effect split");
            let active_bb = self
                .context
                .append_basic_block(current_fn, "lowered_class_ctor_active");
            let inactive_bb = self
                .context
                .append_basic_block(current_fn, "lowered_class_ctor_inactive");
            let merge_bb = self
                .context
                .append_basic_block(current_fn, "lowered_class_ctor_merge");
            let is_propagating =
                self.effect_outcome_is_propagating(span, outcome_ptr, "lowered_class_ctor_effect")?;
            self.builder
                .build_conditional_branch(is_propagating, active_bb, inactive_bb)?;

            self.builder.position_at_end(active_bb);
            self.clear_deferred_cg_value_root_homes(
                span,
                "lowered_class_ctor_obj_active_drop",
                &deferred_obj,
            )?;
            let active_bb_end =
                self.builder
                    .get_insert_block()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "class ctor active block",
                        at: span.into(),
                    })?;
            self.builder.build_unconditional_branch(merge_bb)?;

            self.builder.position_at_end(inactive_bb);
            let current_obj = self.reload_deferred_gc_ref_without_clearing(
                span,
                "lowered_class_ctor_obj_return",
                &deferred_obj,
            )?;
            let inactive_bb_end =
                self.builder
                    .get_insert_block()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "class ctor inactive block",
                        at: span.into(),
                    })?;
            self.builder.build_unconditional_branch(merge_bb)?;

            self.builder.position_at_end(merge_bb);
            let result_phi = self
                .builder
                .build_phi(self.llvm_gc_i8_ptr_type(), "lowered_class_ctor_result")?;
            result_phi.add_incoming(&[
                (&self.llvm_gc_i8_ptr_type().const_null(), active_bb_end),
                (&current_obj, inactive_bb_end),
            ]);
            return Ok(CgValue {
                ty: CgTy::Ref,
                value: Some(result_phi.as_basic_value()),
            });
        }

        let current_obj = self.reload_deferred_gc_ref_without_clearing(
            span,
            "lowered_class_ctor_obj_return",
            &deferred_obj,
        )?;

        Ok(CgValue {
            ty: CgTy::Ref,
            value: Some(current_obj.into()),
        })
    }

    pub(in crate::llvm::codegen) fn codegen_bound_mir_call_args(
        &mut self,
        span: crate::span::Span,
        sig_fun: &hir::FunDecl,
        args: &[crate::mir::CallArg],
        slots: &[MirLocalSlot<'ctx>],
        uses_native_abi: bool,
    ) -> Result<Vec<EvaluatedCallArg<'ctx>>, LlvmEmitError> {
        let param_names = sig_fun
            .params
            .iter()
            .map(|param| param.name.clone())
            .collect::<Vec<_>>();
        let param_tys = sig_fun
            .params
            .iter()
            .map(|param| param.ty)
            .collect::<Vec<_>>();
        self.codegen_bound_mir_call_args_from_signature(
            span,
            &param_names,
            &param_tys,
            args,
            slots,
            uses_native_abi,
            self.types,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_bound_mir_call_args_from_signature(
        &mut self,
        span: crate::span::Span,
        param_names: &[String],
        param_tys: &[TypeId],
        args: &[crate::mir::CallArg],
        slots: &[MirLocalSlot<'ctx>],
        uses_native_abi: bool,
        source_types: &TypeStore,
    ) -> Result<Vec<EvaluatedCallArg<'ctx>>, LlvmEmitError> {
        let arg_to_param = map_mir_call_args_to_param_names(param_names, args).unwrap_or_else(|| {
            panic!("codegen_bound_mir_call_args_from_signature: MIR call ABI verifier accepted arg binding drift")
        });

        let mut evaluated: Vec<Option<(crate::span::Span, DeferredCgValue<'ctx>)>> =
            vec![None; param_tys.len()];
        for (arg_idx, arg) in args.iter().enumerate() {
            let param_idx = arg_to_param[arg_idx];
            let param_ty = param_tys[param_idx];
            let target_cg = self
                .cg_ty_of_mir_type(source_types, param_ty)
                .or_else(|| {
                    self.equivalent_codegen_type_id(source_types, param_ty)
                        .and_then(|ty| self.cg_ty_of(ty))
                })
                .or_else(|| self.cg_ty_of(param_ty))
                .unwrap_or_else(|| {
                    panic!("codegen_bound_mir_call_args_from_signature: TypeStore equivalence verifier accepted unsupported call arg type")
                });
            let value =
                self.codegen_mir_operand_expected(arg.span, &arg.value, slots, Some(target_cg))?;
            let coerced = self.coerce_value(arg.span, value, target_cg)?;
            let deferred = self.defer_gc_sensitive_cg_value(
                arg.span,
                &format!("pass_mir_call_arg_{param_idx}"),
                coerced,
            )?;
            evaluated[param_idx] = Some((arg.span, deferred));
        }

        evaluated
            .into_iter()
            .enumerate()
            .map(|(param_idx, slot)| {
                let (arg_span, deferred) = slot.unwrap_or_else(|| {
                    panic!("codegen_bound_mir_call_args_from_signature: MIR call ABI verifier accepted missing evaluated arg slot")
                });
                let param_ty = param_tys[param_idx];
                let abi_ty = self
                    .equivalent_codegen_type_id(source_types, param_ty)
                    .unwrap_or(param_ty);
                let param_abi = if uses_native_abi {
                    None
                } else {
                    Some(self.ordinary_param_abi(span, abi_ty)?)
                };
                if let Some(abi) = param_abi
                    && abi.pointee_ty().is_some()
                {
                    let (slot_ptr, cleanup_spills) = self.deferred_gc_spill_slot_for_call_arg(
                        arg_span,
                        &format!("pass_mir_call_arg_reload_{param_idx}"),
                        deferred,
                    )?;
                    return Ok(EvaluatedCallArg {
                        value: slot_ptr.into(),
                        pointer_value: None,
                        cleanup_spills,
                    });
                }

                let (materialized, cleanup_spills) = self
                    .materialize_deferred_cg_value_for_call_arg(
                        arg_span,
                        &format!("pass_mir_call_arg_reload_{param_idx}"),
                        deferred,
                    )?;
                let pointer_value = match materialized.value {
                    Some(inkwell::values::BasicValueEnum::PointerValue(ptr)) => Some(ptr),
                    _ => None,
                };
                let param_cg = param_abi
                    .map(OrdinaryParamAbi::cg_ty)
                    .unwrap_or(materialized.ty);
                let value = self.as_llvm_arg_value(arg_span, param_cg, materialized)?;
                Ok(EvaluatedCallArg {
                    value,
                    pointer_value,
                    cleanup_spills,
                })
            })
            .collect()
    }

    pub(in crate::llvm::codegen) fn codegen_bound_materialized_mir_call_args(
        &mut self,
        _span: crate::span::Span,
        mir_fun: &crate::mir::FunDecl,
        mir_types: &TypeStore,
        args: &[crate::mir::CallArg],
        slots: &[MirLocalSlot<'ctx>],
        uses_native_abi: bool,
    ) -> Result<Vec<EvaluatedCallArg<'ctx>>, LlvmEmitError> {
        let arg_to_param = map_mir_call_args_to_mir_params(&mir_fun.params, args).unwrap_or_else(|| {
            panic!("codegen_bound_materialized_mir_call_args: MIR call ABI verifier accepted arg binding drift")
        });

        let mut evaluated: Vec<Option<(crate::span::Span, DeferredCgValue<'ctx>)>> =
            vec![None; mir_fun.params.len()];
        for (arg_idx, arg) in args.iter().enumerate() {
            let param_idx = arg_to_param[arg_idx];
            let param = &mir_fun.params[param_idx];
            let target_cg = self.cg_ty_of_mir_type(mir_types, param.ty).unwrap_or_else(|| {
                panic!("codegen_bound_materialized_mir_call_args: TypeStore equivalence verifier accepted unsupported call arg type")
            });
            let value =
                self.codegen_mir_operand_expected(arg.span, &arg.value, slots, Some(target_cg))?;
            let coerced = self.coerce_value(arg.span, value, target_cg)?;
            let deferred = self.defer_gc_sensitive_cg_value(
                arg.span,
                &format!("pass_mir_call_arg_{param_idx}"),
                coerced,
            )?;
            evaluated[param_idx] = Some((arg.span, deferred));
        }

        evaluated
            .into_iter()
            .enumerate()
            .map(|(param_idx, slot)| {
                let (arg_span, deferred) = slot.unwrap_or_else(|| {
                    panic!("codegen_bound_materialized_mir_call_args: MIR call ABI verifier accepted missing evaluated arg slot")
                });
                let param = &mir_fun.params[param_idx];
                let abi_ty = self.equivalent_codegen_type_id(mir_types, param.ty).unwrap_or_else(|| {
                    panic!(
                        "codegen_bound_materialized_mir_call_args: MIR verifier accepted unsupported plain param type"
                    )
                });
                let param_abi = if uses_native_abi {
                    None
                } else {
                    Some(self.ordinary_param_abi(param.span, abi_ty)?)
                };
                if let Some(abi) = param_abi
                    && abi.pointee_ty().is_some()
                {
                    let (slot_ptr, cleanup_spills) = self.deferred_gc_spill_slot_for_call_arg(
                        arg_span,
                        &format!("pass_mir_call_arg_reload_{param_idx}"),
                        deferred,
                    )?;
                    return Ok(EvaluatedCallArg {
                        value: slot_ptr.into(),
                        pointer_value: None,
                        cleanup_spills,
                    });
                }

                let (materialized, cleanup_spills) = self
                    .materialize_deferred_cg_value_for_call_arg(
                        arg_span,
                        &format!("pass_mir_call_arg_reload_{param_idx}"),
                        deferred,
                    )?;
                let pointer_value = match materialized.value {
                    Some(inkwell::values::BasicValueEnum::PointerValue(ptr)) => Some(ptr),
                    _ => None,
                };
                let param_cg = param_abi
                    .map(OrdinaryParamAbi::cg_ty)
                    .unwrap_or(materialized.ty);
                let value = self.as_llvm_arg_value(arg_span, param_cg, materialized)?;
                Ok(EvaluatedCallArg {
                    value,
                    pointer_value,
                    cleanup_spills,
                })
            })
            .collect()
    }
}
