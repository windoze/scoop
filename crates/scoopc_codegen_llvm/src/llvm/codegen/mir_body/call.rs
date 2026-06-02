//! MIR call lowering: direct, class-ctor, closure, fun-value, funptr-value, plain-dynamic.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_mir_call(
        &mut self,
        span: crate::span::Span,
        site_id: crate::mir::SiteId,
        kind: &crate::mir::CallKind,
        args: &[crate::mir::CallArg],
        transport: &crate::mir::CallTransportMetadata,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match kind {
            crate::mir::CallKind::Direct {
                callee_fqn,
                generic_type_args,
                ..
            } => {
                if let Some(class_key) = self.registered_class_instance_key(callee_fqn) {
                    return self.codegen_mir_class_ctor_call_at_site(span, &class_key, args, slots);
                }
                self.codegen_mir_direct_call_with_type_args(
                    span,
                    site_id,
                    callee_fqn,
                    generic_type_args,
                    args,
                    body,
                    mir_types,
                    transport,
                    slots,
                )
            }
            crate::mir::CallKind::Closure { callee, fn_ptr } => {
                let fun_ty = self
                    .mir_operand_function_type(body, mir_types, callee)
                    .unwrap_or_else(|| {
                        panic!("codegen_mir_call: MIR call ABI verifier accepted non-function closure callee")
                    });
                self.codegen_mir_closure_call(span, callee, fn_ptr, args, &fun_ty, slots)
            }
            crate::mir::CallKind::FunValue { callee } => {
                let fun_ty = self
                    .mir_operand_function_type(body, mir_types, callee)
                    .unwrap_or_else(|| {
                        panic!("codegen_mir_call: MIR call ABI verifier accepted non-function function-value callee")
                    });
                self.codegen_mir_fun_value_call(span, callee, args, &fun_ty, slots)
            }
            crate::mir::CallKind::FunPtr { callee } => {
                let fun_ty = self
                    .mir_operand_funptr_function_type(body, mir_types, callee)
                    .unwrap_or_else(|| {
                        panic!(
                            "codegen_mir_call: materialized MIR verifier accepted non-FunPtr callee type"
                        )
                    });
                self.codegen_mir_funptr_value_call(
                    span,
                    callee,
                    args,
                    &fun_ty,
                    (body, mir_types, slots),
                )
            }
            crate::mir::CallKind::Virtual { .. }
            | crate::mir::CallKind::Interface { .. }
            | crate::mir::CallKind::Resume { .. } => Err(raw_mir_route_gate_error(
                self.function_cx
                    .current_callable_fqn
                    .as_deref()
                    .unwrap_or("<unknown raw mir body>"),
                span,
                "PIPELINE_GAPS §3.6",
                RAW_MIR_CALL_KIND_DETAIL,
            )),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_mir_direct_call_with_policy(
        &mut self,
        span: crate::span::Span,
        site_id: Option<crate::mir::SiteId>,
        fqn: &str,
        generic_type_args: &[TypeId],
        args: &[crate::mir::CallArg],
        transport: &crate::mir::CallTransportMetadata,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        require_plain_surface: bool,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let source_site = site_id.and_then(|site_id| self.published_lir_source_call_site(site_id));
        if source_site.is_none() && site_id.is_some() && !generic_type_args.is_empty() {
            return Err(frontend_error(format!(
                "generic direct call `{fqn}` lacks published LIR source call-site contract"
            )));
        }
        let concrete_fqn = source_site
            .and_then(|site| site.contract.exact_callee.as_ref())
            .map(|exact| exact.root_fqn.clone())
            .or_else(|| instantiated_mir_callee_fqn(fqn, generic_type_args, mir_types))
            .unwrap_or_else(|| fqn.to_string());
        let concrete_fqn = concrete_fqn.as_str();
        let semantic_root = source_site.and_then(|site| site.semantic_root_fqn.as_deref());
        let named_intrinsic_entry = if let Some(entry_name) = source_site
            .and_then(|site| site.named_entry_name.as_deref())
            .map(str::to_string)
        {
            Some(entry_name)
        } else {
            self.published_named_intrinsic_entry_name_for_root(
                semantic_root.unwrap_or(concrete_fqn),
            )?
            .map(str::to_string)
        };
        if let Some(entry_name) = named_intrinsic_entry
            && let Some(value) = self.try_codegen_named_intrinsic_mir_direct_call(
                span,
                &entry_name,
                args,
                body,
                mir_types,
                transport.array.as_ref(),
                slots,
            )?
        {
            return Ok(value);
        }
        let callable_abi = self.direct_call_abi_identity(concrete_fqn);
        let uses_effect_step_surface = callable_abi.uses_effect_bridge_abi();
        if require_plain_surface && uses_effect_step_surface {
            return Err(frontend_error(format!(
                "plain direct call `{}` 仍要求 effect-step callable surface；应走 published boundary/dynamic adapter，而不是 ordinary direct call",
                concrete_fqn,
            )));
        }
        let uses_explicit_effect_hidden_abi = !require_plain_surface && uses_effect_step_surface;
        let (source_types, param_names, source_param_tys, source_return_ty) = self
            .published_callable_signature_with_names(concrete_fqn)
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!("direct call `{concrete_fqn}` 缺少 LIR callable signature facts"),
            })?;
        let (param_tys, return_ty_for_codegen) = self
            .published_signature_tys_as_codegen_tys(
                source_types,
                source_param_tys.clone(),
                source_return_ty,
            )
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "direct call `{concrete_fqn}` 的 LIR callable signature 无法映射到 LLVM codegen TypeStore"
                ),
            })?;
        if param_names.len() != param_tys.len() {
            panic!(
                "codegen_mir_direct_call_with_policy: MIR call ABI verifier accepted signature arity mismatch"
            );
        }
        if args.len() != param_tys.len() {
            panic!(
                "codegen_mir_direct_call_with_policy: MIR call ABI verifier accepted direct call arity mismatch"
            );
        }

        let native_abi = if callable_abi.uses_native_abi() {
            Some(self.classify_direct_extern_native_callable(
                span,
                concrete_fqn,
                &param_tys,
                return_ty_for_codegen,
            )?)
        } else {
            None
        };

        let ret_cg = native_abi
            .as_ref()
            .map(|abi| abi.return_abi.cg_ty)
            .or_else(|| self.cg_ty_of_mir_type(source_types, source_return_ty))
            .or_else(|| self.try_cg_ty_of_type_id(return_ty_for_codegen))
            .or_else(|| self.cg_ty_of_mir_type(mir_types, transport.result.source_ty))
            .or_else(|| {
                self.equivalent_codegen_type_id(mir_types, transport.result.source_ty)
                    .and_then(|ty| self.try_cg_ty_of_type_id(ty))
            })
            .unwrap_or_else(|| {
                panic!("codegen_mir_direct_call_with_policy: MIR call ABI verifier accepted unsupported direct call return type")
            });
        let hidden_sret_result_ty = if native_abi.is_some() {
            None
        } else {
            self.hidden_sret_result_ty(span, ret_cg)?
        };
        let evaluated_args = self.codegen_bound_mir_call_args_from_signature(
            span,
            &param_names,
            &param_tys,
            args,
            slots,
            native_abi.is_some(),
            self.types,
        )?;

        let mut llvm_args: Vec<BasicMetadataValueEnum<'ctx>> = Vec::with_capacity(
            evaluated_args.len()
                + usize::from(hidden_sret_result_ty.is_some())
                + self.explicit_effect_hidden_abi_param_count(uses_explicit_effect_hidden_abi)
                    as usize,
        );
        let sret_result_slot = if hidden_sret_result_ty.is_some() {
            let slot = self.create_entry_alloca(span, "pass_mir_direct_call_sret", ret_cg)?;
            llvm_args.push(slot.into());
            Some(slot)
        } else {
            None
        };
        let effect_outcome_slot = if uses_explicit_effect_hidden_abi {
            let slot = self.alloc_effect_outcome_slot(span, "pass_mir_direct_call")?;
            llvm_args.push(self.current_effect_ctx_arg().into());
            llvm_args.push(self.llvm_gc_i8_ptr_type().const_null().into());
            llvm_args.push(slot.into());
            Some(slot)
        } else {
            None
        };
        llvm_args.extend(evaluated_args.iter().map(|arg| arg.value));

        let llvm_name = self
            .extern_funs
            .get(concrete_fqn)
            .map(|extern_fun| extern_fun.symbol.as_str())
            .unwrap_or(concrete_fqn);
        let llvm_fun = match self.module.get_function(llvm_name) {
            Some(function) => function,
            None => {
                let declaration_surface = if callable_abi.is_extern() {
                    LlvmFunctionDeclarationSurface::RuntimeOrNativeImport
                } else {
                    LlvmFunctionDeclarationSurface::ExportedAbi
                };
                self.declare_lir_plain_fun_with_symbol(
                    llvm_name,
                    declaration_surface,
                    concrete_fqn,
                    &source_param_tys,
                    source_return_ty,
                    source_types,
                    false,
                )?
            }
        };
        let call_site_result = if let Some(native_abi) = native_abi.as_ref() {
            self.emit_native_callable_call(
                span,
                native_abi,
                NativeCallableTarget::Direct(llvm_fun),
                &llvm_args,
            )
        } else {
            self.with_conservative_gc_local_root_spills(span, |cg| {
                let call_site =
                    cg.builder
                        .build_call(llvm_fun, &llvm_args, "pass_mir_direct_call")?;
                if let Some(result_ty) = hidden_sret_result_ty {
                    cg.add_sret_attribute_to_call(call_site, 0, result_ty);
                }
                call_site.set_call_convention(cg.llvm_call_convention_for_fqn(concrete_fqn));
                Ok(call_site)
            })
        };
        self.release_evaluated_call_arg_roots(&evaluated_args);
        let call_site = call_site_result?;
        if let Some(result_ptr) = sret_result_slot {
            self.sync_hidden_sret_result_roots(
                span,
                ret_cg,
                result_ptr,
                "pass_mir_direct_call_sret",
            )?;
        }
        let deferred_direct_result = if sret_result_slot.is_none() {
            self.defer_direct_call_result(span, ret_cg, call_site, "pass_mir_direct_call_result")?
        } else {
            None
        };
        if let Some(outcome_slot) = effect_outcome_slot {
            self.maybe_record_active_suspend_site_effect_outcome(span, outcome_slot);
            self.emit_ordinary_call_effect_propagation_check_from_outcome(
                span,
                outcome_slot,
                "pass_mir_direct_call_effect",
            )?;
        }

        match ret_cg {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            _ => {
                if let Some(result_ptr) = sret_result_slot {
                    self.load_hidden_sret_result_from_ptr(
                        span,
                        ret_cg,
                        result_ptr,
                        "pass_mir_direct_call_sret",
                    )
                } else {
                    self.materialize_deferred_cg_value(
                        span,
                        "pass_mir_direct_call_result_reload",
                        deferred_direct_result.unwrap_or_else(|| {
                            panic!("codegen_mir_direct_call_with_policy: MIR call ABI verifier accepted missing deferred return value")
                        }),
                    )
                }
            }
        }
    }

    pub(in crate::llvm::codegen) fn codegen_mir_class_ctor_call_at_site(
        &mut self,
        span: crate::span::Span,
        class_layout_key: &hir::ClassInstanceKey,
        args: &[crate::mir::CallArg],
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let site = self
            .ctor_call_sites
            .get(&self.current_call_site(span)?)
            .unwrap_or_else(|| panic!("codegen_mir_class_ctor_call_at_site: verifier accepted missing class ctor call site"));
        self.codegen_mir_class_ctor_call(
            span,
            class_layout_key,
            &crate::mir::ClassCtorCallMetadata {
                selected_ctor_span: site.ctor_span,
                ordered_param_count: site.arg_mapping.len(),
            },
            args,
            slots,
        )
    }

    pub(in crate::llvm::codegen) fn codegen_mir_closure_call(
        &mut self,
        span: crate::span::Span,
        callee: &crate::mir::Operand,
        fn_ptr: &str,
        args: &[crate::mir::CallArg],
        fun_ty: &crate::ty::FunctionType,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let call_may_suspend = self
            .published_late_lowered_program()
            .and_then(|program| program.callable(fn_ptr))
            .map(|callable| callable.effect_step_abi().is_some())
            .unwrap_or_else(|| {
                self.managed_callable_abi_identity_from_fun_ty(fun_ty)
                    .uses_effect_bridge_abi()
            });
        let callee_value =
            self.codegen_mir_operand_expected(span, callee, slots, Some(CgTy::Ref))?;
        let callee_value = self.coerce_value(span, callee_value, CgTy::Ref)?;
        let Some(BasicValueEnum::PointerValue(closure_obj_i8)) = callee_value.value else {
            panic!(
                "codegen_mir_closure_call: MIR call ABI verifier accepted non-pointer closure callee"
            )
        };
        self.codegen_mir_function_value_call_from_closure_obj(
            span,
            closure_obj_i8,
            fun_ty,
            call_may_suspend,
            args,
            slots,
        )
    }

    pub(in crate::llvm::codegen) fn codegen_mir_fun_value_call(
        &mut self,
        span: crate::span::Span,
        callee: &crate::mir::Operand,
        args: &[crate::mir::CallArg],
        fun_ty: &crate::ty::FunctionType,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let callee_value =
            self.codegen_mir_operand_expected(span, callee, slots, Some(CgTy::Ref))?;
        let callee_value = self.coerce_value(span, callee_value, CgTy::Ref)?;
        let Some(BasicValueEnum::PointerValue(closure_obj_i8)) = callee_value.value else {
            panic!(
                "codegen_mir_fun_value_call: MIR call ABI verifier accepted non-pointer function-value callee"
            );
        };
        self.codegen_mir_function_value_call_from_closure_obj(
            span,
            closure_obj_i8,
            fun_ty,
            self.managed_callable_abi_identity_from_fun_ty(fun_ty)
                .uses_effect_bridge_abi(),
            args,
            slots,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_mir_funptr_value_call(
        &mut self,
        span: crate::span::Span,
        callee: &crate::mir::Operand,
        args: &[crate::mir::CallArg],
        fun_ty: &crate::ty::FunctionType,
        mir_ctx: (&crate::mir::Body, &TypeStore, &[MirLocalSlot<'ctx>]),
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let (_body, mir_types, slots) = mir_ctx;
        let callee_value = self.codegen_mir_operand(span, callee, slots)?;
        let (funptr_addr, funptr_int_ty) = callee_value.as_int().unwrap_or_else(|| {
            panic!(
                "codegen_mir_funptr_value_call: materialized MIR verifier accepted non-int FunPtr callee value"
            )
        });
        let expected_arity = fun_ty.params.len() + usize::from(fun_ty.receiver.is_some());
        if args.len() != expected_arity {
            panic!(
                "codegen_mir_funptr_value_call: materialized MIR verifier accepted arity mismatch"
            );
        }

        let param_tys = self.callable_value_param_tys(fun_ty);
        let native_abi =
            self.classify_funptr_native_callable(span, &param_tys, fun_ty.return_ty)?;
        let ret_cg = native_abi.return_abi.cg_ty;

        let fun_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let casted_addr = if funptr_int_ty.bits == self.host.word_bit_width() {
            funptr_addr
        } else {
            self.cast_int(
                funptr_addr,
                funptr_int_ty,
                IntTy {
                    bits: self.host.word_bit_width(),
                    signed: false,
                },
            )?
        };
        let typed_fn_ptr =
            self.builder
                .build_int_to_ptr(casted_addr, fun_ptr_ty, "pass_mir_funptr_typed")?;

        let mut llvm_args: Vec<BasicMetadataValueEnum<'ctx>> = Vec::with_capacity(args.len());
        let evaluated_args =
            self.codegen_mir_funptr_value_args(span, fun_ty, args, mir_types, slots)?;
        for arg in &evaluated_args {
            llvm_args.push(arg.value);
        }

        let call_site_result = self.emit_native_callable_call(
            span,
            &native_abi,
            NativeCallableTarget::Indirect {
                fn_ty: native_abi.fn_ty,
                ptr: typed_fn_ptr,
                call_name: "pass_mir_call_funptr",
            },
            &llvm_args,
        );
        self.release_evaluated_call_arg_roots(&evaluated_args);
        let call_site = call_site_result?;
        let deferred_direct_result =
            self.defer_direct_call_result(span, ret_cg, call_site, "pass_mir_funptr_call_result")?;

        match ret_cg {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            _ => self.materialize_deferred_cg_value(
                span,
                "pass_mir_funptr_call_result_reload",
                deferred_direct_result.unwrap_or_else(|| {
                    panic!(
                        "codegen_mir_funptr_value_call: materialized MIR verifier accepted missing deferred return value"
                    )
                }),
            ),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_mir_function_value_call_from_closure_obj(
        &mut self,
        span: crate::span::Span,
        closure_obj_i8: PointerValue<'ctx>,
        fun_ty: &crate::ty::FunctionType,
        call_may_suspend: bool,
        args: &[crate::mir::CallArg],
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let expected_arity = fun_ty.params.len() + usize::from(fun_ty.receiver.is_some());
        if args.len() != expected_arity {
            panic!(
                "codegen_mir_function_value_call_from_closure_obj: MIR call ABI verifier accepted function-value arity mismatch"
            );
        }
        let deferred_closure =
            self.defer_gc_ref_pointer(span, "pass_mir_function_value_closure", closure_obj_i8)?;

        let closure_ty = self.llvm_closure_object_type();
        let closure_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let ret_cg = self
            .try_cg_ty_of_type_id(fun_ty.return_ty)
            .unwrap_or_else(|| {
                panic!("codegen_mir_function_value_call_from_closure_obj: MIR call ABI verifier accepted unsupported function-value return type")
            });
        let hidden_sret_result_ty = self.hidden_sret_result_ty(span, ret_cg)?;

        let mut llvm_param_tys: Vec<BasicMetadataTypeEnum<'ctx>> = Vec::with_capacity(
            1 + expected_arity
                + usize::from(hidden_sret_result_ty.is_some())
                + self.explicit_effect_hidden_abi_param_count(call_may_suspend) as usize,
        );
        if let Some(result_ty) = hidden_sret_result_ty {
            let _ = result_ty;
            llvm_param_tys.push(self.context.ptr_type(AddressSpace::default()).into());
        }
        if call_may_suspend {
            self.push_explicit_effect_hidden_abi_param_tys(&mut llvm_param_tys);
        }
        llvm_param_tys.push(gc_i8_ptr_ty.into());
        if let Some(receiver_ty) = fun_ty.receiver {
            llvm_param_tys.push(self.ordinary_param_abi(span, receiver_ty)?.llvm_param_ty());
        }
        for ty in &fun_ty.params {
            llvm_param_tys.push(self.ordinary_param_abi(span, *ty)?.llvm_param_ty());
        }
        let llvm_fun_ty = match (hidden_sret_result_ty, ret_cg) {
            (Some(_), _) | (None, CgTy::Unit | CgTy::Never) => {
                self.context.void_type().fn_type(&llvm_param_tys, false)
            }
            (None, CgTy::Bool) => self.context.bool_type().fn_type(&llvm_param_tys, false),
            (None, CgTy::Float64) => self.context.f64_type().fn_type(&llvm_param_tys, false),
            (None, CgTy::Float32) => self.context.f32_type().fn_type(&llvm_param_tys, false),
            (None, CgTy::Int(int_ty)) => self.int_type(int_ty).fn_type(&llvm_param_tys, false),
            (None, CgTy::String) => self
                .llvm_scoop_string_ptr_type()
                .fn_type(&llvm_param_tys, false),
            (None, CgTy::Ref) => gc_i8_ptr_ty.fn_type(&llvm_param_tys, false),
            (None, CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_)) => unreachable!(
                "aggregate MIR function-value returns should have been lowered through hidden sret"
            ),
        };

        let mut llvm_args: Vec<BasicMetadataValueEnum<'ctx>> = Vec::with_capacity(
            1 + args.len()
                + usize::from(hidden_sret_result_ty.is_some())
                + self.explicit_effect_hidden_abi_param_count(call_may_suspend) as usize,
        );
        let sret_result_slot = if hidden_sret_result_ty.is_some() {
            let slot = self.create_entry_alloca(span, "pass_mir_closure_call_sret", ret_cg)?;
            llvm_args.push(slot.into());
            Some(slot)
        } else {
            None
        };
        let effect_outcome_slot = if call_may_suspend {
            let slot = self.alloc_effect_outcome_slot(span, "pass_mir_closure_call")?;
            llvm_args.push(self.current_effect_ctx_arg().into());
            llvm_args.push(self.llvm_gc_i8_ptr_type().const_null().into());
            llvm_args.push(slot.into());
            Some(slot)
        } else {
            None
        };
        let evaluated_args = self.codegen_mir_callable_value_args(span, fun_ty, args, slots)?;

        let closure_obj_i8 = self.reload_deferred_gc_ref_without_clearing(
            span,
            "pass_mir_closure_call_obj_reload",
            &deferred_closure,
        )?;
        let closure_ptr = self.builder.build_pointer_cast(
            closure_obj_i8,
            closure_ptr_ty,
            "pass_mir_closure_obj_ptr",
        )?;
        let env_ptr_gep = self.builder.build_struct_gep(
            closure_ty,
            closure_ptr,
            1,
            "pass_mir_closure_env_gep",
        )?;
        let fn_ptr_gep =
            self.builder
                .build_struct_gep(closure_ty, closure_ptr, 2, "pass_mir_closure_fn_gep")?;
        let env_ptr = self
            .builder
            .build_load(gc_i8_ptr_ty, env_ptr_gep, "pass_mir_closure_env")?
            .into_pointer_value();
        let fn_ptr_raw = self
            .builder
            .build_load(i8_ptr_ty, fn_ptr_gep, "pass_mir_closure_fn")?
            .into_pointer_value();
        let typed_fn_ptr = self.builder.build_pointer_cast(
            fn_ptr_raw,
            self.llvm_ptr_type(AddressSpace::default()),
            "pass_mir_closure_fn_typed",
        )?;
        llvm_args.push(env_ptr.into());
        for arg in &evaluated_args {
            llvm_args.push(arg.value);
        }

        let call_site_result = self.with_conservative_gc_local_root_spills(span, |cg| {
            let call_site = cg.builder.build_indirect_call(
                llvm_fun_ty,
                typed_fn_ptr,
                &llvm_args,
                "pass_mir_call_closure",
            )?;
            if let Some(result_ty) = hidden_sret_result_ty {
                cg.add_sret_attribute_to_call(call_site, 0, result_ty);
            }
            Ok(call_site)
        });
        self.release_evaluated_call_arg_roots(&evaluated_args);
        let call_site = call_site_result?;
        if let Some(result_ptr) = sret_result_slot {
            self.sync_hidden_sret_result_roots(
                span,
                ret_cg,
                result_ptr,
                "pass_mir_closure_call_sret",
            )?;
        }
        let deferred_direct_result = if sret_result_slot.is_none() {
            self.defer_direct_call_result(span, ret_cg, call_site, "pass_mir_closure_call_result")?
        } else {
            None
        };
        if let Some(outcome_slot) = effect_outcome_slot {
            self.maybe_record_active_suspend_site_effect_outcome(span, outcome_slot);
            self.emit_ordinary_call_effect_propagation_check_from_outcome(
                span,
                outcome_slot,
                "pass_mir_closure_call_effect",
            )?;
        }

        match ret_cg {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            _ => {
                if let Some(result_ptr) = sret_result_slot {
                    self.load_hidden_sret_result_from_ptr(
                        span,
                        ret_cg,
                        result_ptr,
                        "pass_mir_closure_call_sret",
                    )
                } else {
                    self.materialize_deferred_cg_value(
                        span,
                        "pass_mir_closure_call_result_reload",
                        deferred_direct_result.unwrap_or_else(|| {
                            panic!("codegen_mir_function_value_call_from_closure_obj: MIR call ABI verifier accepted missing deferred return value")
                        }),
                    )
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_mir_plain_dynamic_call(
        &mut self,
        span: crate::span::Span,
        site_id: Option<crate::mir::SiteId>,
        kind: &crate::mir::CallKind,
        args: &[crate::mir::CallArg],
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_mir_plain_dynamic_call_with_policy(
            span, site_id, kind, args, body, mir_types, slots, true,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_mir_plain_dynamic_call_with_policy(
        &mut self,
        span: crate::span::Span,
        site_id: Option<crate::mir::SiteId>,
        kind: &crate::mir::CallKind,
        args: &[crate::mir::CallArg],
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        allow_effect_typed_dispatch_signature: bool,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match kind {
            crate::mir::CallKind::Closure { callee, fn_ptr } => {
                let fun_ty = self
                    .mir_operand_function_type(body, mir_types, callee)
                    .unwrap_or_else(|| {
                        panic!("codegen_mir_plain_dynamic_call_with_policy: MIR verifier accepted non-function plain closure callee")
                    });
                if !fun_ty.effects.is_pure() {
                    if self.plain_callable_carrier_fallback_allowed(
                        CallableCarrierKind::ClosureObject,
                        fn_ptr,
                    ) {
                        return self.codegen_mir_plain_function_value_call(
                            span, callee, args, &fun_ty, slots,
                        );
                    }
                    match self.mir_callable_fqn_may_outward_effect(fn_ptr) {
                        Some(false) => {}
                        Some(true) => {
                            panic!(
                                "codegen_mir_plain_dynamic_call_with_policy: effect boundary router accepted outward-effect closure target in plain lowering at {span:?}"
                            );
                        }
                        None => {
                            panic!(
                                "codegen_mir_plain_dynamic_call_with_policy: effect-typed closure surface reached plain lowering without adapter at {span:?}"
                            );
                        }
                    }
                }
                self.codegen_mir_plain_function_value_call(span, callee, args, &fun_ty, slots)
            }
            crate::mir::CallKind::FunValue { callee } => {
                let fun_ty = self
                    .mir_operand_function_type(body, mir_types, callee)
                    .unwrap_or_else(|| {
                        panic!("codegen_mir_plain_dynamic_call_with_policy: MIR call ABI verifier accepted non-function plain function-value callee")
                    });
                if !fun_ty.effects.is_pure() {
                    match self
                        .mir_fun_value_callee_fqn(body, mir_types, callee)
                        .and_then(|fqn| self.mir_callable_fqn_may_outward_effect(&fqn))
                    {
                        Some(false) => {}
                        Some(true) => {
                            panic!(
                                "codegen_mir_plain_dynamic_call_with_policy: effect boundary router accepted outward-effect function-value target in plain lowering at {span:?}"
                            );
                        }
                        None => {
                            panic!(
                                "codegen_mir_plain_dynamic_call_with_policy: effect-typed function-value surface reached plain lowering without adapter at {span:?}"
                            );
                        }
                    }
                }
                self.codegen_mir_plain_function_value_call(span, callee, args, &fun_ty, slots)
            }
            crate::mir::CallKind::FunPtr { callee } => {
                let fun_ty = self
                    .mir_operand_funptr_function_type(body, mir_types, callee)
                    .unwrap_or_else(|| {
                        panic!(
                            "codegen_mir_plain_call: materialized MIR verifier accepted non-FunPtr plain callee type"
                        )
                });
                if !fun_ty.effects.is_pure() {
                    panic!(
                        "codegen_mir_plain_dynamic_call_with_policy: effect-typed FunPtr surface reached plain lowering without adapter at {span:?}"
                    );
                }
                self.codegen_mir_funptr_value_call(
                    span,
                    callee,
                    args,
                    &fun_ty,
                    (body, mir_types, slots),
                )
            }
            crate::mir::CallKind::Virtual { receiver, .. } => {
                let site_id = site_id.ok_or_else(|| {
                    frontend_error("plain virtual dispatch missing LIR site id".to_string())
                })?;
                let target = self.resolve_plain_virtual_dispatch_target(site_id, args.len())?;
                self.codegen_mir_plain_dispatch_call(
                    span,
                    receiver,
                    args,
                    mir_types,
                    slots,
                    target,
                    allow_effect_typed_dispatch_signature,
                )
            }
            crate::mir::CallKind::Interface { receiver, .. } => {
                let site_id = site_id.ok_or_else(|| {
                    frontend_error("plain interface dispatch missing LIR site id".to_string())
                })?;
                let target = self.resolve_plain_interface_dispatch_target(site_id, args.len())?;
                self.codegen_mir_plain_dispatch_call(
                    span,
                    receiver,
                    args,
                    mir_types,
                    slots,
                    target,
                    allow_effect_typed_dispatch_signature,
                )
            }
            crate::mir::CallKind::Direct { .. } | crate::mir::CallKind::Resume { .. } => {
                panic!(
                    "codegen_mir_plain_dynamic_call_with_policy: MIR call ABI verifier accepted unsupported plain dynamic call kind"
                )
            }
        }
    }

    pub(in crate::llvm::codegen) fn codegen_mir_plain_function_value_call(
        &mut self,
        span: crate::span::Span,
        callee: &crate::mir::Operand,
        args: &[crate::mir::CallArg],
        fun_ty: &crate::ty::FunctionType,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let callee_value =
            self.codegen_mir_operand_expected(span, callee, slots, Some(CgTy::Ref))?;
        let callee_value = self.coerce_value(span, callee_value, CgTy::Ref)?;
        let Some(BasicValueEnum::PointerValue(closure_obj_i8)) = callee_value.value else {
            panic!(
                "codegen_mir_plain_function_value_call: MIR call ABI verifier accepted non-pointer plain function-value callee"
            );
        };
        self.codegen_mir_plain_function_value_call_from_closure_obj(
            span,
            closure_obj_i8,
            args,
            fun_ty,
            slots,
        )
    }

    pub(in crate::llvm::codegen) fn codegen_mir_plain_function_value_call_from_closure_obj(
        &mut self,
        span: crate::span::Span,
        closure_obj_i8: PointerValue<'ctx>,
        args: &[crate::mir::CallArg],
        fun_ty: &crate::ty::FunctionType,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let deferred_callee =
            self.defer_gc_ref_pointer(span, "plain_function_value_callee", closure_obj_i8)?;
        let closure_obj_i8 = self.reload_deferred_gc_ref_without_clearing(
            span,
            "plain_function_value_callee_reload",
            &deferred_callee,
        )?;
        self.codegen_mir_function_value_call_from_closure_obj(
            span,
            closure_obj_i8,
            fun_ty,
            false,
            args,
            slots,
        )
    }
}

fn instantiated_mir_callee_fqn(
    fqn: &str,
    generic_type_args: &[TypeId],
    mir_types: &TypeStore,
) -> Option<String> {
    if generic_type_args.is_empty() {
        return None;
    }
    let args = generic_type_args
        .iter()
        .map(|ty| mir_types.display(*ty).to_string())
        .collect::<Vec<_>>()
        .join(", ");
    Some(format!("{fqn}::<{args}>"))
}
