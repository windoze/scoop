//! Plain virtual / interface dispatch resolution and dispatch-call lowering.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn resolve_plain_virtual_dispatch_target(
        &self,
        site_id: mir_source::SiteId,
        explicit_arg_count: usize,
    ) -> Result<PlainDispatchTarget, LlvmEmitError> {
        let dispatch = self.required_lir_dispatch(site_id, "plain virtual dispatch")?;
        if dispatch.kind != LirCallSiteKind::Virtual {
            return Err(frontend_error(format!(
                "plain virtual call site{} published non-virtual dispatch contract",
                site_id.as_u32()
            )));
        }
        if dispatch.explicit_arg_count != explicit_arg_count {
            return Err(frontend_error(format!(
                "plain virtual call site{} arity drift: LIR facts={}, MIR={}",
                site_id.as_u32(),
                dispatch.explicit_arg_count,
                explicit_arg_count
            )));
        }
        let signature = self.dispatch_callable_signature(dispatch)?;
        Ok(PlainDispatchTarget::Virtual {
            slot: dispatch.method_slot,
            signature,
        })
    }

    pub(in crate::llvm::codegen) fn resolve_plain_interface_dispatch_target(
        &self,
        site_id: mir_source::SiteId,
        explicit_arg_count: usize,
    ) -> Result<PlainDispatchTarget, LlvmEmitError> {
        let dispatch = self.required_lir_dispatch(site_id, "plain interface dispatch")?;
        if dispatch.kind != LirCallSiteKind::Interface {
            return Err(frontend_error(format!(
                "plain interface call site{} published non-interface dispatch contract",
                site_id.as_u32()
            )));
        }
        if dispatch.explicit_arg_count != explicit_arg_count {
            return Err(frontend_error(format!(
                "plain interface call site{} arity drift: LIR facts={}, MIR={}",
                site_id.as_u32(),
                dispatch.explicit_arg_count,
                explicit_arg_count
            )));
        }
        let interface_id = dispatch.interface_id.ok_or_else(|| {
            frontend_error(format!(
                "plain interface call site{} missing published interface id",
                site_id.as_u32()
            ))
        })?;
        let signature = self.dispatch_callable_signature(dispatch)?;
        Ok(PlainDispatchTarget::Interface {
            interface_fqn: dispatch.owner_fqn.clone(),
            interface_id,
            slot: dispatch.method_slot,
            receiver_ty: dispatch.receiver_ty,
            signature,
        })
    }

    fn dispatch_callable_signature(
        &self,
        dispatch: &LirDispatchContract,
    ) -> Result<CodegenCallableSignature, LlvmEmitError> {
        self.dispatch_target_signature(dispatch).ok_or_else(|| {
            frontend_error(format!(
                "plain dispatch call site{} 缺少 `{}` 的 LIR signature",
                dispatch.site_id.as_u32(),
                dispatch.member_fqn,
            ))
        })
    }

    fn dispatch_target_signature(
        &self,
        dispatch: &LirDispatchContract,
    ) -> Option<CodegenCallableSignature> {
        let target = dispatch.candidate_targets.first().copied()?;
        self.published_codegen_callable_signature_for_ref(target)
    }

    pub(in crate::llvm::codegen) fn static_interface_receiver_owner_fqn(
        &self,
        mir_types: &TypeStore,
        receiver_ty: TypeId,
    ) -> Option<(String, TypeId)> {
        let codegen_ty = self
            .equivalent_codegen_type_id(mir_types, receiver_ty)
            .unwrap_or(receiver_ty);
        let owner = match self.types.kind(codegen_ty) {
            TypeKind::Value(ValueTypeKind::Bool) => "scoop.core.Bool".to_string(),
            TypeKind::Value(ValueTypeKind::Char) => "scoop.core.Char".to_string(),
            TypeKind::Value(ValueTypeKind::Float64) => "scoop.core.Float64".to_string(),
            TypeKind::Value(ValueTypeKind::Float32) => "scoop.core.Float32".to_string(),
            TypeKind::Value(ValueTypeKind::Int) => "scoop.core.Int".to_string(),
            TypeKind::Ref(RefTypeKind::String) => "scoop.core.String".to_string(),
            TypeKind::Value(ValueTypeKind::Nominal(nominal)) => nominal.fqn.clone(),
            _ => return None,
        };
        Some((owner, codegen_ty))
    }

    pub(in crate::llvm::codegen) fn static_interface_dispatch_impl(
        &self,
        mir_types: &TypeStore,
        receiver_ty: TypeId,
        interface_id: u64,
        slot: u32,
    ) -> Option<(TypeId, scoopc_lir_facts::LirCallableRef, String)> {
        let (owner_fqn, source_ty) =
            self.static_interface_receiver_owner_fqn(mir_types, receiver_ty)?;
        let itable = self
            .expect_active_lir_program("static_interface_dispatch_impl")
            .physical_layout()
            .class_itables
            .get(owner_fqn.as_str())?;
        let entry = itable
            .entries
            .iter()
            .find(|entry| entry.interface_id == interface_id)?;
        let idx = slot as usize;
        let target = entry
            .method_impl_targets
            .get(idx)
            .and_then(|target| *target)?;
        let target_label = target.display_text();
        Some((source_ty, target, target_label))
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_mir_plain_static_interface_dispatch_call(
        &mut self,
        span: crate::span::Span,
        receiver: &mir_source::Operand,
        args: &[mir_source::CallArg],
        slots: &[MirLocalSlot<'ctx>],
        source_ty: TypeId,
        target: scoopc_lir_facts::LirCallableRef,
        target_label: &str,
        allow_effect_typed_signature: bool,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let impl_sig = self
            .published_codegen_callable_signature_for_ref(target)
            .ok_or_else(|| {
                frontend_error(format!(
                    "static interface dispatch target `{target_label}` missing LIR signature"
                ))
            })?;
        if impl_sig.param_tys.len() != args.len() + 1 {
            std::panic::panic_any(
                "codegen_mir_plain_static_interface_dispatch_call: MIR verifier accepted static interface arity drift",
            );
        }
        if !allow_effect_typed_signature
            && self
                .direct_call_abi_identity_for_ref(target)
                .uses_effect_bridge_abi()
        {
            panic!(
                "codegen_mir_plain_static_interface_dispatch_call: effect boundary router accepted outward-effect static dispatch target in plain lowering at {span:?}"
            );
        }

        let ret_cg = self.try_cg_ty_of_type_id(impl_sig.return_ty).unwrap_or_else(|| {
            panic!(
                "codegen_mir_plain_static_interface_dispatch_call: MIR verifier accepted unsupported return type"
            )
        });
        let hidden_sret_result_ty = self.hidden_sret_result_ty(span, ret_cg)?;
        let hidden_sret_slot = if hidden_sret_result_ty.is_some() {
            Some(self.create_entry_alloca(span, "static_iface_call_sret", ret_cg)?)
        } else {
            None
        };
        let direct_result_storage =
            if hidden_sret_result_ty.is_none() && !matches!(ret_cg, CgTy::Unit | CgTy::Never) {
                Some(self.create_entry_alloca(span, "static_iface_call_result", ret_cg)?)
            } else {
                None
            };

        let source_cg = self.cg_ty_of_type_id(source_ty, "static interface receiver type");
        let receiver_value =
            self.codegen_mir_operand_expected(span, receiver, slots, Some(source_cg))?;
        let receiver_value = self.coerce_value(span, receiver_value, source_cg)?;
        let receiver_arg = if self
            .ordinary_param_abi(span, source_ty)?
            .pointee_ty()
            .is_some()
        {
            let receiver_slot =
                self.create_entry_alloca(span, "static_iface_receiver", source_cg)?;
            let _ = self.store_local_value(span, receiver_slot, source_cg, receiver_value)?;
            receiver_slot.into()
        } else {
            self.as_llvm_arg_value(span, source_cg, receiver_value)?
        };

        let explicit_param_names = impl_sig.param_names[1..].to_vec();
        let explicit_param_tys = impl_sig.param_tys[1..].to_vec();
        let evaluated_explicit_args = self.codegen_bound_mir_call_args_from_signature(
            span,
            &explicit_param_names,
            &explicit_param_tys,
            args,
            slots,
            false,
            self.types,
        )?;
        let explicit_args = evaluated_explicit_args
            .iter()
            .map(|arg| arg.value)
            .collect::<Vec<_>>();

        let function = self.declare_lir_plain_fun_with_symbol_for_ref(
            &impl_sig.fqn,
            LlvmFunctionDeclarationSurface::ExportedAbi,
            target,
            &impl_sig.fqn,
            &impl_sig.param_tys,
            impl_sig.return_ty,
            self.types,
            false,
        )?;
        let fn_i8 = function.as_global_value().as_pointer_value();
        self.emit_interface_dispatch_case_call_to_storage(
            span,
            span,
            &impl_sig.fqn,
            fn_i8,
            receiver_arg,
            source_ty,
            &explicit_param_tys,
            &explicit_args,
            ret_cg,
            hidden_sret_result_ty,
            hidden_sret_slot,
            false,
            None,
            direct_result_storage,
        )?;
        self.release_evaluated_call_arg_roots(&evaluated_explicit_args);

        if let Some(result_ptr) = hidden_sret_slot {
            self.sync_hidden_sret_result_roots(span, ret_cg, result_ptr, "static_iface_call_sret")?;
        }

        match ret_cg {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            _ => {
                if let Some(result_ptr) = hidden_sret_slot {
                    self.load_hidden_sret_result_from_ptr(
                        span,
                        ret_cg,
                        result_ptr,
                        "static_iface_call_sret",
                    )
                } else {
                    self.load_dispatch_result_from_storage(
                        span,
                        ret_cg,
                        direct_result_storage.unwrap_or_else(|| {
                            std::panic::panic_any(
                                "codegen_mir_plain_static_interface_dispatch_call: direct return must publish result storage",
                            )
                        }),
                    )
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_lir_plain_static_interface_dispatch_call(
        &mut self,
        span: crate::span::Span,
        receiver: &LirOperand,
        args: &[LirCallArg],
        body: &LirExecutableBody,
        source_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        source_ty: TypeId,
        target: scoopc_lir_facts::LirCallableRef,
        target_label: &str,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let impl_sig = self
            .published_codegen_callable_signature_for_ref(target)
            .ok_or_else(|| {
                frontend_error(format!(
                    "static LIR interface dispatch target `{target_label}` missing LIR signature"
                ))
            })?;
        if impl_sig.param_tys.len() != args.len() + 1 {
            std::panic::panic_any(
                "codegen_lir_plain_static_interface_dispatch_call: LIR verifier accepted static interface arity drift",
            );
        }
        if self
            .direct_call_abi_identity_for_ref(target)
            .uses_effect_bridge_abi()
        {
            panic!(
                "codegen_lir_plain_static_interface_dispatch_call: effect boundary router accepted outward-effect static dispatch target in plain lowering at {span:?}"
            );
        }

        let ret_cg = self.try_cg_ty_of_type_id(impl_sig.return_ty).unwrap_or_else(|| {
            panic!(
                "codegen_lir_plain_static_interface_dispatch_call: LIR verifier accepted unsupported return type"
            )
        });
        let hidden_sret_result_ty = self.hidden_sret_result_ty(span, ret_cg)?;
        let hidden_sret_slot = if hidden_sret_result_ty.is_some() {
            Some(self.create_entry_alloca(span, "lir_static_iface_call_sret", ret_cg)?)
        } else {
            None
        };
        let direct_result_storage =
            if hidden_sret_result_ty.is_none() && !matches!(ret_cg, CgTy::Unit | CgTy::Never) {
                Some(self.create_entry_alloca(span, "lir_static_iface_call_result", ret_cg)?)
            } else {
                None
            };

        let source_cg = self.cg_ty_of_type_id(source_ty, "static LIR interface receiver type");
        let receiver_value =
            self.codegen_lir_operand_expected(span, receiver, slots, Some(source_cg))?;
        let receiver_value = self.coerce_value(span, receiver_value, source_cg)?;
        let receiver_arg = if self
            .ordinary_param_abi(span, source_ty)?
            .pointee_ty()
            .is_some()
        {
            let receiver_slot =
                self.create_entry_alloca(span, "lir_static_iface_receiver", source_cg)?;
            let _ = self.store_local_value(span, receiver_slot, source_cg, receiver_value)?;
            receiver_slot.into()
        } else {
            self.as_llvm_arg_value(span, source_cg, receiver_value)?
        };

        let explicit_param_names = impl_sig.param_names[1..].to_vec();
        let explicit_param_tys = impl_sig.param_tys[1..].to_vec();
        let evaluated_explicit_args = self.codegen_bound_lir_call_args_from_signature(
            span,
            &explicit_param_names,
            &explicit_param_tys,
            args,
            body,
            source_types,
            slots,
            false,
            self.types,
        )?;
        let explicit_args = evaluated_explicit_args
            .iter()
            .map(|arg| arg.value)
            .collect::<Vec<_>>();

        let llvm_name = self
            .exported_abi_symbol_for_lir_callable_ref(target)
            .unwrap_or_else(|_| impl_sig.fqn.clone());
        let function = self.declare_lir_plain_fun_with_symbol_for_ref(
            &llvm_name,
            LlvmFunctionDeclarationSurface::ExportedAbi,
            target,
            &impl_sig.fqn,
            &impl_sig.param_tys,
            impl_sig.return_ty,
            self.types,
            false,
        )?;
        let fn_i8 = function.as_global_value().as_pointer_value();
        self.emit_interface_dispatch_case_call_to_storage(
            span,
            span,
            &impl_sig.fqn,
            fn_i8,
            receiver_arg,
            source_ty,
            &explicit_param_tys,
            &explicit_args,
            ret_cg,
            hidden_sret_result_ty,
            hidden_sret_slot,
            false,
            None,
            direct_result_storage,
        )?;
        self.release_evaluated_call_arg_roots(&evaluated_explicit_args);

        if let Some(result_ptr) = hidden_sret_slot {
            self.sync_hidden_sret_result_roots(
                span,
                ret_cg,
                result_ptr,
                "lir_static_iface_call_sret",
            )?;
        }

        match ret_cg {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            _ => {
                if let Some(result_ptr) = hidden_sret_slot {
                    self.load_hidden_sret_result_from_ptr(
                        span,
                        ret_cg,
                        result_ptr,
                        "lir_static_iface_call_sret",
                    )
                } else {
                    self.load_dispatch_result_from_storage(
                        span,
                        ret_cg,
                        direct_result_storage.unwrap_or_else(|| {
                            std::panic::panic_any(
                                "codegen_lir_plain_static_interface_dispatch_call: direct return must publish result storage",
                            )
                        }),
                    )
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_mir_plain_dispatch_call(
        &mut self,
        span: crate::span::Span,
        receiver: &mir_source::Operand,
        args: &[mir_source::CallArg],
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        target: PlainDispatchTarget,
        allow_effect_typed_signature: bool,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let signature = target.signature();
        if signature.param_tys.len() != args.len() + 1 {
            std::panic::panic_any(
                "codegen_mir_plain_dispatch_call: MIR verifier accepted dispatch arity drift",
            );
        }
        if !allow_effect_typed_signature
            && self
                .direct_call_abi_identity_for_ref(signature.target.unwrap_or_else(|| {
                    panic!("codegen_mir_plain_dispatch_call: LIR dispatch signature missing callable target")
                }))
                .uses_effect_bridge_abi()
        {
            panic!(
                "codegen_mir_plain_dispatch_call: effect boundary router accepted outward-effect dispatch target in plain lowering at {span:?}"
            );
        }

        if let PlainDispatchTarget::Interface {
            interface_fqn,
            interface_id,
            slot,
            receiver_ty,
            ..
        } = &target
        {
            if let Some((source_ty, target, target_label)) =
                self.static_interface_dispatch_impl(mir_types, *receiver_ty, *interface_id, *slot)
            {
                return self.codegen_mir_plain_static_interface_dispatch_call(
                    span,
                    receiver,
                    args,
                    slots,
                    source_ty,
                    target,
                    &target_label,
                    allow_effect_typed_signature,
                );
            }

            let signature = self.instantiate_interface_dispatch_signature(signature, *receiver_ty);
            let receiver_value =
                self.codegen_mir_operand_expected(span, receiver, slots, Some(CgTy::Ref))?;
            let receiver_value = self.coerce_value(span, receiver_value, CgTy::Ref)?;
            let Some(BasicValueEnum::PointerValue(receiver_ptr)) = receiver_value.value else {
                panic!(
                    "codegen_mir_plain_dispatch_call: verifier accepted non-ref interface receiver"
                );
            };
            let deferred_receiver =
                self.defer_gc_ref_pointer(span, "plain_interface_receiver", receiver_ptr)?;
            let explicit_param_names = signature.param_names[1..].to_vec();
            let explicit_param_tys = signature.param_tys[1..].to_vec();
            let evaluated_explicit_args = self.codegen_bound_mir_call_args_from_signature(
                span,
                &explicit_param_names,
                &explicit_param_tys,
                args,
                slots,
                false,
                mir_types,
            )?;
            let explicit_args = evaluated_explicit_args
                .iter()
                .map(|arg| arg.value)
                .collect::<Vec<_>>();
            let receiver_ptr = self.reload_deferred_gc_ref_without_clearing(
                span,
                "plain_interface_receiver_reload",
                &deferred_receiver,
            )?;
            let lookup =
                self.lookup_interface_itable_slot(span, receiver_ptr, *interface_id, *slot)?;
            let result = self.emit_interface_dispatch_indirect_call(
                span,
                span,
                interface_fqn,
                *slot,
                &signature,
                false,
                receiver_ptr,
                lookup,
                &explicit_args,
            )?;
            self.release_evaluated_call_arg_roots(&evaluated_explicit_args);
            return Ok(result);
        }

        let ret_cg = self
            .try_cg_ty_of_type_id(signature.return_ty)
            .unwrap_or_else(|| {
                panic!(
                    "codegen_mir_plain_dispatch_call: MIR verifier accepted unsupported return type"
                )
            });
        let hidden_sret_result_ty = self.hidden_sret_result_ty(span, ret_cg)?;
        let mut llvm_param_tys: Vec<BasicMetadataTypeEnum<'ctx>> = Vec::with_capacity(
            signature.param_tys.len() + usize::from(hidden_sret_result_ty.is_some()),
        );
        if hidden_sret_result_ty.is_some() {
            llvm_param_tys.push(self.context.ptr_type(AddressSpace::default()).into());
        }
        for param_ty in &signature.param_tys {
            llvm_param_tys.push(self.ordinary_param_abi(span, *param_ty)?.llvm_param_ty());
        }
        let llvm_fun_ty = match (hidden_sret_result_ty, ret_cg) {
            (Some(_), _) | (None, CgTy::Unit | CgTy::Never) => {
                self.context.void_type().fn_type(&llvm_param_tys, false)
            }
            (None, other) => self
                .llvm_basic_type_of(span, other)?
                .fn_type(&llvm_param_tys, false),
        };

        let mut all_args = Vec::with_capacity(args.len() + 1);
        all_args.push(mir_source::CallArg {
            span,
            name: None,
            value: receiver.clone(),
        });
        all_args.extend(args.iter().cloned());
        let evaluated_args = self.codegen_bound_mir_call_args_from_signature(
            span,
            &signature.param_names,
            &signature.param_tys,
            &all_args,
            slots,
            false,
            mir_types,
        )?;
        let receiver_ptr = evaluated_args
            .first()
            .and_then(|arg| arg.pointer_value)
            .unwrap_or_else(|| panic!("codegen_mir_plain_dispatch_call: verifier accepted missing dispatch receiver pointer"));
        let deferred_receiver = self.defer_gc_ref_pointer(
            span,
            &format!("{}_receiver", target.label().replace(' ', "_")),
            receiver_ptr,
        )?;
        let mut llvm_args: Vec<BasicMetadataValueEnum<'ctx>> =
            Vec::with_capacity(evaluated_args.len() + usize::from(hidden_sret_result_ty.is_some()));
        let sret_result_slot = if let Some(result_ty) = hidden_sret_result_ty {
            let slot = self.create_entry_alloca(span, "plain_dispatch_sret", ret_cg)?;
            llvm_args.push(slot.into());
            Some((slot, result_ty))
        } else {
            None
        };
        llvm_args.extend(evaluated_args.iter().map(|arg| arg.value));

        let receiver_ptr = self.reload_deferred_gc_ref_without_clearing(
            span,
            "plain_dispatch_receiver_reload",
            &deferred_receiver,
        )?;
        let fn_i8 = match &target {
            PlainDispatchTarget::Virtual { slot, .. } => {
                self.load_class_vtable_slot_fn_ptr_i8(span, receiver_ptr, *slot)?
            }
            PlainDispatchTarget::Interface {
                interface_id, slot, ..
            } => {
                self.load_interface_itable_slot_fn_ptr_i8(span, receiver_ptr, *interface_id, *slot)?
            }
        };
        let typed_fn_ptr = self.builder.build_pointer_cast(
            fn_i8,
            self.llvm_ptr_type(AddressSpace::default()),
            "plain_dispatch_fn_typed",
        )?;
        let call_site_result = self.with_conservative_gc_local_root_spills(span, |cg| {
            let call_site = cg.builder.build_indirect_call(
                llvm_fun_ty,
                typed_fn_ptr,
                &llvm_args,
                "plain_dispatch_call",
            )?;
            if let Some((_, result_ty)) = sret_result_slot {
                cg.add_sret_attribute_to_call(call_site, 0, result_ty);
            }
            let target = signature.target.unwrap_or_else(|| {
                panic!("codegen_mir_plain_dispatch_call: LIR dispatch signature missing callable target")
            });
            call_site.set_call_convention(cg.llvm_call_convention_for_lir_callable_ref(target));
            Ok(call_site)
        });
        self.release_evaluated_call_arg_roots(&evaluated_args);
        let call_site = call_site_result?;
        if let Some((result_ptr, _)) = sret_result_slot {
            self.sync_hidden_sret_result_roots(span, ret_cg, result_ptr, "plain_dispatch_sret")?;
        }
        let deferred_direct_result = if sret_result_slot.is_none() {
            self.defer_direct_call_result(span, ret_cg, call_site, "plain_dispatch_direct_result")?
        } else {
            None
        };
        match ret_cg {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            _ => Ok(if let Some((result_ptr, _)) = sret_result_slot {
                self.load_hidden_sret_result_from_ptr(
                    span,
                    ret_cg,
                    result_ptr,
                    "plain_dispatch_sret",
                )?
            } else {
                self.materialize_deferred_cg_value(
                    span,
                    "plain_dispatch_direct_result_reload",
                    deferred_direct_result.unwrap_or_else(|| {
                        std::panic::panic_any(
                            "codegen_mir_plain_dispatch_call: direct return must publish deferred result",
                        )
                    }),
                )?
            }),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_lir_plain_dynamic_dispatch_call(
        &mut self,
        span: crate::span::Span,
        site_id: mir_source::SiteId,
        receiver: &LirOperand,
        args: &[LirCallArg],
        body: &LirExecutableBody,
        source_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        is_interface: bool,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let target = if is_interface {
            self.resolve_plain_interface_dispatch_target(site_id, args.len())?
        } else {
            self.resolve_plain_virtual_dispatch_target(site_id, args.len())?
        };
        self.codegen_lir_plain_dispatch_call(
            span,
            receiver,
            args,
            body,
            source_types,
            slots,
            target,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_lir_plain_dispatch_call(
        &mut self,
        span: crate::span::Span,
        receiver: &LirOperand,
        args: &[LirCallArg],
        body: &LirExecutableBody,
        source_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        target: PlainDispatchTarget,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let signature = target.signature();
        if signature.param_tys.len() != args.len() + 1 {
            std::panic::panic_any(
                "codegen_lir_plain_dispatch_call: LIR verifier accepted dispatch arity drift",
            );
        }
        if self
            .direct_call_abi_identity_for_ref(signature.target.unwrap_or_else(|| {
                panic!("codegen_lir_plain_dispatch_call: LIR dispatch signature missing callable target")
            }))
            .uses_effect_bridge_abi()
        {
            panic!(
                "codegen_lir_plain_dispatch_call: effect boundary router accepted outward-effect dispatch target in plain lowering at {span:?}"
            );
        }

        if let PlainDispatchTarget::Interface {
            interface_fqn,
            interface_id,
            slot,
            receiver_ty,
            ..
        } = &target
        {
            if let Some((source_ty, target, target_label)) = self.static_interface_dispatch_impl(
                source_types,
                *receiver_ty,
                *interface_id,
                *slot,
            ) {
                return self.codegen_lir_plain_static_interface_dispatch_call(
                    span,
                    receiver,
                    args,
                    body,
                    source_types,
                    slots,
                    source_ty,
                    target,
                    &target_label,
                );
            }

            let signature = self.instantiate_interface_dispatch_signature(signature, *receiver_ty);
            let receiver_value =
                self.codegen_lir_operand_expected(span, receiver, slots, Some(CgTy::Ref))?;
            let receiver_value = self.coerce_value(span, receiver_value, CgTy::Ref)?;
            let Some(BasicValueEnum::PointerValue(receiver_ptr)) = receiver_value.value else {
                panic!(
                    "codegen_lir_plain_dispatch_call: verifier accepted non-ref interface receiver"
                );
            };
            let deferred_receiver =
                self.defer_gc_ref_pointer(span, "lir_interface_receiver", receiver_ptr)?;
            let explicit_param_names = signature.param_names[1..].to_vec();
            let explicit_param_tys = signature.param_tys[1..].to_vec();
            let evaluated_explicit_args = self.codegen_bound_lir_call_args_from_signature(
                span,
                &explicit_param_names,
                &explicit_param_tys,
                args,
                body,
                source_types,
                slots,
                false,
                self.types,
            )?;
            let explicit_args = evaluated_explicit_args
                .iter()
                .map(|arg| arg.value)
                .collect::<Vec<_>>();
            let receiver_ptr = self.reload_deferred_gc_ref_without_clearing(
                span,
                "lir_interface_receiver_reload",
                &deferred_receiver,
            )?;
            let lookup =
                self.lookup_interface_itable_slot(span, receiver_ptr, *interface_id, *slot)?;
            let result = self.emit_interface_dispatch_indirect_call(
                span,
                span,
                interface_fqn,
                *slot,
                &signature,
                false,
                receiver_ptr,
                lookup,
                &explicit_args,
            )?;
            self.release_evaluated_call_arg_roots(&evaluated_explicit_args);
            return Ok(result);
        }

        let ret_cg = self
            .try_cg_ty_of_type_id(signature.return_ty)
            .unwrap_or_else(|| {
                panic!(
                    "codegen_lir_plain_dispatch_call: LIR verifier accepted unsupported return type"
                )
            });
        let hidden_sret_result_ty = self.hidden_sret_result_ty(span, ret_cg)?;
        let mut llvm_param_tys: Vec<BasicMetadataTypeEnum<'ctx>> = Vec::with_capacity(
            signature.param_tys.len() + usize::from(hidden_sret_result_ty.is_some()),
        );
        if hidden_sret_result_ty.is_some() {
            llvm_param_tys.push(self.context.ptr_type(AddressSpace::default()).into());
        }
        for param_ty in &signature.param_tys {
            llvm_param_tys.push(self.ordinary_param_abi(span, *param_ty)?.llvm_param_ty());
        }
        let llvm_fun_ty = match (hidden_sret_result_ty, ret_cg) {
            (Some(_), _) | (None, CgTy::Unit | CgTy::Never) => {
                self.context.void_type().fn_type(&llvm_param_tys, false)
            }
            (None, other) => self
                .llvm_basic_type_of(span, other)?
                .fn_type(&llvm_param_tys, false),
        };

        let mut all_args = Vec::with_capacity(args.len() + 1);
        all_args.push(LirCallArg {
            span,
            name: None,
            value: receiver.clone(),
        });
        all_args.extend(args.iter().cloned());
        let evaluated_args = self.codegen_bound_lir_call_args_from_signature(
            span,
            &signature.param_names,
            &signature.param_tys,
            &all_args,
            body,
            source_types,
            slots,
            false,
            self.types,
        )?;
        let receiver_ptr = evaluated_args
            .first()
            .and_then(|arg| arg.pointer_value)
            .unwrap_or_else(|| panic!("codegen_lir_plain_dispatch_call: verifier accepted missing dispatch receiver pointer"));
        let deferred_receiver = self.defer_gc_ref_pointer(
            span,
            &format!("{}_receiver", target.label().replace(' ', "_")),
            receiver_ptr,
        )?;
        let mut llvm_args: Vec<BasicMetadataValueEnum<'ctx>> =
            Vec::with_capacity(evaluated_args.len() + usize::from(hidden_sret_result_ty.is_some()));
        let sret_result_slot = if let Some(result_ty) = hidden_sret_result_ty {
            let slot = self.create_entry_alloca(span, "lir_dispatch_sret", ret_cg)?;
            llvm_args.push(slot.into());
            Some((slot, result_ty))
        } else {
            None
        };
        llvm_args.extend(evaluated_args.iter().map(|arg| arg.value));

        let receiver_ptr = self.reload_deferred_gc_ref_without_clearing(
            span,
            "lir_dispatch_receiver_reload",
            &deferred_receiver,
        )?;
        let fn_i8 = match &target {
            PlainDispatchTarget::Virtual { slot, .. } => {
                self.load_class_vtable_slot_fn_ptr_i8(span, receiver_ptr, *slot)?
            }
            PlainDispatchTarget::Interface {
                interface_id, slot, ..
            } => {
                self.load_interface_itable_slot_fn_ptr_i8(span, receiver_ptr, *interface_id, *slot)?
            }
        };
        let typed_fn_ptr = self.builder.build_pointer_cast(
            fn_i8,
            self.llvm_ptr_type(AddressSpace::default()),
            "lir_dispatch_fn_typed",
        )?;
        let call_site_result = self.with_conservative_gc_local_root_spills(span, |cg| {
            let call_site = cg.builder.build_indirect_call(
                llvm_fun_ty,
                typed_fn_ptr,
                &llvm_args,
                "lir_dispatch_call",
            )?;
            if let Some((_, result_ty)) = sret_result_slot {
                cg.add_sret_attribute_to_call(call_site, 0, result_ty);
            }
            let target = signature.target.unwrap_or_else(|| {
                panic!("codegen_lir_plain_dispatch_call: LIR dispatch signature missing callable target")
            });
            call_site.set_call_convention(cg.llvm_call_convention_for_lir_callable_ref(target));
            Ok(call_site)
        });
        self.release_evaluated_call_arg_roots(&evaluated_args);
        let call_site = call_site_result?;
        if let Some((result_ptr, _)) = sret_result_slot {
            self.sync_hidden_sret_result_roots(span, ret_cg, result_ptr, "lir_dispatch_sret")?;
        }
        let deferred_direct_result = if sret_result_slot.is_none() {
            self.defer_direct_call_result(span, ret_cg, call_site, "lir_dispatch_direct_result")?
        } else {
            None
        };
        match ret_cg {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            _ => Ok(if let Some((result_ptr, _)) = sret_result_slot {
                self.load_hidden_sret_result_from_ptr(
                    span,
                    ret_cg,
                    result_ptr,
                    "lir_dispatch_sret",
                )?
            } else {
                self.materialize_deferred_cg_value(
                    span,
                    "lir_dispatch_direct_result_reload",
                    deferred_direct_result.unwrap_or_else(|| {
                        std::panic::panic_any(
                            "codegen_lir_plain_dispatch_call: direct return must publish deferred result",
                        )
                    }),
                )?
            }),
        }
    }

    fn instantiate_interface_dispatch_signature(
        &self,
        signature: &CodegenCallableSignature,
        receiver_ty: TypeId,
    ) -> CodegenCallableSignature {
        let Some(receiver_param_ty) = signature.param_tys.first().copied() else {
            return signature.clone();
        };
        let TypeKind::Ref(RefTypeKind::Nominal(signature_receiver)) =
            self.types.kind(receiver_param_ty).clone()
        else {
            return signature.clone();
        };
        let TypeKind::Ref(RefTypeKind::Nominal(actual_receiver)) =
            self.types.kind(receiver_ty).clone()
        else {
            return signature.clone();
        };
        if signature_receiver.args.len() != actual_receiver.args.len() {
            return signature.clone();
        }

        let mut type_args = std::collections::HashMap::new();
        for (signature_arg, actual_arg) in signature_receiver
            .args
            .iter()
            .copied()
            .zip(actual_receiver.args.iter().copied())
        {
            if let TypeKind::Param(param) = self.types.kind(signature_arg) {
                type_args.insert(param.name.clone(), actual_arg);
            }
        }
        if type_args.is_empty() {
            return signature.clone();
        }

        CodegenCallableSignature {
            target: signature.target,
            fqn: signature.fqn.clone(),
            param_names: signature.param_names.clone(),
            param_tys: signature
                .param_tys
                .iter()
                .copied()
                .map(|ty| self.substitute_interface_dispatch_ty(ty, &type_args))
                .collect(),
            return_ty: self.substitute_interface_dispatch_ty(signature.return_ty, &type_args),
        }
    }

    fn substitute_interface_dispatch_ty(
        &self,
        ty: TypeId,
        type_args: &std::collections::HashMap<String, TypeId>,
    ) -> TypeId {
        match self.types.kind(ty).clone() {
            TypeKind::Param(param) => type_args.get(&param.name).copied().unwrap_or(ty),
            TypeKind::Ref(
                RefTypeKind::Any
                | RefTypeKind::String
                | RefTypeKind::Nominal(_)
                | RefTypeKind::Function(_)
                | RefTypeKind::Union(_),
            )
            | TypeKind::Value(
                ValueTypeKind::Unit
                | ValueTypeKind::Nothing
                | ValueTypeKind::Bool
                | ValueTypeKind::Char
                | ValueTypeKind::Float64
                | ValueTypeKind::Float32
                | ValueTypeKind::Int
                | ValueTypeKind::UInt
                | ValueTypeKind::IntN(_)
                | ValueTypeKind::UIntN(_)
                | ValueTypeKind::Nominal(_)
                | ValueTypeKind::Option(_)
                | ValueTypeKind::Tuple(_),
            )
            | TypeKind::StarProjection(_) => ty,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_mir_direct_call(
        &mut self,
        span: crate::span::Span,
        site_id: Option<mir_source::SiteId>,
        fqn: &str,
        args: &[mir_source::CallArg],
        body: &mir_source::Body,
        mir_types: &TypeStore,
        transport: &mir_source::CallTransportMetadata,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_mir_direct_call_with_policy(
            span,
            site_id,
            fqn,
            &[],
            args,
            transport,
            body,
            mir_types,
            slots,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_mir_direct_call_with_type_args(
        &mut self,
        span: crate::span::Span,
        site_id: mir_source::SiteId,
        fqn: &str,
        generic_type_args: &[TypeId],
        args: &[mir_source::CallArg],
        body: &mir_source::Body,
        mir_types: &TypeStore,
        transport: &mir_source::CallTransportMetadata,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_mir_direct_call_with_policy(
            span,
            Some(site_id),
            fqn,
            generic_type_args,
            args,
            transport,
            body,
            mir_types,
            slots,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_mir_plain_direct_call(
        &mut self,
        span: crate::span::Span,
        site_id: Option<mir_source::SiteId>,
        fqn: &str,
        args: &[mir_source::CallArg],
        body: &mir_source::Body,
        mir_types: &TypeStore,
        transport: &mir_source::CallTransportMetadata,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_mir_direct_call_with_policy(
            span,
            site_id,
            fqn,
            &[],
            args,
            transport,
            body,
            mir_types,
            slots,
            true,
        )
    }
    pub(in crate::llvm::codegen) fn selected_mir_class_ctor_from_contract<'b>(
        &self,
        _span: crate::span::Span,
        class: &'b hir::MonoClassInit,
        ctor: &mir_source::ClassCtorCallMetadata,
        args: &[mir_source::CallArg],
        _kind: &'static str,
    ) -> Result<Option<&'b hir::ClassCtor<MonoTypeId>>, LlvmEmitError> {
        if args.iter().any(|arg| arg.name.is_some()) || args.len() != ctor.ordered_param_count {
            std::panic::panic_any(
                "selected_mir_class_ctor_from_contract: MIR verifier accepted constructor argument drift",
            );
        }

        let selected_ctor = match ctor.selected_ctor_span {
            Some(selected_span) => Some(
                class
                    .ctors
                    .iter()
                    .find(|candidate| candidate.span == selected_span)
                    .unwrap_or_else(|| panic!("selected_mir_class_ctor_from_contract: verifier accepted missing selected ctor")),
            ),
            None if class.ctors.is_empty() => None,
            None => {
                panic!("selected_mir_class_ctor_from_contract: verifier accepted unselected ctor contract");
            }
        };

        let param_count = selected_ctor.map_or(0, |ctor| ctor.params.len());
        if param_count != args.len() {
            std::panic::panic_any(
                "selected_mir_class_ctor_from_contract: selected ctor arity must match lowered args",
            );
        }

        Ok(selected_ctor)
    }
}
