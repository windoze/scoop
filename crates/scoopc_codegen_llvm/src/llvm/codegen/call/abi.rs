//! Call ABI and argument binding helpers.

use super::super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    fn try_cg_ty_for_call_abi_type(&self, ty: TypeId) -> Option<CgTy> {
        self.try_cg_ty_of_type_id(ty)
            .or_else(|| match self.types.kind(ty) {
                // Function values are always passed as managed references; their generic parameter and
                // return types do not affect the ABI shape of the closure object pointer.
                TypeKind::Ref(RefTypeKind::Function(_)) => Some(CgTy::Ref),
                _ => None,
            })
    }

    fn callable_source_carrier_tys_impl(
        &self,
        source_types: &TypeStore,
        carrier_ty: TypeId,
    ) -> Option<Vec<TypeId>> {
        match source_types.kind(carrier_ty) {
            TypeKind::Value(ValueTypeKind::Tuple(elements)) => Some(elements.clone()),
            TypeKind::Value(ValueTypeKind::Unit) => Some(Vec::new()),
            _ => Some(vec![carrier_ty]),
        }
    }

    pub(in crate::llvm::codegen) fn published_signature_tys_as_codegen_tys_impl(
        &self,
        source_types: &TypeStore,
        param_tys: Vec<TypeId>,
        return_ty: TypeId,
    ) -> Option<(Vec<TypeId>, TypeId)> {
        let map_ty = |ty| {
            if std::ptr::eq(source_types, self.types) {
                Some(ty)
            } else {
                self.equivalent_codegen_type_id(source_types, ty)
            }
        };
        let param_tys = param_tys
            .into_iter()
            .map(map_ty)
            .collect::<Option<Vec<_>>>()?;
        Some((param_tys, map_ty(return_ty)?))
    }

    pub(in crate::llvm::codegen) fn llvm_param_ty_impl(
        &mut self,
        span: crate::span::Span,
        ty: TypeId,
    ) -> Result<BasicMetadataTypeEnum<'ctx>, LlvmEmitError> {
        let cg = self.try_cg_ty_for_call_abi_type(ty).unwrap_or_else(|| {
            let ty_desc = self
                .try_mono_type_id(ty)
                .map(|ty| self.types.display(ty.inner()).to_string())
                .unwrap_or_else(|| self.types.display(ty).to_string());
            tracing::warn!(
                current_callable = ?self.function_cx.current_callable_fqn,
                ?span,
                ty = %ty_desc,
                "llvm_param_ty: unsupported function param type"
            );
            panic!(
                "llvm_param_ty: MIR signature verifier accepted unsupported param type {ty_desc}"
            )
        });

        Ok(self.llvm_basic_type_of(span, cg)?.into())
    }

    pub(in crate::llvm::codegen) fn ordinary_param_abi_impl(
        &mut self,
        span: crate::span::Span,
        ty: TypeId,
    ) -> Result<OrdinaryParamAbi<'ctx>, LlvmEmitError> {
        let cg = self.try_cg_ty_for_call_abi_type(ty).unwrap_or_else(|| {
            let ty_desc = self
                .try_mono_type_id(ty)
                .map(|ty| self.types.display(ty.inner()).to_string())
                .unwrap_or_else(|| self.types.display(ty).to_string());
            tracing::warn!(
                current_callable = ?self.function_cx.current_callable_fqn,
                ?span,
                ty = %ty_desc,
                "ordinary_param_abi: unsupported function param type"
            );
            panic!("ordinary_param_abi: MIR signature verifier accepted unsupported param type {ty_desc}")
        });
        self.ordinary_param_abi_from_cg(span, ty, cg)
    }

    pub(in crate::llvm::codegen) fn ordinary_param_abi_from_cg(
        &mut self,
        span: crate::span::Span,
        ty: TypeId,
        cg: CgTy,
    ) -> Result<OrdinaryParamAbi<'ctx>, LlvmEmitError> {
        let llvm_ty = self.llvm_basic_type_of(span, cg)?;
        let needs_indirect = matches!(
            llvm_ty,
            BasicTypeEnum::StructType(_)
                | BasicTypeEnum::ArrayType(_)
                | BasicTypeEnum::VectorType(_)
                | BasicTypeEnum::ScalableVectorType(_)
        ) && self.type_contains_gc_refs(ty, &mut HashSet::new());
        if needs_indirect {
            Ok(OrdinaryParamAbi::IndirectGcAggregate {
                cg_ty: cg,
                llvm_param_ty: self.llvm_ptr_type(AddressSpace::default()).into(),
                pointee_ty: llvm_ty,
            })
        } else {
            Ok(OrdinaryParamAbi::Direct {
                cg_ty: cg,
                llvm_param_ty: llvm_ty.into(),
            })
        }
    }

    fn native_call_convention_for_origin_impl(&self, origin: NativeCallableOrigin<'_>) -> u32 {
        match origin {
            NativeCallableOrigin::DirectExtern { callable_fqn } => {
                self.llvm_call_convention_for_fqn(callable_fqn)
            }
            NativeCallableOrigin::FunPtr => 0,
        }
    }

    fn native_param_abi_impl(
        &mut self,
        span: crate::span::Span,
        ty: TypeId,
    ) -> Result<NativeParamAbi<'ctx>, LlvmEmitError> {
        self.try_cg_ty_of_type_id(ty).unwrap_or_else(|| {
            let ty_desc = self
                .try_mono_type_id(ty)
                .map(|ty| self.types.display(ty.inner()).to_string())
                .unwrap_or_else(|| format!("t{}", ty.as_u32()));
            tracing::warn!(
                current_callable = ?self.function_cx.current_callable_fqn,
                ?span,
                ty = %ty_desc,
                "native_param_abi: unsupported function param type"
            );
            panic!(
                "native_param_abi: MIR signature verifier accepted unsupported param type {ty_desc}"
            )
        });
        Ok(NativeParamAbi {
            llvm_param_ty: self.llvm_param_ty(span, ty)?,
        })
    }

    fn native_return_abi_impl(
        &mut self,
        span: crate::span::Span,
        return_ty: TypeId,
    ) -> Result<NativeReturnAbi<'ctx>, LlvmEmitError> {
        let cg_ty = self.try_cg_ty_of_type_id(return_ty).unwrap_or_else(|| {
            let ty_desc = self
                .try_mono_type_id(return_ty)
                .map(|ty| self.types.display(ty.inner()).to_string())
                .unwrap_or_else(|| format!("t{}", return_ty.as_u32()));
            tracing::warn!(
                current_callable = ?self.function_cx.current_callable_fqn,
                ?span,
                ty = %ty_desc,
                "native_return_abi: unsupported function return type"
            );
            panic!("native_return_abi: MIR signature verifier accepted unsupported return type {ty_desc}")
        });
        let llvm_return_ty = match cg_ty {
            CgTy::Unit | CgTy::Never => None,
            _ => Some(self.llvm_basic_type_of(span, cg_ty)?),
        };
        Ok(NativeReturnAbi {
            cg_ty,
            llvm_return_ty,
        })
    }

    fn classify_native_callable_impl(
        &mut self,
        span: crate::span::Span,
        param_tys: &[TypeId],
        return_ty: TypeId,
        origin: NativeCallableOrigin<'_>,
    ) -> Result<NativeCallableAbi<'ctx>, LlvmEmitError> {
        let param_abis = param_tys
            .iter()
            .copied()
            .map(|ty| self.native_param_abi_impl(span, ty))
            .collect::<Result<Vec<_>, _>>()?;
        let return_abi = self.native_return_abi_impl(span, return_ty)?;
        let llvm_param_tys = param_abis
            .iter()
            .map(|abi| abi.llvm_param_ty)
            .collect::<Vec<_>>();
        let fn_ty = match return_abi.llvm_return_ty {
            Some(return_ty) => return_ty.fn_type(&llvm_param_tys, false),
            None => self.context.void_type().fn_type(&llvm_param_tys, false),
        };
        Ok(NativeCallableAbi {
            param_abis,
            return_abi,
            fn_ty,
            aggregate_return_mode: NativeAggregateReturnMode::TargetAbiDirect,
            call_convention: self.native_call_convention_for_origin_impl(origin),
            boundary_mode: NativeBoundaryMode::EnterLeaveNative,
            effect_boundary_policy: NativeEffectBoundaryPolicy::PlainNativeLeaf,
        })
    }

    pub(in crate::llvm::codegen) fn classify_direct_extern_native_callable_impl(
        &mut self,
        span: crate::span::Span,
        callable_fqn: &str,
        param_tys: &[TypeId],
        return_ty: TypeId,
    ) -> Result<NativeCallableAbi<'ctx>, LlvmEmitError> {
        self.classify_native_callable_impl(
            span,
            param_tys,
            return_ty,
            NativeCallableOrigin::DirectExtern { callable_fqn },
        )
    }

    pub(in crate::llvm::codegen) fn classify_funptr_native_callable_impl(
        &mut self,
        span: crate::span::Span,
        param_tys: &[TypeId],
        return_ty: TypeId,
    ) -> Result<NativeCallableAbi<'ctx>, LlvmEmitError> {
        self.classify_native_callable_impl(span, param_tys, return_ty, NativeCallableOrigin::FunPtr)
    }

    pub(in crate::llvm::codegen) fn classify_native_callable_body_symbol_impl(
        &mut self,
        span: crate::span::Span,
        param_tys: &[TypeId],
        return_ty: TypeId,
        calling_convention: &str,
    ) -> Result<NativeCallableAbi<'ctx>, LlvmEmitError> {
        let mut abi = self.classify_native_callable_impl(
            span,
            param_tys,
            return_ty,
            NativeCallableOrigin::FunPtr,
        )?;
        abi.call_convention = self.llvm_call_convention_for_name(calling_convention);
        Ok(abi)
    }

    pub(in crate::llvm::codegen) fn emit_native_callable_call_impl(
        &mut self,
        at: crate::span::Span,
        abi: &NativeCallableAbi<'ctx>,
        target: NativeCallableTarget<'ctx>,
        llvm_args: &[inkwell::values::BasicMetadataValueEnum<'ctx>],
    ) -> Result<CallSiteValue<'ctx>, LlvmEmitError> {
        debug_assert!(matches!(
            abi.aggregate_return_mode,
            NativeAggregateReturnMode::TargetAbiDirect
        ));
        debug_assert!(matches!(
            abi.effect_boundary_policy,
            NativeEffectBoundaryPolicy::PlainNativeLeaf
        ));

        match abi.boundary_mode {
            NativeBoundaryMode::EnterLeaveNative => self.emit_enter_native_for_extern_call(at)?,
        }

        let call_site = match target {
            NativeCallableTarget::Direct(llvm_fun) => {
                self.builder.build_call(llvm_fun, llvm_args, "call")?
            }
            NativeCallableTarget::Indirect {
                fn_ty,
                ptr,
                call_name,
            } => self
                .builder
                .build_indirect_call(fn_ty, ptr, llvm_args, call_name)?,
        };
        call_site.set_call_convention(abi.call_convention);

        match abi.boundary_mode {
            NativeBoundaryMode::EnterLeaveNative => {
                let leave = self.declare_runtime_leave_native();
                let _ = self.builder.build_call(leave, &[], "leave_native")?;
            }
        }
        Ok(call_site)
    }

    fn direct_callable_abi_identity_for_fqn_impl(
        &self,
        callable_fqn: &str,
    ) -> hir::CallableAbiIdentity {
        if let Some(extern_fun) = self.extern_funs.get(callable_fqn) {
            return extern_fun.callable_abi_identity();
        }
        if self.callable_uses_explicit_effect_hidden_abi_impl(callable_fqn) {
            return hir::CallableAbiIdentity::EffectBridge;
        }
        hir::CallableAbiIdentity::ManagedOrdinary
    }

    pub(in crate::llvm::codegen) fn direct_call_abi_identity_impl(
        &self,
        callable_fqn: &str,
    ) -> hir::CallableAbiIdentity {
        self.direct_callable_abi_identity_for_fqn_impl(callable_fqn)
    }

    pub(in crate::llvm::codegen) fn managed_callable_abi_identity_impl(
        &self,
        call_may_suspend: bool,
    ) -> hir::CallableAbiIdentity {
        hir::CallableAbiIdentity::managed_callable(call_may_suspend)
    }

    pub(in crate::llvm::codegen) fn managed_callable_abi_identity_from_fun_ty_impl(
        &self,
        fun_ty: &crate::ty::FunctionType,
    ) -> hir::CallableAbiIdentity {
        self.managed_callable_abi_identity_impl(!fun_ty.effects.is_pure())
    }

    /// 已发布 callable contract 中，只要某个 root version 仍需要 effect-step callable surface，
    /// 其遗留声明入口就必须预留显式 hidden ABI，而不能再从 HIR `effectful` 布尔值反推。
    pub(in crate::llvm::codegen) fn callable_uses_explicit_effect_hidden_abi_impl(
        &self,
        callable_fqn: &str,
    ) -> bool {
        self.published_lir_facts.callables.values().any(|callable| {
            callable.root_fqn == callable_fqn
                && matches!(
                    callable.kind(),
                    scoopc_lir_facts::LirCallableKind::EffectStep
                )
        })
    }

    pub(in crate::llvm::codegen) fn callable_needs_callee_resume_shell_impl(
        &self,
        callable_fqn: &str,
    ) -> bool {
        self.published_lir_facts.callables.values().any(|callable| {
            callable.root_fqn == callable_fqn && callable.body_version.needs_reentry
        })
    }

    pub(in crate::llvm::codegen) fn published_callable_signature_impl(
        &self,
        callable_fqn: &str,
    ) -> Option<(&'a TypeStore, Vec<TypeId>, TypeId)> {
        let program = self.published_late_lowered_program()?;
        let source_types = self.published_late_lowered_types()?;
        let callable = program.callable(callable_fqn)?;
        if let Some(plain) = callable.plain_abi() {
            return Some((source_types, plain.param_tys().to_vec(), plain.return_ty()));
        }
        let effect = callable.effect_step_abi()?;
        let param_tys = self.callable_source_carrier_tys_impl(
            source_types,
            effect.dynamic_invoke_entry().invoke_args_tuple_ty(),
        )?;
        let return_ty = program.step_type(effect.step_schema())?.complete_ty();
        Some((source_types, param_tys, return_ty))
    }

    pub(in crate::llvm::codegen) fn published_callable_signature_with_names_impl(
        &self,
        callable_fqn: &str,
    ) -> Option<(&'a TypeStore, Vec<String>, Vec<TypeId>, TypeId)> {
        if let Some((source_types, param_tys, return_ty)) =
            self.published_callable_signature_impl(callable_fqn)
        {
            let callable_facts = self
                .published_lir_facts
                .callables
                .values()
                .find(|callable| callable.root_fqn == callable_fqn)?;
            return Some((
                source_types,
                callable_facts.param_names.clone(),
                param_tys,
                return_ty,
            ));
        }
        if let Some(signature) = self.published_lir_facts.source_signatures.get(callable_fqn) {
            return Some((
                self.types,
                signature.param_names.clone(),
                signature.param_tys.clone(),
                signature.return_ty,
            ));
        }
        None
    }

    pub(in crate::llvm::codegen) fn published_codegen_callable_signature_impl(
        &self,
        callable_fqn: &str,
    ) -> Option<CodegenCallableSignature> {
        let (source_types, param_names, param_tys, return_ty) =
            self.published_callable_signature_with_names_impl(callable_fqn)?;
        let (param_tys, return_ty) =
            self.published_signature_tys_as_codegen_tys_impl(source_types, param_tys, return_ty)?;
        Some(CodegenCallableSignature {
            fqn: callable_fqn.to_string(),
            param_names,
            param_tys,
            return_ty,
        })
    }

    pub(in crate::llvm::codegen) fn explicit_effect_hidden_abi_param_count_impl(
        &self,
        uses_explicit_effect_hidden_abi: bool,
    ) -> u32 {
        if uses_explicit_effect_hidden_abi {
            3
        } else {
            0
        }
    }

    pub(in crate::llvm::codegen) fn push_explicit_effect_hidden_abi_param_tys_impl(
        &self,
        llvm_params: &mut Vec<BasicMetadataTypeEnum<'ctx>>,
    ) {
        llvm_params.push(self.llvm_gc_i8_ptr_type().into());
        llvm_params.push(self.llvm_gc_i8_ptr_type().into());
        llvm_params.push(self.context.ptr_type(AddressSpace::default()).into());
    }

    pub(in crate::llvm::codegen) fn bind_explicit_effect_hidden_abi_slots_impl(
        &mut self,
        at: crate::span::Span,
        llvm_fun: FunctionValue<'ctx>,
        first_hidden_param_index: u32,
        uses_explicit_effect_hidden_abi: bool,
    ) -> Result<(), LlvmEmitError> {
        if !uses_explicit_effect_hidden_abi {
            self.clear_explicit_effect_hidden_abi_slots();
            return Ok(());
        }

        self.function_cx.current_effect_ctx_ref = Some(
            llvm_fun
                .get_nth_param(first_hidden_param_index)
                .expect("explicit-effect ABI must provide current_effect_ctx_ref parameter")
                .into_pointer_value(),
        );
        self.function_cx.current_incoming_resume_token_ref = Some(
            llvm_fun
                .get_nth_param(first_hidden_param_index + 1)
                .unwrap_or_else(|| {
                    panic!(
                        "bind_explicit_effect_hidden_abi_slots: effect ABI verifier accepted missing incoming_resume_token_ref param at {at:?}"
                    )
                })
                .into_pointer_value(),
        );
        self.function_cx.current_effect_outcome_ptr = Some(
            llvm_fun
                .get_nth_param(first_hidden_param_index + 2)
                .unwrap_or_else(|| {
                    panic!(
                        "bind_explicit_effect_hidden_abi_slots: effect ABI verifier accepted missing effect outcome param at {at:?}"
                    )
                })
                .into_pointer_value(),
        );
        Ok(())
    }

    pub(in crate::llvm::codegen) fn clear_explicit_effect_hidden_abi_slots_impl(&mut self) {
        self.function_cx.current_effect_ctx_ref = None;
        self.function_cx.current_incoming_resume_token_ref = None;
        self.function_cx.current_effect_outcome_ptr = None;
    }

    pub(in crate::llvm::codegen) fn cg_value_from_llvm_param_impl(
        &self,
        _at: crate::span::Span,
        llvm_fun: FunctionValue<'ctx>,
        param_index: u32,
        target_ty: CgTy,
        missing_kind: &'static str,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let raw = llvm_fun
            .get_nth_param(param_index)
            .unwrap_or_else(|| std::panic::panic_any(missing_kind));

        Ok(match target_ty {
            CgTy::Unit => CgValue::unit(),
            CgTy::Never => CgValue::never(),
            CgTy::Bool => CgValue::bool(raw.into_int_value()),
            CgTy::Float64 | CgTy::Float32 => CgValue::float(raw.into_float_value(), target_ty),
            CgTy::Int(int_ty) => CgValue::int(raw.into_int_value(), int_ty),
            CgTy::String => CgValue {
                ty: CgTy::String,
                value: Some(raw.into_pointer_value().into()),
            },
            CgTy::Ref => CgValue {
                ty: CgTy::Ref,
                value: Some(raw.into_pointer_value().into()),
            },
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => CgValue {
                ty: target_ty,
                value: Some(raw),
            },
        })
    }

    pub(in crate::llvm::codegen) fn bind_ordinary_param_local_impl(
        &mut self,
        binding: OrdinaryParamLocalBinding<'ctx, '_>,
    ) -> Result<(), LlvmEmitError> {
        let OrdinaryParamLocalBinding {
            at,
            llvm_fun,
            param_index,
            name,
            id,
            ty_id,
            call_may_suspend,
            missing_kind,
        } = binding;
        let abi = self.ordinary_param_abi(at, ty_id)?;
        let target_ty = abi.cg_ty();
        let ptr = if abi.pointee_ty().is_some() {
            let storage_ty = self.llvm_basic_type_of(at, target_ty)?;
            let ptr = llvm_fun
                .get_nth_param(param_index)
                .unwrap_or_else(|| std::panic::panic_any(missing_kind))
                .into_pointer_value();
            let frame_slots =
                self.reserve_explicit_frame_leaf_slots_for_storage_type(at, storage_ty)?;
            self.record_explicit_frame_slot_mirrors(ptr, frame_slots);
            self.sync_storage_slot_into_explicit_frame(at, ptr, storage_ty, name)?;
            ptr
        } else {
            let ptr = self.create_entry_alloca(at, name, target_ty)?;
            let init =
                self.cg_value_from_llvm_param(at, llvm_fun, param_index, target_ty, missing_kind)?;
            let _ = self.store_local_value(at, ptr, target_ty, init)?;
            ptr
        };

        self.function_cx.env.insert(
            id,
            CgLocal {
                hir_ty: Some(ty_id),
                call_may_suspend,
                ty: target_ty,
                ptr,
                frame_backing_ptr: None,
                mutable: false,
            },
        );
        Ok(())
    }

    pub(in crate::llvm::codegen) fn materialize_deferred_cg_value_for_call_arg_impl(
        &mut self,
        at: crate::span::Span,
        name: &str,
        value: DeferredCgValue<'ctx>,
    ) -> Result<(CgValue<'ctx>, Vec<DeferredGcSensitiveSpill<'ctx>>), LlvmEmitError> {
        if let Some(raw) = value.immediate {
            return Ok((
                CgValue {
                    ty: value.ty,
                    value: Some(raw),
                },
                Vec::new(),
            ));
        }

        if let Some(spill) = value.spill {
            let llvm_ty = self.llvm_basic_type_of(at, value.ty)?;
            let reload_slot = self.storage_slot_for_use(at, spill.slot, value.ty, name)?;
            let loaded = self.builder.build_load(llvm_ty, reload_slot, name)?;
            return Ok((
                CgValue {
                    ty: value.ty,
                    value: Some(loaded),
                },
                vec![spill],
            ));
        }

        Ok((
            CgValue {
                ty: value.ty,
                value: None,
            },
            Vec::new(),
        ))
    }

    pub(in crate::llvm::codegen) fn deferred_gc_spill_slot_for_call_arg_impl(
        &mut self,
        at: crate::span::Span,
        name: &str,
        value: DeferredCgValue<'ctx>,
    ) -> Result<(PointerValue<'ctx>, Vec<DeferredGcSensitiveSpill<'ctx>>), LlvmEmitError> {
        let spill = value.spill.unwrap_or_else(|| {
            panic!("deferred_gc_spill_slot_for_call_arg_impl: aggregate ABI verifier accepted an unspilled indirect argument")
        });
        let slot = self.storage_slot_for_use(at, spill.slot, value.ty, name)?;
        Ok((slot, vec![spill]))
    }

    pub(in crate::llvm::codegen) fn release_evaluated_call_arg_roots_impl(
        &mut self,
        args: &[EvaluatedCallArg<'ctx>],
    ) {
        for arg in args {
            for spill in &arg.cleanup_spills {
                let _ = self.clear_spill_slot_root_homes(
                    crate::span::Span::new(0, 0),
                    spill.slot,
                    spill.value_ty,
                    "call_arg_cleanup",
                );
            }
        }
    }

    pub(in crate::llvm::codegen) fn as_llvm_arg_value_impl(
        &self,
        _span: crate::span::Span,
        param_ty: CgTy,
        value: CgValue<'ctx>,
    ) -> Result<inkwell::values::BasicMetadataValueEnum<'ctx>, LlvmEmitError> {
        Ok(match param_ty {
            CgTy::Unit | CgTy::Never => self.context.i8_type().const_int(0, false).into(),
            CgTy::Bool
            | CgTy::Float64
            | CgTy::Float32
            | CgTy::Int(_)
            | CgTy::String
            | CgTy::Ref
            | CgTy::Tuple(_)
            | CgTy::Struct(_)
            | CgTy::Enum(_) => value
                .value
                .unwrap_or_else(|| {
                    panic!("as_llvm_arg_value_impl: call ABI verifier accepted valueless call arg")
                })
                .into(),
        })
    }

    pub(in crate::llvm::codegen) fn map_call_args_to_params_by_name_impl(
        &self,
        param_names: &[String],
        args: &[hir::CallArg],
    ) -> Option<Vec<usize>> {
        if args.len() != param_names.len() {
            return None;
        }

        let mut seen_named = false;
        let mut positional_count = 0usize;
        for arg in args {
            match arg {
                hir::CallArg::Positional(_) => {
                    if seen_named {
                        return None;
                    }
                    positional_count = positional_count.saturating_add(1);
                }
                hir::CallArg::Named { .. } => {
                    seen_named = true;
                }
            }
        }

        if positional_count > param_names.len() {
            return None;
        }

        let mut param_to_arg: Vec<Option<usize>> = vec![None; param_names.len()];
        for (slot_idx, arg_idx) in (0..positional_count).enumerate() {
            let slot = param_to_arg.get_mut(slot_idx)?;
            *slot = Some(arg_idx);
        }

        for (arg_idx, arg) in args.iter().enumerate().skip(positional_count) {
            let hir::CallArg::Named { name, .. } = arg else {
                return None;
            };
            let slot_idx = param_names.iter().position(|param| param == name)?;
            let slot = param_to_arg.get_mut(slot_idx)?;
            if slot.is_some() {
                return None;
            }
            *slot = Some(arg_idx);
        }

        let mut arg_to_param: Vec<Option<usize>> = vec![None; args.len()];
        for (param_idx, arg_idx) in param_to_arg.into_iter().enumerate() {
            let arg_idx = arg_idx?;
            let slot = arg_to_param.get_mut(arg_idx)?;
            if slot.is_some() {
                return None;
            }
            *slot = Some(param_idx);
        }

        arg_to_param.into_iter().collect()
    }

    pub(in crate::llvm::codegen) fn codegen_bound_call_args_impl(
        &mut self,
        spec: BoundCallArgsSpec,
        param_names: &[String],
        param_tys: &[TypeId],
        args: &[hir::CallArg],
    ) -> Result<Vec<EvaluatedCallArg<'ctx>>, LlvmEmitError> {
        let BoundCallArgsSpec {
            span,
            callee_span,
            kind,
            abi_mode,
        } = spec;
        assert!(
            param_names.len() == param_tys.len() && args.len() == param_names.len(),
            "typecheck must bind call arguments before LLVM codegen at {span:?}: {kind}"
        );

        let arg_to_param = self
            .map_call_args_to_params_by_name(param_names, args)
            .unwrap_or_else(|| std::panic::panic_any(kind));

        let mut evaluated: Vec<Option<(crate::span::Span, DeferredCgValue<'ctx>, CgTy)>> =
            vec![None; param_names.len()];
        for (arg_idx, arg) in args.iter().enumerate() {
            let param_idx = arg_to_param
                .get(arg_idx)
                .copied()
                .unwrap_or_else(|| std::panic::panic_any(kind));
            let param_ty = *param_tys
                .get(param_idx)
                .unwrap_or_else(|| std::panic::panic_any(kind));
            let target_cg = self
                .try_cg_ty_for_call_abi_type(param_ty)
                .unwrap_or_else(|| {
                    panic!(
                        "codegen_bound_call_args_impl: call ABI verifier accepted unsupported call arg type {}",
                        self.types.display(param_ty)
                    )
                });
            let expr = match arg {
                hir::CallArg::Positional(expr) => expr,
                hir::CallArg::Named { value, .. } => value,
            };
            let v = match &expr.kind {
                hir::ExprKind::Closure(closure) => {
                    self.codegen_closure_expr(expr.span, closure, param_ty)?
                }
                _ => self.codegen_expr_in_expected_context(expr, Some(target_cg))?,
            };
            let coerced = self.coerce_value(expr.span, v, target_cg)?;
            let deferred = self.defer_gc_sensitive_cg_value(
                expr.span,
                &format!("call_arg_{param_idx}"),
                coerced,
            )?;
            let slot = evaluated
                .get_mut(param_idx)
                .unwrap_or_else(|| std::panic::panic_any(kind));
            *slot = Some((expr.span, deferred, target_cg));
        }

        evaluated
            .into_iter()
            .enumerate()
            .map(|(param_idx, slot)| {
                let (expr_span, deferred, param_cg) =
                    slot.unwrap_or_else(|| std::panic::panic_any(kind));
                let param_ty = *param_tys.get(param_idx).unwrap_or_else(|| {
                    panic!(
                        "codegen_bound_call_args_impl: call ABI verifier accepted param index drift"
                    )
                });
                let param_abi = match abi_mode {
                    CallArgAbiMode::Native => None,
                    CallArgAbiMode::Ordinary => {
                        Some(self.ordinary_param_abi_from_cg(callee_span, param_ty, param_cg)?)
                    }
                };
                if let Some(abi) = param_abi
                    && abi.pointee_ty().is_some()
                {
                    let (slot_ptr, cleanup_spills) = self.deferred_gc_spill_slot_for_call_arg(
                        expr_span,
                        &format!("call_arg_reload_{param_idx}"),
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
                        expr_span,
                        &format!("call_arg_reload_{param_idx}"),
                        deferred,
                    )?;
                let pointer_value = match materialized.value {
                    Some(BasicValueEnum::PointerValue(ptr)) => Some(ptr),
                    _ => None,
                };
                let param_cg = param_abi
                    .map(OrdinaryParamAbi::cg_ty)
                    .unwrap_or(materialized.ty);
                let value = self.as_llvm_arg_value(expr_span, param_cg, materialized)?;
                Ok(EvaluatedCallArg {
                    value,
                    pointer_value,
                    cleanup_spills,
                })
            })
            .collect()
    }

    pub(in crate::llvm::codegen) fn callable_value_param_names_impl(
        &self,
        fun_ty: &crate::ty::FunctionType,
    ) -> Vec<String> {
        let mut out =
            Vec::with_capacity(fun_ty.params.len() + usize::from(fun_ty.receiver.is_some()));
        if fun_ty.receiver.is_some() {
            out.push("receiver".to_string());
        }
        for idx in 0..fun_ty.params.len() {
            out.push(format!("a{idx}"));
        }
        out
    }

    pub(in crate::llvm::codegen) fn callable_value_param_tys_impl(
        &self,
        fun_ty: &crate::ty::FunctionType,
    ) -> Vec<TypeId> {
        let mut out =
            Vec::with_capacity(fun_ty.params.len() + usize::from(fun_ty.receiver.is_some()));
        if let Some(receiver_ty) = fun_ty.receiver {
            out.push(receiver_ty);
        }
        out.extend(fun_ty.params.iter().copied());
        out
    }

    pub(in crate::llvm::codegen) fn codegen_callable_value_args_impl(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fun_ty: &crate::ty::FunctionType,
        args: &[hir::CallArg],
        kind: &'static str,
        abi_mode: CallArgAbiMode,
    ) -> Result<Vec<EvaluatedCallArg<'ctx>>, LlvmEmitError> {
        let param_names = self.callable_value_param_names(fun_ty);
        let param_tys = self.callable_value_param_tys(fun_ty);
        self.codegen_bound_call_args(
            BoundCallArgsSpec {
                span,
                callee_span,
                kind,
                abi_mode,
            },
            &param_names,
            &param_tys,
            args,
        )
    }
}
