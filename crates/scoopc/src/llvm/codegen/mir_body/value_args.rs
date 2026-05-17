//! Callable / funptr value-args lowering and closure-env / value-box type metadata.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn codegen_mir_callable_value_args(
        &mut self,
        span: crate::span::Span,
        fun_ty: &crate::ty::FunctionType,
        args: &[crate::mir::CallArg],
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<Vec<EvaluatedCallArg<'ctx>>, LlvmEmitError> {
        let param_names = self.callable_value_param_names(fun_ty);
        let param_tys = self.callable_value_param_tys(fun_ty);
        let arg_to_param = map_mir_call_args_to_param_names(&param_names, args).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR closure call arg binding",
                at: span.into(),
            },
        )?;

        let mut evaluated: Vec<Option<(crate::span::Span, DeferredCgValue<'ctx>)>> =
            vec![None; param_tys.len()];
        for (arg_idx, arg) in args.iter().enumerate() {
            let param_idx = arg_to_param[arg_idx];
            let target_cg =
                self.cg_ty_of(param_tys[param_idx])
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR closure call arg type",
                        at: arg.span.into(),
                    })?;
            let value =
                self.codegen_mir_operand_expected(arg.span, &arg.value, slots, Some(target_cg))?;
            let coerced = self.coerce_value(arg.span, value, target_cg)?;
            let deferred = self.defer_gc_sensitive_cg_value(
                arg.span,
                &format!("pass_mir_closure_arg_{param_idx}"),
                coerced,
            )?;
            evaluated[param_idx] = Some((arg.span, deferred));
        }

        evaluated
            .into_iter()
            .enumerate()
            .map(|(param_idx, slot)| {
                let (arg_span, deferred) = slot.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR closure call arg binding",
                    at: span.into(),
                })?;
                let param_ty = param_tys[param_idx];
                let param_abi = self.ordinary_param_abi(span, param_ty)?;
                if param_abi.pointee_ty().is_some() {
                    let (slot_ptr, cleanup_spills) = self.deferred_gc_spill_slot_for_call_arg(
                        arg_span,
                        &format!("pass_mir_closure_arg_reload_{param_idx}"),
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
                        &format!("pass_mir_closure_arg_reload_{param_idx}"),
                        deferred,
                    )?;
                let pointer_value = match materialized.value {
                    Some(inkwell::values::BasicValueEnum::PointerValue(ptr)) => Some(ptr),
                    _ => None,
                };
                let value = self.as_llvm_arg_value(arg_span, param_abi.cg_ty(), materialized)?;
                Ok(EvaluatedCallArg {
                    value,
                    pointer_value,
                    cleanup_spills,
                })
            })
            .collect()
    }

    pub(in crate::llvm::codegen) fn codegen_mir_funptr_value_args(
        &mut self,
        span: crate::span::Span,
        fun_ty: &crate::ty::FunctionType,
        args: &[crate::mir::CallArg],
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<Vec<EvaluatedCallArg<'ctx>>, LlvmEmitError> {
        let param_names = self.callable_value_param_names(fun_ty);
        let param_tys = self.callable_value_param_tys(fun_ty);
        let arg_to_param = map_mir_call_args_to_param_names(&param_names, args).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR FunPtr call arg binding",
                at: span.into(),
            },
        )?;

        let mut evaluated: Vec<Option<(crate::span::Span, DeferredCgValue<'ctx>)>> =
            vec![None; param_tys.len()];
        for (arg_idx, arg) in args.iter().enumerate() {
            let param_idx = arg_to_param[arg_idx];
            let target_cg = self
                .cg_ty_of_mir_type(mir_types, param_tys[param_idx])
                .or_else(|| self.cg_ty_of(param_tys[param_idx]))
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR FunPtr call arg type",
                    at: arg.span.into(),
                })?;
            let value =
                self.codegen_mir_operand_expected(arg.span, &arg.value, slots, Some(target_cg))?;
            let coerced = self.coerce_value(arg.span, value, target_cg)?;
            let deferred = self.defer_gc_sensitive_cg_value(
                arg.span,
                &format!("pass_mir_funptr_arg_{param_idx}"),
                coerced,
            )?;
            evaluated[param_idx] = Some((arg.span, deferred));
        }

        evaluated
            .into_iter()
            .enumerate()
            .map(|(param_idx, slot)| {
                let (arg_span, deferred) = slot.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR FunPtr call arg binding",
                    at: span.into(),
                })?;
                let (materialized, cleanup_spills) = self
                    .materialize_deferred_cg_value_for_call_arg(
                        arg_span,
                        &format!("pass_mir_funptr_arg_reload_{param_idx}"),
                        deferred,
                    )?;
                let pointer_value = match materialized.value {
                    Some(BasicValueEnum::PointerValue(ptr)) => Some(ptr),
                    _ => None,
                };
                let value = self.as_llvm_arg_value(arg_span, materialized.ty, materialized)?;
                Ok(EvaluatedCallArg {
                    value,
                    pointer_value,
                    cleanup_spills,
                })
            })
            .collect()
    }

    pub(in crate::llvm::codegen) fn mir_closure_env_object_type(
        &mut self,
        at: crate::span::Span,
        closure_key: &StableClosureKey,
        field_cgs: &[CgTy],
    ) -> Result<StructType<'ctx>, LlvmEmitError> {
        let name = private_closure_env_type_name(closure_key);
        if let Some(existing) = self.context.get_struct_type(&name) {
            return Ok(existing);
        }
        let env_ty = self.context.opaque_struct_type(&name);
        let mut fields: Vec<BasicTypeEnum<'ctx>> = Vec::with_capacity(1 + field_cgs.len());
        fields.push(self.llvm_gc_object_header_type().into());
        for cg in field_cgs {
            fields.push(self.llvm_basic_type_of(at, *cg)?);
        }
        env_ty.set_body(&fields, false);
        Ok(env_ty)
    }

    pub(in crate::llvm::codegen) fn mir_value_box_object_type(
        &mut self,
        at: crate::span::Span,
        source_ty: TypeId,
        source_cg: CgTy,
    ) -> Result<StructType<'ctx>, LlvmEmitError> {
        let key = CanonicalTextKey::new(
            self.canonical_type_key_text_for_codegen(source_ty, "MIR value box LLVM type")?,
        );
        let name = PrivateSymbolMangler.type_name("MirValueBox", "mir_value_box_type", &key);
        if let Some(existing) = self.context.get_struct_type(&name) {
            return Ok(existing);
        }
        let box_ty = self.context.opaque_struct_type(&name);
        let fields = [
            self.llvm_gc_object_header_type().into(),
            self.llvm_basic_type_of(at, source_cg)?,
        ];
        box_ty.set_body(&fields, false);
        Ok(box_ty)
    }

    pub(in crate::llvm::codegen) fn get_or_create_mir_closure_env_type_desc_global(
        &mut self,
        at: crate::span::Span,
        closure_key: &StableClosureKey,
        env_ty: StructType<'ctx>,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        let global_name = private_closure_env_type_desc_name(closure_key);
        if let Some(existing) = self.module.get_global(&global_name) {
            return Ok(existing);
        }
        let trace_start_offset_bytes = self.target_data.offset_of_element(&env_ty, 1).unwrap_or(0);
        let canonical_name = closure_key.env_canonical_name();
        self.get_or_create_type_descriptor_global(TypeDescriptorSpec {
            at,
            global_name: &global_name,
            type_id_key: &canonical_name,
            obj_ty: env_ty,
            trace_start_offset_bytes,
            parent: None,
            itable: None,
            vtable: None,
        })
    }

    pub(in crate::llvm::codegen) fn get_or_create_mir_value_box_type_desc_global(
        &mut self,
        at: crate::span::Span,
        source_ty: TypeId,
        box_ty: StructType<'ctx>,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        let base_type_key =
            self.canonical_type_key_text_for_codegen(source_ty, "MIR value box type descriptor")?;
        let key = CanonicalTextKey::new(base_type_key.clone());
        let global_name = PrivateSymbolMangler.mangle("mir_value_box_type_desc", &key);
        if let Some(existing) = self.module.get_global(&global_name) {
            return Ok(existing);
        }
        let trace_start_offset_bytes = self.target_data.offset_of_element(&box_ty, 1).unwrap_or(0);
        let type_id_key = stable_rtti_derived_type_key("mir_value_box_type_desc", &base_type_key);
        let itable = self
            .get_or_create_mir_value_box_itable_global(at, source_ty)?
            .map(|gv| gv.as_pointer_value().const_cast(self.llvm_i8_ptr_type()));
        self.get_or_create_type_descriptor_global(TypeDescriptorSpec {
            at,
            global_name: &global_name,
            type_id_key: type_id_key.as_str(),
            obj_ty: box_ty,
            trace_start_offset_bytes,
            parent: None,
            itable,
            vtable: None,
        })
    }

    pub(in crate::llvm::codegen) fn get_or_create_mir_value_box_itable_global(
        &mut self,
        at: crate::span::Span,
        source_ty: TypeId,
    ) -> Result<Option<GlobalValue<'ctx>>, LlvmEmitError> {
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(source_ty) else {
            return Ok(None);
        };
        if !self
            .struct_layouts
            .contains_key(&self.nominal_layout_key(nominal))
        {
            return Ok(None);
        }
        let entries = self.mir_value_box_itable_entries(source_ty)?;
        if entries.is_empty() {
            return Ok(None);
        }
        let owner_key = CanonicalTextKey::new(canonical_record(
            "mir_value_box_itable_owner",
            [self.canonical_type_key_text_for_codegen(source_ty, "MIR value box itable owner")?],
        ));
        self.get_or_create_itable_global_from_entries(at, &owner_key, &entries)
    }

    pub(in crate::llvm::codegen) fn materialized_value_box_member_impl_fqn(
        &self,
        source_ty: TypeId,
        impl_member_fqn: &str,
    ) -> String {
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(source_ty) else {
            return impl_member_fqn.to_string();
        };
        let Some((owner_fqn, _)) = impl_member_fqn.rsplit_once('.') else {
            return impl_member_fqn.to_string();
        };
        if nominal.fqn != owner_fqn || nominal.args.is_empty() {
            return impl_member_fqn.to_string();
        }
        let Some(template_fun) = self.fun_index.get(impl_member_fqn).copied() else {
            return impl_member_fqn.to_string();
        };
        let template = crate::mir::TemplateKey {
            fqn: impl_member_fqn.to_string(),
            source_path: template_fun.source_path.clone(),
            decl_span: template_fun.span,
        };
        crate::hir::stable_instance_fqn(self.types, &template, &nominal.args, &[], "")
    }

    pub(in crate::llvm::codegen) fn mir_value_box_itable_entries(
        &self,
        source_ty: TypeId,
    ) -> Result<Vec<crate::itable::ClassItableEntry>, LlvmEmitError> {
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(source_ty) else {
            return Ok(Vec::new());
        };
        let mut interfaces = Vec::new();
        let mut visiting = HashSet::new();
        self.collect_mir_value_box_interfaces(&nominal.fqn, &mut interfaces, &mut visiting);
        interfaces
            .into_iter()
            .map(|interface_fqn| {
                let iface = self.interfaces.get(&interface_fqn).ok_or_else(|| {
                    frontend_error(format!(
                        "value box interface `{interface_fqn}` missing interface metadata"
                    ))
                })?;
                let mut method_impl_fqns = Vec::with_capacity(iface.method_slots.len());
                let value_receiver_type_id = self
                    .stable_rtti_type_id_for_codegen(source_ty, "MIR value box receiver RTTI")
                    .map_err(|err| {
                        frontend_error(format!(
                            "MIR value box `{}` 无法构造 receiver stable RTTI type id: {err}",
                            self.types.display(source_ty)
                        ))
                    })?;
                let mut method_receiver_type_ids = Vec::with_capacity(iface.method_slots.len());
                for slot in &iface.method_slots {
                    let impl_fqn = self.materialized_value_box_member_impl_fqn(
                        source_ty,
                        &format!("{}.{}", nominal.fqn, slot.name),
                    );
                    if self.fun_index.contains_key(impl_fqn.as_str()) {
                        method_impl_fqns.push(impl_fqn);
                        method_receiver_type_ids.push(value_receiver_type_id);
                    } else if slot.has_body {
                        method_impl_fqns.push(slot.member_fqn.clone());
                        method_receiver_type_ids.push(crate::itable::ITABLE_RECEIVER_REF_TYPE_ID);
                    } else {
                        return Err(frontend_error(format!(
                            "value box `{}` missing implementation for interface method `{}`",
                            nominal.fqn, slot.member_fqn
                        )));
                    }
                }
                let interface_type_name = iface.fqn.clone();
                let interface_ty =
                    self.types
                        .find_nominal_ref_by_fqn(&iface.fqn)
                        .ok_or_else(|| {
                            frontend_error(format!(
                                "MIR value box interface `{}` missing nominal TypeId",
                                iface.fqn
                            ))
                        })?;
                let interface_type_id = self
                    .stable_rtti_type_id_for_codegen(interface_ty, "MIR value box interface RTTI")
                    .map_err(|err| {
                        frontend_error(format!(
                            "MIR value box interface `{}` 无法构造 stable RTTI type id: {err}",
                            iface.fqn
                        ))
                    })?;
                Ok(crate::itable::ClassItableEntry {
                    interface_fqn: iface.fqn.clone(),
                    interface_id: iface.interface_id,
                    interface_type_name: interface_type_name.clone(),
                    interface_type_id,
                    runtime_match_type_names: vec![interface_type_name],
                    runtime_match_type_ids: vec![interface_type_id],
                    method_impl_fqns,
                    method_receiver_type_ids,
                })
            })
            .collect()
    }

    pub(in crate::llvm::codegen) fn collect_mir_value_box_interfaces(
        &self,
        fqn: &str,
        out: &mut Vec<String>,
        visiting: &mut HashSet<String>,
    ) {
        if !visiting.insert(fqn.to_string()) {
            return;
        }
        if let Some(supertypes) = self.direct_supertypes.get(fqn) {
            for super_fqn in supertypes {
                if self.interfaces.contains_key(super_fqn) && !out.contains(super_fqn) {
                    out.push(super_fqn.clone());
                }
                self.collect_mir_value_box_interfaces(super_fqn, out, visiting);
            }
        }
        visiting.remove(fqn);
    }
}
