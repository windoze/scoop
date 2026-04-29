//! Call ABI and argument binding helpers.

use super::super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn llvm_param_ty_impl(
        &mut self,
        span: crate::span::Span,
        ty: TypeId,
    ) -> Result<BasicMetadataTypeEnum<'ctx>, LlvmEmitError> {
        let cg = self
            .cg_ty_of(ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "function param type",
                at: span.into(),
            })?;

        Ok(self.llvm_basic_type_of(span, cg)?.into())
    }

    pub(in crate::llvm::codegen) fn ordinary_param_abi_impl(
        &mut self,
        span: crate::span::Span,
        ty: TypeId,
    ) -> Result<OrdinaryParamAbi<'ctx>, LlvmEmitError> {
        let cg = self
            .cg_ty_of(ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "function param type",
                at: span.into(),
            })?;
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

    pub(in crate::llvm::codegen) fn cg_value_from_llvm_param_impl(
        &self,
        at: crate::span::Span,
        llvm_fun: FunctionValue<'ctx>,
        param_index: u32,
        target_ty: CgTy,
        missing_kind: &'static str,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let raw =
            llvm_fun
                .get_nth_param(param_index)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: missing_kind,
                    at: at.into(),
                })?;

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
            self.track_explicit_frame_storage_type(at, storage_ty)?;
            llvm_fun
                .get_nth_param(param_index)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: missing_kind,
                    at: at.into(),
                })?
                .into_pointer_value()
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
    ) -> Result<(CgValue<'ctx>, Vec<u32>), LlvmEmitError> {
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
            let slot =
                self.rematerialize_ptr_in_current_block(at, spill.slot, &format!("{name}_slot"))?;
            let llvm_ty = self.llvm_basic_type_of(at, value.ty)?;
            let loaded = self.builder.build_load(llvm_ty, slot, name)?;
            return Ok((
                CgValue {
                    ty: value.ty,
                    value: Some(loaded),
                },
                spill.root_slot_ids,
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
    ) -> Result<(PointerValue<'ctx>, Vec<u32>), LlvmEmitError> {
        let spill = value.spill.ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "indirect aggregate call arg spill",
            at: at.into(),
        })?;
        let slot =
            self.rematerialize_ptr_in_current_block(at, spill.slot, &format!("{name}_slot"))?;
        Ok((slot, spill.root_slot_ids))
    }

    pub(in crate::llvm::codegen) fn release_evaluated_call_arg_roots_impl(
        &mut self,
        args: &[EvaluatedCallArg<'ctx>],
    ) {
        for arg in args {
            self.release_gc_root_slot_ids(&arg.cleanup_root_slot_ids);
        }
    }

    pub(in crate::llvm::codegen) fn as_llvm_arg_value_impl(
        &self,
        span: crate::span::Span,
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
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "call arg value",
                    at: span.into(),
                })?
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
        if param_names.len() != param_tys.len() || args.len() != param_names.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind,
                at: span.into(),
            });
        }

        let arg_to_param = self
            .map_call_args_to_params_by_name(param_names, args)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind,
                at: span.into(),
            })?;

        let mut evaluated: Vec<Option<(crate::span::Span, DeferredCgValue<'ctx>)>> =
            vec![None; param_names.len()];
        for (arg_idx, arg) in args.iter().enumerate() {
            let param_idx =
                arg_to_param
                    .get(arg_idx)
                    .copied()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind,
                        at: span.into(),
                    })?;
            let param_ty = *param_tys
                .get(param_idx)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind,
                    at: callee_span.into(),
                })?;
            let target_cg = self
                .cg_ty_of(param_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "call arg type",
                    at: callee_span.into(),
                })?;
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
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind,
                    at: span.into(),
                })?;
            *slot = Some((expr.span, deferred));
        }

        evaluated
            .into_iter()
            .enumerate()
            .map(|(param_idx, slot)| {
                let (expr_span, deferred) = slot.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind,
                    at: span.into(),
                })?;
                let param_ty =
                    *param_tys
                        .get(param_idx)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "call arg param type",
                            at: callee_span.into(),
                        })?;
                let param_abi = match abi_mode {
                    CallArgAbiMode::Native => None,
                    CallArgAbiMode::Ordinary => {
                        Some(self.ordinary_param_abi(callee_span, param_ty)?)
                    }
                };
                if let Some(abi) = param_abi
                    && abi.pointee_ty().is_some()
                {
                    let (slot_ptr, cleanup_root_slot_ids) = self
                        .deferred_gc_spill_slot_for_call_arg(
                            expr_span,
                            &format!("call_arg_reload_{param_idx}"),
                            deferred,
                        )?;
                    return Ok(EvaluatedCallArg {
                        value: slot_ptr.into(),
                        pointer_value: None,
                        cleanup_root_slot_ids,
                    });
                }

                let (materialized, cleanup_root_slot_ids) = self
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
                    cleanup_root_slot_ids,
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
