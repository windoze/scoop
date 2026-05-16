//! Plain virtual / interface dispatch resolution and dispatch-call lowering.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn resolve_plain_virtual_dispatch_target(
        &self,
        dispatch: &crate::mir::DispatchMetadata,
        explicit_arg_count: usize,
    ) -> Result<PlainDispatchTarget<'a>, LlvmEmitError> {
        let slots = self.class_vtables.get(&dispatch.owner_fqn).ok_or_else(|| {
            frontend_error(format!(
                "plain virtual call 缺少 `{}` 的 class vtable metadata",
                dispatch.owner_fqn,
            ))
        })?;
        let mut candidates = slots.iter().filter(|slot| {
            slot.name == dispatch.member_name && slot.params_len == explicit_arg_count as u32
        });
        let slot = candidates.next().ok_or_else(|| {
            frontend_error(format!(
                "plain virtual call 缺少 `{}`.`{}`/{} 的 vtable slot",
                dispatch.owner_fqn, dispatch.member_name, explicit_arg_count,
            ))
        })?;
        if candidates.next().is_some() {
            return Err(frontend_error(format!(
                "plain virtual call `{}`.`{}`/{} 的 vtable slot 多义",
                dispatch.owner_fqn, dispatch.member_name, explicit_arg_count,
            )));
        }
        let sig_fun = self
            .fun_index
            .get(slot.impl_member_fqn.as_str())
            .copied()
            .ok_or_else(|| {
                frontend_error(format!(
                    "plain virtual call 缺少 target `{}` 的 signature",
                    slot.impl_member_fqn,
                ))
            })?;
        Ok(PlainDispatchTarget::Virtual {
            slot: slot.slot,
            sig_fun,
        })
    }

    pub(in crate::llvm::codegen) fn resolve_plain_interface_dispatch_target(
        &self,
        dispatch: &crate::mir::DispatchMetadata,
        explicit_arg_count: usize,
    ) -> Result<PlainDispatchTarget<'a>, LlvmEmitError> {
        let iface = self.interfaces.get(&dispatch.owner_fqn).ok_or_else(|| {
            frontend_error(format!(
                "plain interface call 缺少 `{}` 的 interface metadata",
                dispatch.owner_fqn,
            ))
        })?;
        let mut slots = iface.method_slots.iter().filter(|slot| {
            slot.member_fqn == dispatch.member_fqn && slot.params_len == explicit_arg_count as u32
        });
        let slot = slots.next().ok_or_else(|| {
            frontend_error(format!(
                "plain interface call 缺少 `{}` 的 selected itable slot",
                dispatch.member_fqn,
            ))
        })?;
        if slots.next().is_some() {
            return Err(frontend_error(format!(
                "plain interface call `{}` 的 selected itable slot 多义",
                dispatch.member_fqn,
            )));
        }

        let sig_fun = self
            .fun_index
            .get(dispatch.member_fqn.as_str())
            .copied()
            .ok_or_else(|| {
                frontend_error(format!(
                    "plain interface call 缺少 `{}` 的 selected signature",
                    dispatch.member_fqn,
                ))
            })?;
        Ok(PlainDispatchTarget::Interface {
            interface_id: iface.interface_id,
            slot: slot.slot,
            receiver_ty: dispatch.receiver_ty,
            sig_fun,
        })
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
    ) -> Option<(TypeId, String)> {
        let (owner_fqn, source_ty) =
            self.static_interface_receiver_owner_fqn(mir_types, receiver_ty)?;
        let entries = self.class_itables.get(&owner_fqn)?;
        let entry = entries
            .iter()
            .find(|entry| entry.interface_id == interface_id)?;
        let impl_fqn = entry.method_impl_fqns.get(slot as usize)?.clone();
        (!impl_fqn.is_empty()).then_some((source_ty, impl_fqn))
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_mir_plain_static_interface_dispatch_call(
        &mut self,
        span: crate::span::Span,
        receiver: &crate::mir::Operand,
        args: &[crate::mir::CallArg],
        slots: &[MirLocalSlot<'ctx>],
        source_ty: TypeId,
        impl_fqn: &str,
        allow_effect_typed_signature: bool,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let impl_sig = self.fun_index.get(impl_fqn).copied().ok_or_else(|| {
            frontend_error(format!(
                "static interface dispatch target `{impl_fqn}` missing signature"
            ))
        })?;
        if impl_sig.params.len() != args.len() + 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "static interface dispatch arity mismatch",
                at: span.into(),
            });
        }
        if !allow_effect_typed_signature
            && self.known_fun_body_may_outward_effect(&impl_sig.fqn, impl_sig.ty)
        {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "static interface dispatch target may outward-effect",
                at: span.into(),
            });
        }

        let ret_cg =
            self.cg_ty_of(impl_sig.return_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "static interface dispatch return type",
                    at: span.into(),
                })?;
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

        let source_cg = self
            .cg_ty_of(source_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "static interface receiver type",
                at: span.into(),
            })?;
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

        let explicit_param_names = impl_sig.params[1..]
            .iter()
            .map(|param| param.name.clone())
            .collect::<Vec<_>>();
        let explicit_param_tys = impl_sig.params[1..]
            .iter()
            .map(|param| param.ty)
            .collect::<Vec<_>>();
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

        let function = self.declare_top_level_fun(impl_sig)?;
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
                        direct_result_storage.ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "static interface direct result storage",
                            at: span.into(),
                        })?,
                    )
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_mir_plain_dispatch_call(
        &mut self,
        span: crate::span::Span,
        receiver: &crate::mir::Operand,
        args: &[crate::mir::CallArg],
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        target: PlainDispatchTarget<'a>,
        allow_effect_typed_signature: bool,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let sig_fun = target.sig_fun();
        if sig_fun.params.len() != args.len() + 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "plain dispatch arity mismatch",
                at: span.into(),
            });
        }
        if !allow_effect_typed_signature
            && self.known_fun_body_may_outward_effect(&sig_fun.fqn, sig_fun.ty)
        {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "plain dispatch target may outward-effect",
                at: span.into(),
            });
        }

        if let PlainDispatchTarget::Interface {
            interface_id,
            slot,
            receiver_ty,
            ..
        } = &target
        {
            if let Some((source_ty, impl_fqn)) =
                self.static_interface_dispatch_impl(mir_types, *receiver_ty, *interface_id, *slot)
            {
                return self.codegen_mir_plain_static_interface_dispatch_call(
                    span,
                    receiver,
                    args,
                    slots,
                    source_ty,
                    &impl_fqn,
                    allow_effect_typed_signature,
                );
            }

            let interface_fqn = sig_fun.fqn.rsplit_once('.').map(|(owner, _)| owner).ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "plain interface owner fqn",
                    at: span.into(),
                },
            )?;
            let receiver_value =
                self.codegen_mir_operand_expected(span, receiver, slots, Some(CgTy::Ref))?;
            let receiver_value = self.coerce_value(span, receiver_value, CgTy::Ref)?;
            let Some(BasicValueEnum::PointerValue(receiver_ptr)) = receiver_value.value else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "plain interface receiver value",
                    at: span.into(),
                });
            };
            let deferred_receiver =
                self.defer_gc_ref_pointer(span, "plain_interface_receiver", receiver_ptr)?;
            let explicit_param_names = sig_fun.params[1..]
                .iter()
                .map(|param| param.name.clone())
                .collect::<Vec<_>>();
            let explicit_param_tys = sig_fun.params[1..]
                .iter()
                .map(|param| param.ty)
                .collect::<Vec<_>>();
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
                sig_fun,
                false,
                receiver_ptr,
                lookup,
                &explicit_args,
            )?;
            self.release_evaluated_call_arg_roots(&evaluated_explicit_args);
            return Ok(result);
        }

        let ret_cg =
            self.cg_ty_of(sig_fun.return_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "plain dispatch return type",
                    at: span.into(),
                })?;
        let hidden_sret_result_ty = self.hidden_sret_result_ty(span, ret_cg)?;
        let mut llvm_param_tys: Vec<BasicMetadataTypeEnum<'ctx>> =
            Vec::with_capacity(sig_fun.params.len() + usize::from(hidden_sret_result_ty.is_some()));
        if hidden_sret_result_ty.is_some() {
            llvm_param_tys.push(self.context.ptr_type(AddressSpace::default()).into());
        }
        for param in &sig_fun.params {
            llvm_param_tys.push(self.ordinary_param_abi(span, param.ty)?.llvm_param_ty());
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
        all_args.push(crate::mir::CallArg {
            span,
            name: None,
            value: receiver.clone(),
        });
        all_args.extend(args.iter().cloned());
        let evaluated_args =
            self.codegen_bound_mir_call_args(span, sig_fun, &all_args, slots, false)?;
        let receiver_ptr = evaluated_args
            .first()
            .and_then(|arg| arg.pointer_value)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "plain dispatch receiver value",
                at: span.into(),
            })?;
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
        let fn_i8 = match target {
            PlainDispatchTarget::Virtual { slot, .. } => {
                self.load_class_vtable_slot_fn_ptr_i8(span, receiver_ptr, slot)?
            }
            PlainDispatchTarget::Interface {
                interface_id, slot, ..
            } => {
                self.load_interface_itable_slot_fn_ptr_i8(span, receiver_ptr, interface_id, slot)?
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
            call_site.set_call_convention(cg.llvm_call_convention_for_fqn(&sig_fun.fqn));
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
                    deferred_direct_result.ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "plain dispatch deferred return value",
                        at: span.into(),
                    })?,
                )?
            }),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_mir_direct_call(
        &mut self,
        span: crate::span::Span,
        fqn: &str,
        args: &[crate::mir::CallArg],
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        transport: &crate::mir::CallTransportMetadata,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_mir_direct_call_with_policy(
            span, fqn, args, transport, body, mir_types, slots, false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_mir_plain_direct_call(
        &mut self,
        span: crate::span::Span,
        fqn: &str,
        args: &[crate::mir::CallArg],
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        transport: &crate::mir::CallTransportMetadata,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_mir_direct_call_with_policy(
            span, fqn, args, transport, body, mir_types, slots, true,
        )
    }
    pub(in crate::llvm::codegen) fn selected_mir_class_ctor_from_contract<'b>(
        &self,
        span: crate::span::Span,
        class: &'b hir::ClassInit,
        ctor: &crate::mir::ClassCtorCallMetadata,
        args: &[crate::mir::CallArg],
        kind: &'static str,
    ) -> Result<Option<&'b hir::ClassCtor>, LlvmEmitError> {
        if args.iter().any(|arg| arg.name.is_some()) || args.len() != ctor.ordered_param_count {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind,
                at: span.into(),
            });
        }

        let selected_ctor = match ctor.selected_ctor_span {
            Some(selected_span) => Some(
                class
                    .ctors
                    .iter()
                    .find(|candidate| candidate.span == selected_span)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "class ctor selected ctor contract",
                        at: span.into(),
                    })?,
            ),
            None if class.ctors.is_empty() => None,
            None => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "class ctor selected ctor contract",
                    at: span.into(),
                });
            }
        };

        let param_count = selected_ctor.map_or(0, |ctor| ctor.params.len());
        if param_count != args.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind,
                at: span.into(),
            });
        }

        Ok(selected_ctor)
    }
}
