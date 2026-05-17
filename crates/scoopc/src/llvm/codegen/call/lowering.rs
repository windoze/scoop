//! Direct/static/dynamic call lowering rebuilt on top of the target-shape ABI.

use std::collections::HashSet;

use super::super::*;
use inkwell::AddressSpace;
use inkwell::IntPredicate;
use inkwell::types::{BasicMetadataTypeEnum, FunctionType, StructType};
use inkwell::values::{
    BasicMetadataValueEnum, BasicValueEnum, FunctionValue, IntValue, PointerValue,
};

use crate::hir;
use crate::ty::{RefTypeKind, TypeId, TypeKind, ValueTypeKind};

/// direct-call target 已在 HIR 中物化为 `foo::<Bar>` 时，返回其模板 FQN `foo`。
fn direct_call_dispatch_fqn(fqn: &str) -> &str {
    if let Some((base, _)) = fqn.rsplit_once("::<") {
        return base;
    }

    fqn.split_once("$overload$")
        .map(|(base, _)| base)
        .unwrap_or(fqn)
}

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    fn try_codegen_builtin_member_call_short_circuit(
        &mut self,
        span: crate::span::Span,
        callee: &hir::Expr,
        args: &[hir::CallArg],
        expected: Option<CgTy>,
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        let hir::ExprKind::MemberAccess { receiver, member } = &callee.kind else {
            return Ok(None);
        };
        let member_fun_fqn = match member.resolved.as_ref() {
            Some(hir::MemberRef::Fun { fqn, .. } | hir::MemberRef::ExtensionFun { fqn, .. }) => {
                Some(fqn.as_str())
            }
            Some(hir::MemberRef::Value { .. } | hir::MemberRef::ExtensionValue { .. }) | None => {
                None
            }
        };

        if let Some(fqn) = member_fun_fqn {
            if fqn == "scoop.core.GC.handleNew" {
                return self
                    .codegen_sysroot_gc_handle_new(span, member.span, args, expected)
                    .map(Some);
            }
            if fqn == "scoop.core.GC.handleGet" {
                return self
                    .codegen_sysroot_gc_handle_get(span, member.span, args)
                    .map(Some);
            }
            if fqn == "scoop.core.GC.handleDrop" {
                return self
                    .codegen_sysroot_gc_handle_drop(span, member.span, args)
                    .map(Some);
            }
            if fqn == "scoop.core.GC.pin" {
                return self
                    .codegen_sysroot_gc_pin(span, member.span, args, expected)
                    .map(Some);
            }
            if fqn == "scoop.core.GC.unpin" {
                return self
                    .codegen_sysroot_gc_unpin(span, member.span, args)
                    .map(Some);
            }
        }

        if let Some(entry_name) = self
            .current_top_level_fun_call_binding(span)?
            .filter(|binding| {
                member_fun_fqn.is_none_or(|fqn| {
                    direct_call_dispatch_fqn(&binding.fqn) == direct_call_dispatch_fqn(fqn)
                })
            })
            .and_then(|binding| binding.intrinsic_entry_name.clone())
            .or_else(|| {
                member_fun_fqn
                    .and_then(crate::intrinsics::fallback_named_intrinsic_entry_name_for_fqn)
                    .map(str::to_string)
            })
        {
            return self.try_codegen_named_intrinsic_hir_call(
                span,
                callee.span,
                callee,
                args,
                &entry_name,
            );
        }

        if member.name == "toInt" {
            let recv_ty = match &receiver.kind {
                hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) => self
                    .function_cx
                    .env
                    .get(*id)
                    .and_then(|local| local.hir_ty)
                    .unwrap_or(receiver.ty),
                _ => receiver.ty,
            };
            if matches!(
                self.types.kind(recv_ty),
                TypeKind::Value(ValueTypeKind::Char)
            ) {
                return self.codegen_char_method_to_int(receiver).map(Some);
            }
            if matches!(
                self.types.kind(recv_ty),
                TypeKind::Value(ValueTypeKind::Float64 | ValueTypeKind::Float32)
            ) {
                let recv = self.codegen_expr(receiver)?;
                return self
                    .codegen_float_to_int_value(span, receiver.span, recv)
                    .map(Some);
            }
        }
        if matches!(member.name.as_str(), "byteLength" | "getByte") {
            return self
                .codegen_string_method(span, receiver, &member.name, args)
                .map(Some);
        }
        if member.name == "hash" {
            let recv_ty = match &receiver.kind {
                hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) => self
                    .function_cx
                    .env
                    .get(*id)
                    .and_then(|local| local.hir_ty)
                    .unwrap_or(receiver.ty),
                _ => receiver.ty,
            };
            return match self.types.kind(recv_ty) {
                TypeKind::Value(ValueTypeKind::Char) => {
                    self.codegen_char_method_hash(span, receiver).map(Some)
                }
                TypeKind::Value(ValueTypeKind::Int) => {
                    self.codegen_int_method_hash(span, receiver).map(Some)
                }
                TypeKind::Value(ValueTypeKind::Float64 | ValueTypeKind::Float32) => {
                    let recv = self.codegen_expr(receiver)?;
                    self.codegen_float_hash_value(receiver.span, recv).map(Some)
                }
                _ => Ok(None),
            };
        }
        if matches!(member.name.as_str(), "abs" | "isNaN" | "isInfinite") {
            let recv = self.codegen_expr(receiver)?;
            return match member.name.as_str() {
                "abs" => self.codegen_float_abs_value(receiver.span, recv).map(Some),
                "isNaN" => self
                    .codegen_float_is_nan_value(receiver.span, recv)
                    .map(Some),
                "isInfinite" => self
                    .codegen_float_is_infinite_value(receiver.span, recv)
                    .map(Some),
                _ => unreachable!("filtered by matches!"),
            };
        }

        Ok(None)
    }

    pub(in crate::llvm::codegen) fn load_class_vtable_slot_fn_ptr_i8_impl(
        &mut self,
        _at: crate::span::Span,
        receiver: PointerValue<'ctx>,
        slot: u32,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let i32_ty = self.context.i32_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();

        let header_ty = self.llvm_gc_object_header_type();
        let header_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let header_ptr =
            self.builder
                .build_pointer_cast(receiver, header_ptr_ty, "vtable_hdr_ptr")?;

        let type_desc_ptr =
            self.builder
                .build_struct_gep(header_ty, header_ptr, 1, "vtable_type_desc_gep")?;
        let type_desc_i8 = self
            .builder
            .build_load(i8_ptr_ty, type_desc_ptr, "load_type_desc")?
            .into_pointer_value();

        let desc_ty = self.llvm_scoop_type_descriptor_type();
        let desc_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let desc_ptr = self
            .builder
            .build_pointer_cast(type_desc_i8, desc_ptr_ty, "type_desc")?;
        let vtable_field_ptr =
            self.builder
                .build_struct_gep(desc_ty, desc_ptr, 13, "type_desc_vtable_gep")?;
        let vtable_i8 = self
            .builder
            .build_load(i8_ptr_ty, vtable_field_ptr, "load_vtable")?
            .into_pointer_value();

        let vtable_entries_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let vtable_entries =
            self.builder
                .build_pointer_cast(vtable_i8, vtable_entries_ptr_ty, "vtable_entries")?;
        let slot_idx = i32_ty.const_int(slot as u64, false);
        let slot_ptr = unsafe {
            self.builder.build_in_bounds_gep(
                i8_ptr_ty,
                vtable_entries,
                &[slot_idx],
                "vtable_slot_ptr",
            )?
        };
        let fn_i8 = self
            .builder
            .build_load(i8_ptr_ty, slot_ptr, "load_vtable_fn")?
            .into_pointer_value();

        Ok(fn_i8)
    }

    pub(in crate::llvm::codegen) fn llvm_scoop_itable_entry_type_impl(&self) -> StructType<'ctx> {
        const TY_NAME: &str = "scoop.runtime.ScoopItableEntry";
        if let Some(existing) = self.context.get_struct_type(TY_NAME) {
            return existing;
        }

        let ty = self.context.opaque_struct_type(TY_NAME);
        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        ty.set_body(
            &[
                i64_ty.into(),
                i32_ty.into(),
                i32_ty.into(),
                i8_ptr_ty.into(),
                i8_ptr_ty.into(),
                i8_ptr_ty.into(),
            ],
            false,
        );
        ty
    }

    pub(in crate::llvm::codegen) fn llvm_scoop_itable_type_impl(&self) -> StructType<'ctx> {
        const TY_NAME: &str = "scoop.runtime.ScoopItable";
        if let Some(existing) = self.context.get_struct_type(TY_NAME) {
            return existing;
        }

        let ty = self.context.opaque_struct_type(TY_NAME);
        let i32_ty = self.context.i32_type();
        let entry_ty = self.llvm_scoop_itable_entry_type();
        let entries_ty = entry_ty.array_type(0);
        ty.set_body(&[i32_ty.into(), i32_ty.into(), entries_ty.into()], false);
        ty
    }

    pub(in crate::llvm::codegen) fn lookup_interface_itable_slot_impl(
        &mut self,
        at: crate::span::Span,
        receiver: PointerValue<'ctx>,
        interface_id: u64,
        slot: u32,
    ) -> Result<InterfaceItableSlotLookup<'ctx>, LlvmEmitError> {
        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();

        let header_ty = self.llvm_gc_object_header_type();
        let header_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let header_ptr =
            self.builder
                .build_pointer_cast(receiver, header_ptr_ty, "itable_hdr_ptr")?;

        let type_desc_ptr =
            self.builder
                .build_struct_gep(header_ty, header_ptr, 1, "itable_type_desc_gep")?;
        let type_desc_i8 = self
            .builder
            .build_load(i8_ptr_ty, type_desc_ptr, "load_type_desc")?
            .into_pointer_value();

        let desc_ty = self.llvm_scoop_type_descriptor_type();
        let desc_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let desc_ptr = self
            .builder
            .build_pointer_cast(type_desc_i8, desc_ptr_ty, "type_desc")?;
        let itable_field_ptr =
            self.builder
                .build_struct_gep(desc_ty, desc_ptr, 12, "type_desc_itable_gep")?;
        let itable_i8 = self
            .builder
            .build_load(i8_ptr_ty, itable_field_ptr, "load_itable")?
            .into_pointer_value();

        let itable_is_null = self.builder.build_is_null(itable_i8, "itable_is_null")?;
        let current_fn = self.current_codegen_function(at)?;
        let null_bb = self.context.append_basic_block(current_fn, "itable_null");
        let lookup_bb = self.context.append_basic_block(current_fn, "itable_lookup");
        let done_bb = self.context.append_basic_block(current_fn, "itable_done");
        self.builder
            .build_conditional_branch(itable_is_null, null_bb, lookup_bb)?;

        self.builder.position_at_end(null_bb);
        self.builder.build_unconditional_branch(done_bb)?;

        self.builder.position_at_end(lookup_bb);
        let itable_ty = self.llvm_scoop_itable_type();
        let itable_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let itable_ptr = self
            .builder
            .build_pointer_cast(itable_i8, itable_ptr_ty, "itable_ptr")?;

        let len_ptr = self
            .builder
            .build_struct_gep(itable_ty, itable_ptr, 0, "itable_len_gep")?;
        let len_i32 = self
            .builder
            .build_load(i32_ty, len_ptr, "itable_len")?
            .into_int_value();

        let entry_ty = self.llvm_scoop_itable_entry_type();
        let entries_field_ptr =
            self.builder
                .build_struct_gep(itable_ty, itable_ptr, 2, "itable_entries_gep")?;
        let entry_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let entries_base =
            self.builder
                .build_pointer_cast(entries_field_ptr, entry_ptr_ty, "itable_entries")?;

        let loop_bb = self.context.append_basic_block(current_fn, "itable_loop");
        let found_bb = self.context.append_basic_block(current_fn, "itable_found");
        let not_found_bb = self
            .context
            .append_basic_block(current_fn, "itable_not_found");

        self.builder.build_unconditional_branch(loop_bb)?;
        self.builder.position_at_end(loop_bb);

        let idx_phi = self.builder.build_phi(i32_ty, "itable_idx")?;
        idx_phi.add_incoming(&[(&i32_ty.const_zero(), lookup_bb)]);
        let idx_i32 = idx_phi.as_basic_value().into_int_value();

        let cond = self.builder.build_int_compare(
            IntPredicate::ULT,
            idx_i32,
            len_i32,
            "itable_idx_lt_len",
        )?;
        self.builder
            .build_conditional_branch(cond, found_bb, not_found_bb)?;

        self.builder.position_at_end(found_bb);
        let entry_ptr = unsafe {
            self.builder.build_in_bounds_gep(
                entry_ty,
                entries_base,
                &[idx_i32],
                "itable_entry_ptr",
            )?
        };
        let id_ptr =
            self.builder
                .build_struct_gep(entry_ty, entry_ptr, 0, "itable_entry_id_gep")?;
        let id_i64 = self
            .builder
            .build_load(i64_ty, id_ptr, "itable_entry_id")?
            .into_int_value();

        let target_id = i64_ty.const_int(interface_id, false);
        let id_ok =
            self.builder
                .build_int_compare(IntPredicate::EQ, id_i64, target_id, "itable_id_eq")?;

        let hit_bb = self.context.append_basic_block(current_fn, "itable_hit");
        let miss_bb = self.context.append_basic_block(current_fn, "itable_miss");
        self.builder
            .build_conditional_branch(id_ok, hit_bb, miss_bb)?;

        self.builder.position_at_end(miss_bb);
        let next =
            self.builder
                .build_int_add(idx_i32, i32_ty.const_int(1, false), "itable_idx_next")?;
        idx_phi.add_incoming(&[(&next, miss_bb)]);
        self.builder.build_unconditional_branch(loop_bb)?;

        self.builder.position_at_end(hit_bb);
        let methods_ptr =
            self.builder
                .build_struct_gep(entry_ty, entry_ptr, 4, "itable_entry_methods_gep")?;
        let methods_i8 = self
            .builder
            .build_load(i8_ptr_ty, methods_ptr, "itable_entry_methods")?
            .into_pointer_value();
        let receiver_type_ids_ptr = self.builder.build_struct_gep(
            entry_ty,
            entry_ptr,
            5,
            "itable_entry_receiver_type_ids_gep",
        )?;
        let receiver_type_ids_i8 = self
            .builder
            .build_load(
                i8_ptr_ty,
                receiver_type_ids_ptr,
                "itable_entry_receiver_type_ids",
            )?
            .into_pointer_value();
        self.builder.build_unconditional_branch(done_bb)?;

        self.builder.position_at_end(not_found_bb);
        self.builder.build_unconditional_branch(done_bb)?;

        self.builder.position_at_end(done_bb);
        let methods_phi = self.builder.build_phi(i8_ptr_ty, "itable_methods")?;
        methods_phi.add_incoming(&[
            (&i8_ptr_ty.const_null(), null_bb),
            (&i8_ptr_ty.const_null(), not_found_bb),
            (&methods_i8, hit_bb),
        ]);
        let methods_i8 = methods_phi.as_basic_value().into_pointer_value();
        let receiver_type_ids_phi = self
            .builder
            .build_phi(i8_ptr_ty, "itable_receiver_type_ids")?;
        receiver_type_ids_phi.add_incoming(&[
            (&i8_ptr_ty.const_null(), null_bb),
            (&i8_ptr_ty.const_null(), not_found_bb),
            (&receiver_type_ids_i8, hit_bb),
        ]);
        let receiver_type_ids_i8 = receiver_type_ids_phi.as_basic_value().into_pointer_value();

        let methods_is_null = self
            .builder
            .build_is_null(methods_i8, "itable_methods_is_null")?;
        let slot_null_bb = self
            .context
            .append_basic_block(current_fn, "itable_slot_null");
        let slot_ok_bb = self
            .context
            .append_basic_block(current_fn, "itable_slot_ok");
        let slot_done_bb = self
            .context
            .append_basic_block(current_fn, "itable_slot_done");
        self.builder
            .build_conditional_branch(methods_is_null, slot_null_bb, slot_ok_bb)?;

        self.builder.position_at_end(slot_null_bb);
        self.builder.build_unconditional_branch(slot_done_bb)?;

        self.builder.position_at_end(slot_ok_bb);
        let methods_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let methods_entries = self.builder.build_pointer_cast(
            methods_i8,
            methods_ptr_ty,
            "itable_methods_entries",
        )?;
        let slot_idx = i32_ty.const_int(slot as u64, false);
        let slot_ptr = unsafe {
            self.builder.build_in_bounds_gep(
                i8_ptr_ty,
                methods_entries,
                &[slot_idx],
                "itable_slot_ptr",
            )?
        };
        let fn_i8 = self
            .builder
            .build_load(i8_ptr_ty, slot_ptr, "load_itable_fn")?
            .into_pointer_value();

        let receiver_type_ids_is_null = self
            .builder
            .build_is_null(receiver_type_ids_i8, "itable_receiver_type_ids_is_null")?;
        let receiver_type_id_null_bb = self
            .context
            .append_basic_block(current_fn, "itable_receiver_type_id_null");
        let receiver_type_id_load_bb = self
            .context
            .append_basic_block(current_fn, "itable_receiver_type_id_load");
        let receiver_type_id_done_bb = self
            .context
            .append_basic_block(current_fn, "itable_receiver_type_id_done");
        self.builder.build_conditional_branch(
            receiver_type_ids_is_null,
            receiver_type_id_null_bb,
            receiver_type_id_load_bb,
        )?;

        self.builder.position_at_end(receiver_type_id_null_bb);
        self.builder
            .build_unconditional_branch(receiver_type_id_done_bb)?;

        self.builder.position_at_end(receiver_type_id_load_bb);
        let receiver_type_ids_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let receiver_type_ids_entries = self.builder.build_pointer_cast(
            receiver_type_ids_i8,
            receiver_type_ids_ptr_ty,
            "itable_receiver_type_ids_entries",
        )?;
        let receiver_type_id_ptr = unsafe {
            self.builder.build_in_bounds_gep(
                i64_ty,
                receiver_type_ids_entries,
                &[slot_idx],
                "itable_receiver_type_id_ptr",
            )?
        };
        let receiver_type_id = self
            .builder
            .build_load(i64_ty, receiver_type_id_ptr, "load_itable_receiver_type_id")?
            .into_int_value();
        self.builder
            .build_unconditional_branch(receiver_type_id_done_bb)?;

        self.builder.position_at_end(receiver_type_id_done_bb);
        let receiver_type_id_phi = self.builder.build_phi(i64_ty, "itable_receiver_type_id")?;
        receiver_type_id_phi.add_incoming(&[
            (&i64_ty.const_zero(), receiver_type_id_null_bb),
            (&receiver_type_id, receiver_type_id_load_bb),
        ]);
        self.builder.build_unconditional_branch(slot_done_bb)?;

        self.builder.position_at_end(slot_done_bb);
        let fn_phi = self.builder.build_phi(i8_ptr_ty, "itable_fn_i8")?;
        fn_phi.add_incoming(&[
            (&i8_ptr_ty.const_null(), slot_null_bb),
            (&fn_i8, receiver_type_id_done_bb),
        ]);
        let receiver_type_id_done = self
            .builder
            .build_phi(i64_ty, "itable_receiver_type_id_done")?;
        receiver_type_id_done.add_incoming(&[
            (&i64_ty.const_zero(), slot_null_bb),
            (
                &receiver_type_id_phi.as_basic_value().into_int_value(),
                receiver_type_id_done_bb,
            ),
        ]);
        Ok(InterfaceItableSlotLookup {
            fn_i8: fn_phi.as_basic_value().into_pointer_value(),
            receiver_type_id: receiver_type_id_done.as_basic_value().into_int_value(),
        })
    }

    pub(in crate::llvm::codegen) fn load_interface_itable_slot_fn_ptr_i8_impl(
        &mut self,
        at: crate::span::Span,
        receiver: PointerValue<'ctx>,
        interface_id: u64,
        slot: u32,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        Ok(self
            .lookup_interface_itable_slot_impl(at, receiver, interface_id, slot)?
            .fn_i8)
    }
}

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    fn dispatch_call_kind_for_receiver(
        &self,
        span: crate::span::Span,
        receiver_ty: TypeId,
    ) -> Result<Option<hir::DispatchCallKind>, LlvmEmitError> {
        let source = self.current_source()?;
        Ok(self
            .dispatch_call_sites
            .get(&hir::DispatchCallSite::new(
                source.path().to_path_buf(),
                span,
                receiver_ty,
            ))
            .copied())
    }

    pub(in crate::llvm::codegen) fn ordinary_effect_propagation_enabled(&self) -> bool {
        self.function_cx.current_fun_return_ty.is_some()
    }

    pub(in crate::llvm::codegen) fn current_codegen_function(
        &self,
        at: crate::span::Span,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        self.builder
            .get_insert_block()
            .and_then(|bb| bb.get_parent())
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no current function",
                at: at.into(),
            })
    }

    fn materialized_owner_hir_fun_for_callable(&self, fqn: &str) -> Option<&'a hir::FunDecl> {
        let pass_view = self.materialized_pass_view()?;
        let owner = pass_view.owner_of_callable(fqn)?;
        self.fun_index.values().copied().find(|fun| {
            fun.fqn == owner.template.fqn
                && fun.source_path == owner.template.source_path
                && fun.span == owner.template.decl_span
        })
    }

    fn canonical_builtin_signature_ty(&self, ty: TypeId) -> TypeId {
        match self.types.kind(ty) {
            TypeKind::Ref(RefTypeKind::Nominal(nominal)) if nominal.fqn == "scoop.core.String" => {
                self.builtins.string
            }
            _ => ty,
        }
    }

    fn interface_value_receiver_cases(
        &self,
        interface_fqn: &str,
        slot: u32,
    ) -> Result<Vec<InterfaceValueReceiverCase>, LlvmEmitError> {
        let mut seen = HashSet::new();
        let mut cases = Vec::new();
        for source_ty in self.types.iter_ids() {
            for entry in self.mir_value_box_itable_entries(source_ty)? {
                if entry.interface_fqn != interface_fqn {
                    continue;
                }
                let idx = slot as usize;
                let Some(impl_fqn) = entry.method_impl_fqns.get(idx) else {
                    continue;
                };
                if impl_fqn.is_empty() {
                    continue;
                }
                let receiver_type_id = entry
                    .method_receiver_type_ids
                    .get(idx)
                    .copied()
                    .unwrap_or(crate::itable::ITABLE_RECEIVER_REF_TYPE_ID);
                if receiver_type_id == crate::itable::ITABLE_RECEIVER_REF_TYPE_ID
                    || !seen.insert(receiver_type_id)
                {
                    continue;
                }
                cases.push(InterfaceValueReceiverCase {
                    receiver_type_id,
                    source_ty,
                    impl_fqn: impl_fqn.clone(),
                });
            }
        }
        cases.sort_by_key(|case| case.receiver_type_id);
        Ok(cases)
    }

    fn load_interface_value_box_payload(
        &mut self,
        at: crate::span::Span,
        receiver_ptr: PointerValue<'ctx>,
        source_ty: TypeId,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let source_cg = self
            .cg_ty_of(source_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "itable value-box receiver type",
                at: at.into(),
            })?;
        if source_cg == CgTy::Unit {
            return Ok(CgValue::unit());
        }

        let box_obj_ty = self.mir_value_box_object_type(at, source_ty, source_cg)?;
        let payload_gep = self.builder.build_struct_gep(
            box_obj_ty,
            receiver_ptr,
            1,
            "itable_value_box_payload_gep",
        )?;
        let raw = self.builder.build_load(
            self.llvm_basic_type_of(at, source_cg)?,
            payload_gep,
            "load_itable_value_box_payload",
        )?;
        self.cg_value_from_loaded(at, source_cg, raw)
    }

    fn ordinary_dispatch_fn_type(
        &mut self,
        callee_span: crate::span::Span,
        receiver_ty: TypeId,
        explicit_param_tys: &[TypeId],
        ret_cg: CgTy,
        hidden_sret_result_ty: Option<inkwell::types::BasicTypeEnum<'ctx>>,
        uses_explicit_effect_hidden_abi: bool,
    ) -> Result<FunctionType<'ctx>, LlvmEmitError> {
        let mut llvm_param_tys: Vec<BasicMetadataTypeEnum<'ctx>> = Vec::with_capacity(
            1 + explicit_param_tys.len()
                + usize::from(hidden_sret_result_ty.is_some())
                + self.explicit_effect_hidden_abi_param_count(uses_explicit_effect_hidden_abi)
                    as usize,
        );
        if hidden_sret_result_ty.is_some() {
            llvm_param_tys.push(self.context.ptr_type(AddressSpace::default()).into());
        }
        if uses_explicit_effect_hidden_abi {
            self.push_explicit_effect_hidden_abi_param_tys(&mut llvm_param_tys);
        }
        llvm_param_tys.push(
            self.ordinary_param_abi(callee_span, receiver_ty)?
                .llvm_param_ty(),
        );
        for param_ty in explicit_param_tys {
            llvm_param_tys.push(
                self.ordinary_param_abi(callee_span, *param_ty)?
                    .llvm_param_ty(),
            );
        }
        Ok(match (hidden_sret_result_ty, ret_cg) {
            (Some(_), _) | (None, CgTy::Unit | CgTy::Never) => {
                self.context.void_type().fn_type(&llvm_param_tys, false)
            }
            (None, other) => self
                .llvm_basic_type_of(callee_span, other)?
                .fn_type(&llvm_param_tys, false),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn emit_interface_dispatch_case_call_to_storage(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        callconv_fqn: &str,
        fn_i8: PointerValue<'ctx>,
        receiver_arg: BasicMetadataValueEnum<'ctx>,
        receiver_ty: TypeId,
        explicit_param_tys: &[TypeId],
        explicit_args: &[BasicMetadataValueEnum<'ctx>],
        ret_cg: CgTy,
        hidden_sret_result_ty: Option<inkwell::types::BasicTypeEnum<'ctx>>,
        hidden_sret_slot: Option<PointerValue<'ctx>>,
        uses_explicit_effect_hidden_abi: bool,
        effect_outcome_slot: Option<PointerValue<'ctx>>,
        direct_result_storage: Option<PointerValue<'ctx>>,
    ) -> Result<(), LlvmEmitError> {
        let llvm_fun_ty = self.ordinary_dispatch_fn_type(
            callee_span,
            receiver_ty,
            explicit_param_tys,
            ret_cg,
            hidden_sret_result_ty,
            uses_explicit_effect_hidden_abi,
        )?;
        let typed_fn_ptr = self.builder.build_pointer_cast(
            fn_i8,
            self.llvm_ptr_type(AddressSpace::default()),
            "itable_fn_typed",
        )?;

        let mut llvm_args: Vec<BasicMetadataValueEnum<'ctx>> = Vec::with_capacity(
            1 + explicit_args.len()
                + usize::from(hidden_sret_result_ty.is_some())
                + self.explicit_effect_hidden_abi_param_count(uses_explicit_effect_hidden_abi)
                    as usize,
        );
        if let Some(slot) = hidden_sret_slot {
            llvm_args.push(slot.into());
        }
        if uses_explicit_effect_hidden_abi {
            let outcome_slot = effect_outcome_slot.ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "itable effect outcome slot",
                at: span.into(),
            })?;
            llvm_args.push(self.current_effect_ctx_arg().into());
            llvm_args.push(self.llvm_gc_i8_ptr_type().const_null().into());
            llvm_args.push(outcome_slot.into());
        }
        llvm_args.push(receiver_arg);
        llvm_args.extend_from_slice(explicit_args);

        let call_site = self.with_conservative_gc_local_root_spills(span, |cg| {
            let call_site = cg.builder.build_indirect_call(
                llvm_fun_ty,
                typed_fn_ptr,
                &llvm_args,
                "call_itable",
            )?;
            if let Some(result_ty) = hidden_sret_result_ty {
                cg.add_sret_attribute_to_call(call_site, 0, result_ty);
            }
            call_site.set_call_convention(cg.llvm_call_convention_for_fqn(callconv_fqn));
            Ok(call_site)
        })?;

        if hidden_sret_result_ty.is_none() && !matches!(ret_cg, CgTy::Unit | CgTy::Never) {
            let result_storage =
                direct_result_storage.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "itable direct result storage",
                    at: span.into(),
                })?;
            let raw = call_site.try_as_basic_value().basic().ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "itable direct return value",
                    at: span.into(),
                },
            )?;
            let result = self.cg_value_from_loaded(span, ret_cg, raw)?;
            let _ = self.store_local_value(span, result_storage, ret_cg, result)?;
        }
        Ok(())
    }

    pub(in crate::llvm::codegen) fn load_dispatch_result_from_storage(
        &mut self,
        at: crate::span::Span,
        ret_cg: CgTy,
        result_storage: PointerValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let raw = self.builder.build_load(
            self.llvm_basic_type_of(at, ret_cg)?,
            result_storage,
            "load_itable_dispatch_result",
        )?;
        self.cg_value_from_loaded(at, ret_cg, raw)
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn emit_interface_dispatch_indirect_call(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        interface_fqn: &str,
        slot: u32,
        sig_fun: &'a hir::FunDecl,
        uses_explicit_effect_hidden_abi: bool,
        receiver_ptr: PointerValue<'ctx>,
        lookup: InterfaceItableSlotLookup<'ctx>,
        explicit_args: &[BasicMetadataValueEnum<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let fn_is_null = self
            .builder
            .build_is_null(lookup.fn_i8, "itable_fn_is_null")?;
        let current_fn = self.current_codegen_function(span)?;
        let ok_bb = self.context.append_basic_block(current_fn, "itable_fn_ok");
        let bad_bb = self
            .context
            .append_basic_block(current_fn, "itable_fn_null");
        self.builder
            .build_conditional_branch(fn_is_null, bad_bb, ok_bb)?;
        self.builder.position_at_end(bad_bb);
        let exit = self.declare_libc_exit();
        let code = self.context.i32_type().const_int(7, false);
        let _ = self
            .builder
            .build_call(exit, &[code.into()], "itable_fn_null_exit")?;
        self.builder.build_unreachable()?;
        self.builder.position_at_end(ok_bb);

        let ret_cg =
            self.cg_ty_of(sig_fun.return_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "itable call return type",
                    at: span.into(),
                })?;
        let hidden_sret_result_ty = self.hidden_sret_result_ty(callee_span, ret_cg)?;
        let hidden_sret_slot = if hidden_sret_result_ty.is_some() {
            Some(self.create_entry_alloca(callee_span, "itable_call_sret", ret_cg)?)
        } else {
            None
        };
        let effect_outcome_slot = if uses_explicit_effect_hidden_abi {
            Some(self.alloc_effect_outcome_slot(span, "itable_call")?)
        } else {
            None
        };
        let direct_result_storage =
            if hidden_sret_result_ty.is_none() && !matches!(ret_cg, CgTy::Unit | CgTy::Never) {
                Some(self.create_entry_alloca(callee_span, "itable_call_result", ret_cg)?)
            } else {
                None
            };

        let explicit_param_tys = sig_fun.params[1..]
            .iter()
            .map(|param| param.ty)
            .collect::<Vec<_>>();
        let value_cases = self.interface_value_receiver_cases(interface_fqn, slot)?;

        if value_cases.is_empty() {
            self.emit_interface_dispatch_case_call_to_storage(
                span,
                callee_span,
                &sig_fun.fqn,
                lookup.fn_i8,
                receiver_ptr.into(),
                sig_fun.params[0].ty,
                &explicit_param_tys,
                explicit_args,
                ret_cg,
                hidden_sret_result_ty,
                hidden_sret_slot,
                uses_explicit_effect_hidden_abi,
                effect_outcome_slot,
                direct_result_storage,
            )?;
        } else {
            let ref_bb = self
                .context
                .append_basic_block(current_fn, "itable_ref_receiver");
            let value_switch_bb = self
                .context
                .append_basic_block(current_fn, "itable_value_receiver_switch");
            let done_bb = self
                .context
                .append_basic_block(current_fn, "itable_dispatch_done");
            let receiver_is_ref = self.builder.build_int_compare(
                IntPredicate::EQ,
                lookup.receiver_type_id,
                self.context
                    .i64_type()
                    .const_int(crate::itable::ITABLE_RECEIVER_REF_TYPE_ID, false),
                "itable_receiver_is_ref",
            )?;
            self.builder
                .build_conditional_branch(receiver_is_ref, ref_bb, value_switch_bb)?;

            self.builder.position_at_end(ref_bb);
            self.emit_interface_dispatch_case_call_to_storage(
                span,
                callee_span,
                &sig_fun.fqn,
                lookup.fn_i8,
                receiver_ptr.into(),
                sig_fun.params[0].ty,
                &explicit_param_tys,
                explicit_args,
                ret_cg,
                hidden_sret_result_ty,
                hidden_sret_slot,
                uses_explicit_effect_hidden_abi,
                effect_outcome_slot,
                direct_result_storage,
            )?;
            self.builder.build_unconditional_branch(done_bb)?;

            self.builder.position_at_end(value_switch_bb);
            let bad_receiver_bb = self
                .context
                .append_basic_block(current_fn, "itable_receiver_type_bad");
            let case_bbs = value_cases
                .iter()
                .map(|case| {
                    (
                        self.context
                            .i64_type()
                            .const_int(case.receiver_type_id, false),
                        self.context.append_basic_block(
                            current_fn,
                            &format!("itable_value_receiver_{}", case.receiver_type_id),
                        ),
                    )
                })
                .collect::<Vec<_>>();
            let case_refs = case_bbs
                .iter()
                .map(|(value, bb)| (*value, *bb))
                .collect::<Vec<_>>();
            let _ =
                self.builder
                    .build_switch(lookup.receiver_type_id, bad_receiver_bb, &case_refs)?;

            for (case, (_, case_bb)) in value_cases.iter().zip(case_bbs.iter()) {
                self.builder.position_at_end(*case_bb);
                let impl_sig = self.fun_index.get(case.impl_fqn.as_str()).copied().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "itable value receiver target signature",
                        at: span.into(),
                    },
                )?;
                let receiver_value = self.load_interface_value_box_payload(
                    callee_span,
                    receiver_ptr,
                    case.source_ty,
                )?;
                let receiver_arg = if self
                    .ordinary_param_abi(callee_span, case.source_ty)?
                    .pointee_ty()
                    .is_some()
                {
                    let receiver_slot = self.create_entry_alloca(
                        callee_span,
                        &format!("itable_value_receiver_{}", case.receiver_type_id),
                        receiver_value.ty,
                    )?;
                    let _ = self.store_local_value(
                        callee_span,
                        receiver_slot,
                        receiver_value.ty,
                        receiver_value,
                    )?;
                    receiver_slot.into()
                } else {
                    self.as_llvm_arg_value(callee_span, receiver_value.ty, receiver_value)?
                };
                let impl_param_tys = impl_sig.params[1..]
                    .iter()
                    .map(|param| param.ty)
                    .collect::<Vec<_>>();
                self.emit_interface_dispatch_case_call_to_storage(
                    span,
                    callee_span,
                    &case.impl_fqn,
                    lookup.fn_i8,
                    receiver_arg,
                    case.source_ty,
                    &impl_param_tys,
                    explicit_args,
                    ret_cg,
                    hidden_sret_result_ty,
                    hidden_sret_slot,
                    uses_explicit_effect_hidden_abi,
                    effect_outcome_slot,
                    direct_result_storage,
                )?;
                self.builder.build_unconditional_branch(done_bb)?;
            }

            self.builder.position_at_end(bad_receiver_bb);
            let exit = self.declare_libc_exit();
            let code = self.context.i32_type().const_int(8, false);
            let _ =
                self.builder
                    .build_call(exit, &[code.into()], "itable_receiver_type_bad_exit")?;
            self.builder.build_unreachable()?;

            self.builder.position_at_end(done_bb);
        }

        if let Some(result_ptr) = hidden_sret_slot {
            self.sync_hidden_sret_result_roots(span, ret_cg, result_ptr, "itable_call_sret")?;
        }
        if let Some(outcome_slot) = effect_outcome_slot {
            self.maybe_record_active_suspend_site_effect_outcome(span, outcome_slot);
            self.emit_ordinary_call_effect_propagation_check_from_outcome(
                span,
                outcome_slot,
                "itable_call_effect",
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
                        "itable_call_sret",
                    )
                } else {
                    self.load_dispatch_result_from_storage(
                        span,
                        ret_cg,
                        direct_result_storage.ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "itable direct result storage",
                            at: span.into(),
                        })?,
                    )
                }
            }
        }
    }

    fn emit_local_effect_escape_check(
        &mut self,
        at: crate::span::Span,
        outcome_slot: PointerValue<'ctx>,
        label: &str,
    ) -> Result<(), LlvmEmitError> {
        let Some(target_bb) = self.current_local_effect_escape_target() else {
            return Ok(());
        };
        let current_fn = self.current_codegen_function(at)?;
        let continue_bb = self
            .context
            .append_basic_block(current_fn, &format!("{label}_continue"));
        let is_propagating = self.effect_outcome_is_propagating(at, outcome_slot, label)?;
        self.builder
            .build_conditional_branch(is_propagating, target_bb, continue_bb)?;
        self.builder.position_at_end(continue_bb);
        Ok(())
    }

    pub(in crate::llvm::codegen) fn emit_current_local_effect_escape_check(
        &mut self,
        at: crate::span::Span,
        label: &str,
    ) -> Result<(), LlvmEmitError> {
        let Some(outcome_slot) = self.function_cx.current_effect_outcome_ptr else {
            return Ok(());
        };
        self.emit_local_effect_escape_check(at, outcome_slot, label)
    }

    pub(in crate::llvm::codegen) fn current_effect_ctx_arg(&self) -> PointerValue<'ctx> {
        self.function_cx
            .current_effect_ctx_ref
            .unwrap_or_else(|| self.llvm_gc_i8_ptr_type().const_null())
    }

    fn emit_effect_propagation_return(
        &mut self,
        at: crate::span::Span,
    ) -> Result<(), LlvmEmitError> {
        let Some(declared_return_ty) = self.function_cx.current_fun_return_ty else {
            return Ok(());
        };

        if declared_return_ty != CgTy::Never
            && let Some(return_ctx) = self.function_cx.return_context
        {
            let default = self.default_value(at, declared_return_ty)?;
            if let Some(alloca) = return_ctx.return_alloca
                && let Some(raw) = default.value
            {
                self.builder.build_store(alloca, raw)?;
            }
            self.builder
                .build_unconditional_branch(return_ctx.return_bb)?;
            return Ok(());
        }

        match declared_return_ty {
            CgTy::Never => {
                self.builder.build_return(None)?;
                Ok(())
            }
            _ => {
                let default = self.default_value(at, declared_return_ty)?;
                self.emit_return(at, declared_return_ty, default)
            }
        }
    }

    fn copy_effect_outcome_into_current_function_slot(
        &mut self,
        at: crate::span::Span,
        outcome_slot: PointerValue<'ctx>,
        label: &str,
    ) -> Result<(), LlvmEmitError> {
        let Some(current_outcome_ptr) = self.function_cx.current_effect_outcome_ptr else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "ordinary effect propagation destination",
                at: at.into(),
            });
        };
        let outcome = self.builder.build_load(
            self.llvm_effect_outcome_struct_type(),
            outcome_slot,
            &format!("{label}_load_outcome"),
        )?;
        self.builder.build_store(current_outcome_ptr, outcome)?;
        Ok(())
    }

    pub(in crate::llvm::codegen) fn emit_ordinary_non_resuming_effect_exit(
        &mut self,
        at: crate::span::Span,
        label: &str,
    ) -> Result<(), LlvmEmitError> {
        if !self.ordinary_effect_propagation_enabled() {
            if let Some(target_bb) = self.current_local_effect_escape_target() {
                let current_fn = self.current_codegen_function(at)?;
                let dead_bb = self
                    .context
                    .append_basic_block(current_fn, &format!("{label}_dead"));
                self.builder.build_unconditional_branch(target_bb)?;
                self.builder.position_at_end(dead_bb);
            }
            return Ok(());
        }

        let current_fn = self.current_codegen_function(at)?;
        let return_bb = self
            .context
            .append_basic_block(current_fn, &format!("{label}_return"));
        let dead_bb = self
            .context
            .append_basic_block(current_fn, &format!("{label}_dead"));

        self.builder.build_unconditional_branch(return_bb)?;
        self.builder.position_at_end(return_bb);
        self.emit_effect_propagation_return(at)?;

        self.builder.position_at_end(dead_bb);
        Ok(())
    }

    pub(in crate::llvm::codegen) fn emit_ordinary_call_effect_propagation_check(
        &mut self,
        at: crate::span::Span,
        label: &str,
    ) -> Result<(), LlvmEmitError> {
        let Some(outcome_ptr) = self.function_cx.current_effect_outcome_ptr else {
            return Ok(());
        };
        if !self.ordinary_effect_propagation_enabled() {
            return self.emit_local_effect_escape_check(at, outcome_ptr, label);
        }
        let current_fn = self.current_codegen_function(at)?;
        let return_bb = self
            .context
            .append_basic_block(current_fn, &format!("{label}_return"));
        let continue_bb = self
            .context
            .append_basic_block(current_fn, &format!("{label}_continue"));
        let is_propagating = self.effect_outcome_is_propagating(at, outcome_ptr, label)?;

        self.builder
            .build_conditional_branch(is_propagating, return_bb, continue_bb)?;

        self.builder.position_at_end(return_bb);
        self.emit_effect_propagation_return(at)?;

        self.builder.position_at_end(continue_bb);
        Ok(())
    }

    pub(in crate::llvm::codegen) fn emit_ordinary_call_effect_propagation_check_from_outcome(
        &mut self,
        at: crate::span::Span,
        outcome_slot: PointerValue<'ctx>,
        label: &str,
    ) -> Result<(), LlvmEmitError> {
        if !self.ordinary_effect_propagation_enabled() {
            if self.function_cx.current_effect_outcome_ptr.is_some() {
                self.copy_effect_outcome_into_current_function_slot(at, outcome_slot, label)?;
                return self.emit_local_effect_escape_check(at, outcome_slot, label);
            }
            return Ok(());
        }

        let current_fn = self.current_codegen_function(at)?;
        let return_bb = self
            .context
            .append_basic_block(current_fn, &format!("{label}_return"));
        let continue_bb = self
            .context
            .append_basic_block(current_fn, &format!("{label}_continue"));
        let is_propagating = self.effect_outcome_is_propagating(at, outcome_slot, label)?;

        self.builder
            .build_conditional_branch(is_propagating, return_bb, continue_bb)?;

        self.builder.position_at_end(return_bb);
        self.copy_effect_outcome_into_current_function_slot(at, outcome_slot, label)?;
        self.emit_effect_propagation_return(at)?;

        self.builder.position_at_end(continue_bb);
        Ok(())
    }

    pub(in crate::llvm::codegen) fn codegen_call_impl(
        &mut self,
        span: crate::span::Span,
        callee: &hir::Expr,
        args: &[hir::CallArg],
        expected: Option<CgTy>,
        result_ty: Option<TypeId>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if self
            .continuation_resume_call_sites
            .contains(&self.current_call_site(span)?)
        {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Continuation.resume lowering",
                at: span.into(),
            });
        }

        if let Some(value) =
            self.try_codegen_builtin_member_call_short_circuit(span, callee, args, expected)?
        {
            return Ok(value);
        }

        enum CallableCallee {
            FunctionValue(crate::ty::FunctionType),
            FunPtr(crate::ty::FunctionType),
        }

        let callable_callee = self
            .resolve_expr_concrete_type(callee)
            .and_then(|callee_hir_ty| match self.types.kind(callee_hir_ty) {
                TypeKind::Ref(RefTypeKind::Function(fun_ty)) => {
                    Some(CallableCallee::FunctionValue(fun_ty.clone()))
                }
                TypeKind::Value(ValueTypeKind::Nominal(nominal))
                    if nominal.fqn == "scoop.unsafe.FunPtr" =>
                {
                    let sig_ty = nominal.args.first().copied()?;
                    let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = self.types.kind(sig_ty)
                    else {
                        return None;
                    };
                    Some(CallableCallee::FunPtr(fun_ty.clone()))
                }
                _ => None,
            });
        if let Some(callable_callee) = callable_callee {
            match callable_callee {
                CallableCallee::FunctionValue(fun_ty) => {
                    let call_may_suspend = self
                        .function_value_expr_body_may_outward_effect_when_called_for_local(callee);
                    let callable_abi = self.managed_callable_abi_identity(call_may_suspend);
                    let callee_value = self.codegen_expr(callee)?;
                    let callee_value = self.coerce_value(callee.span, callee_value, CgTy::Ref)?;
                    let Some(BasicValueEnum::PointerValue(closure_obj_i8)) = callee_value.value
                    else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "callable callee value type",
                            at: callee.span.into(),
                        });
                    };
                    return self.codegen_function_value_call_from_closure_obj(
                        closure_obj_i8,
                        CallableValueCallSpec {
                            span,
                            callee_span: callee.span,
                            call_may_suspend: callable_abi.uses_effect_bridge_abi(),
                            fun_ty: &fun_ty,
                            args,
                        },
                    );
                }
                CallableCallee::FunPtr(fun_ty) => {
                    let callee_value = self.codegen_expr(callee)?;
                    let (funptr_addr, funptr_int_ty) =
                        callee_value
                            .as_int()
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "funptr callee value type",
                                at: callee.span.into(),
                            })?;
                    return self.codegen_funptr_value_call(
                        funptr_addr,
                        funptr_int_ty,
                        FunPtrCallSpec {
                            span,
                            callee_span: callee.span,
                            fun_ty: &fun_ty,
                            args,
                        },
                    );
                }
            }
        }

        if let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &callee.kind {
            let local =
                self.function_cx
                    .env
                    .get(*id)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "unknown local value",
                        at: callee.span.into(),
                    })?;

            if let Some(hir_ty) = local.hir_ty {
                if let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = self.types.kind(hir_ty) {
                    return self.codegen_function_value_call(
                        &local,
                        CallableValueCallSpec {
                            span,
                            callee_span: callee.span,
                            call_may_suspend: self
                                .managed_callable_abi_identity(local.call_may_suspend)
                                .uses_effect_bridge_abi(),
                            fun_ty,
                            args,
                        },
                    );
                }

                if let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(hir_ty)
                    && nominal.fqn == "scoop.unsafe.FunPtr"
                {
                    let sig_ty = nominal.args.first().copied().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "funptr signature type",
                            at: callee.span.into(),
                        },
                    )?;
                    let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = self.types.kind(sig_ty)
                    else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "funptr signature kind",
                            at: callee.span.into(),
                        });
                    };

                    let CgTy::Int(int_ty) = local.ty else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "funptr local cg type",
                            at: callee.span.into(),
                        });
                    };
                    let local_ptr =
                        self.local_ptr_for_use(callee.span, local, "load_funptr_slot")?;
                    let loaded = self
                        .builder
                        .build_load(
                            self.llvm_basic_type_of(callee.span, local.ty)?,
                            local_ptr,
                            "load_funptr",
                        )?
                        .into_int_value();

                    return self.codegen_funptr_value_call(
                        loaded,
                        int_ty,
                        FunPtrCallSpec {
                            span,
                            callee_span: callee.span,
                            fun_ty,
                            args,
                        },
                    );
                }
            }
        }

        if let hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) = &callee.kind {
            if let Some(entry_name) = self
                .current_top_level_fun_call_binding(span)?
                .and_then(|binding| binding.intrinsic_entry_name.clone())
                .or_else(|| {
                    crate::intrinsics::fallback_named_intrinsic_entry_name_for_fqn(fqn)
                        .map(str::to_string)
                })
                && let Some(value) = self.try_codegen_named_intrinsic_hir_call(
                    span,
                    callee.span,
                    callee,
                    args,
                    &entry_name,
                )?
            {
                return Ok(value);
            }

            let concrete_fqn = self.concrete_top_level_fun_call_fqn(span, fqn)?;
            let dispatch_fqn = direct_call_dispatch_fqn(&concrete_fqn);

            if dispatch_fqn == "scoop.unsafe.invoke" {
                return self.codegen_sysroot_funptr_invoke(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.unsafe.funPtrToUIntPtr" {
                return self.codegen_sysroot_funptr_to_uintptr(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.unsafe.uintPtrToFunPtr" {
                return self.codegen_sysroot_uintptr_to_funptr(span, callee.span, args);
            }

            if let Some(value) = self.try_codegen_class_vtable_call(span, callee.span, fqn, args)? {
                return Ok(value);
            }
            if let Some(value) =
                self.try_codegen_interface_itable_call(span, callee.span, fqn, args)?
            {
                return Ok(value);
            }
            if let Some(value) =
                self.try_codegen_sysroot_gc_debug_intrinsics(span, dispatch_fqn, args)?
            {
                return Ok(value);
            }
            if dispatch_fqn == "scoop.core.sizeOf" {
                return self.codegen_sysroot_size_of(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.core.alignOf" {
                return self.codegen_sysroot_align_of(span);
            }
            if dispatch_fqn == "scoop.core.kindOf" {
                return self.codegen_sysroot_kind_of(span);
            }
            if dispatch_fqn == "scoop.core.descOf" {
                return self.codegen_sysroot_desc_of(span);
            }
            if dispatch_fqn == "scoop.core.panic" {
                return self.codegen_sysroot_panic(span, callee.span, args);
            }
            if (dispatch_fqn == "scoop.core.print" || dispatch_fqn == "scoop.core.println")
                && let Some(value) =
                    self.try_codegen_sysroot_print_string_like(span, dispatch_fqn, args)?
            {
                return Ok(value);
            }
            if dispatch_fqn == "scoop.core.__scoop_print_string"
                || dispatch_fqn == "scoop.core.__scoop_println_string"
            {
                return self.codegen_sysroot_internal_print_string(
                    span,
                    callee.span,
                    dispatch_fqn,
                    args,
                );
            }
            if dispatch_fqn == "scoop.core.toInt" {
                return self.codegen_sysroot_to_int_ext(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.core.hash" {
                return self.codegen_sysroot_hash_ext(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.core.abs"
                && matches!(
                    args.first(),
                    Some(hir::CallArg::Positional(expr))
                        if matches!(self.resolve_expr_cg_ty(expr), Some(CgTy::Float64 | CgTy::Float32))
                )
            {
                return self.codegen_sysroot_abs_ext(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.core.isNaN"
                && matches!(
                    args.first(),
                    Some(hir::CallArg::Positional(expr))
                        if matches!(self.resolve_expr_cg_ty(expr), Some(CgTy::Float64 | CgTy::Float32))
                )
            {
                return self.codegen_sysroot_is_nan_ext(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.core.isInfinite"
                && matches!(
                    args.first(),
                    Some(hir::CallArg::Positional(expr))
                        if matches!(self.resolve_expr_cg_ty(expr), Some(CgTy::Float64 | CgTy::Float32))
                )
            {
                return self.codegen_sysroot_is_infinite_ext(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.sync.mutexCreate" {
                return self.codegen_sysroot_sync_mutex_create(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.sync.lock" {
                return self.codegen_sysroot_sync_mutex_lock(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.sync.unlock" {
                return self.codegen_sysroot_sync_mutex_unlock(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.sync.condVarCreate" {
                return self.codegen_sysroot_sync_condvar_create(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.sync.wait" {
                return self.codegen_sysroot_sync_condvar_wait(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.sync.notifyOne" {
                return self.codegen_sysroot_sync_condvar_notify_one(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.sync.notifyAll" {
                return self.codegen_sysroot_sync_condvar_notify_all(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.sync.onceCreate" {
                return self.codegen_sysroot_sync_once_create(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.sync.isDone" {
                return self.codegen_sysroot_sync_once_is_done(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.sync.run" {
                return self.codegen_sysroot_sync_once_run(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.sync.destroy" {
                return self.codegen_sysroot_sync_destroy(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.thread.threadSpawn" {
                return self.codegen_sysroot_thread_spawn(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.thread.join" {
                return self.codegen_sysroot_thread_join(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.thread.sleepMillis" {
                return self.codegen_sysroot_thread_sleep_millis(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.thread.yield" {
                return self.codegen_sysroot_thread_yield(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.thread.currentId" {
                return self.codegen_sysroot_thread_current_id(span, callee.span, args);
            }
            if dispatch_fqn.starts_with("scoop.unsafe.__atomicInt") {
                return self.codegen_sysroot_atomic_int_intrinsics(
                    span,
                    callee.span,
                    dispatch_fqn,
                    args,
                );
            }
            if let Some(callee_hir_ty) = self.top_level_value_ty(fqn) {
                if let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = self.types.kind(callee_hir_ty)
                {
                    let call_may_suspend = self
                        .function_value_expr_body_may_outward_effect_when_called_for_local(callee);
                    let callable_abi = self.managed_callable_abi_identity(call_may_suspend);
                    let callee_value = self.codegen_top_level_value_ref(callee.span, fqn)?;
                    let CgValue {
                        ty: CgTy::Ref,
                        value: Some(BasicValueEnum::PointerValue(closure_obj_i8)),
                    } = callee_value
                    else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "function value top-level type",
                            at: callee.span.into(),
                        });
                    };
                    return self.codegen_function_value_call_from_closure_obj(
                        closure_obj_i8,
                        CallableValueCallSpec {
                            span,
                            callee_span: callee.span,
                            call_may_suspend: callable_abi.uses_effect_bridge_abi(),
                            fun_ty,
                            args,
                        },
                    );
                }

                if let TypeKind::Value(ValueTypeKind::Nominal(nominal)) =
                    self.types.kind(callee_hir_ty)
                    && nominal.fqn == "scoop.unsafe.FunPtr"
                {
                    let sig_ty = nominal.args.first().copied().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "funptr signature type",
                            at: callee.span.into(),
                        },
                    )?;
                    let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = self.types.kind(sig_ty)
                    else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "funptr signature kind",
                            at: callee.span.into(),
                        });
                    };

                    let callee_value = self.codegen_top_level_value_ref(callee.span, fqn)?;
                    let (funptr_addr, funptr_int_ty) =
                        callee_value
                            .as_int()
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "funptr top-level cg type",
                                at: callee.span.into(),
                            })?;
                    return self.codegen_funptr_value_call(
                        funptr_addr,
                        funptr_int_ty,
                        FunPtrCallSpec {
                            span,
                            callee_span: callee.span,
                            fun_ty,
                            args,
                        },
                    );
                }
            }

            return self.codegen_top_level_fun_call(span, callee.span, &concrete_fqn, args);
        }

        if let hir::ExprKind::MemberAccess { receiver, member } = &callee.kind {
            if let Some(hir::MemberRef::Fun { fqn, .. }) = member.resolved.as_ref() {
                if fqn == "scoop.core.GC.handleNew" {
                    return self.codegen_sysroot_gc_handle_new(span, member.span, args, expected);
                }
                if fqn == "scoop.core.GC.handleGet" {
                    return self.codegen_sysroot_gc_handle_get(span, member.span, args);
                }
                if fqn == "scoop.core.GC.handleDrop" {
                    return self.codegen_sysroot_gc_handle_drop(span, member.span, args);
                }
                if fqn == "scoop.core.GC.pin" {
                    return self.codegen_sysroot_gc_pin(span, member.span, args, expected);
                }
                if fqn == "scoop.core.GC.unpin" {
                    return self.codegen_sysroot_gc_unpin(span, member.span, args);
                }
            }

            if member.name == "toInt" {
                let recv_ty = match &receiver.kind {
                    hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) => self
                        .function_cx
                        .env
                        .get(*id)
                        .and_then(|local| local.hir_ty)
                        .unwrap_or(receiver.ty),
                    _ => receiver.ty,
                };
                if matches!(
                    self.types.kind(recv_ty),
                    TypeKind::Value(ValueTypeKind::Char)
                ) {
                    return self.codegen_char_method_to_int(receiver);
                }
                if matches!(
                    self.types.kind(recv_ty),
                    TypeKind::Value(ValueTypeKind::Float64 | ValueTypeKind::Float32)
                ) {
                    let recv = self.codegen_expr(receiver)?;
                    return self.codegen_float_to_int_value(span, receiver.span, recv);
                }
            }
            if matches!(member.name.as_str(), "byteLength" | "getByte") {
                return self.codegen_string_method(span, receiver, &member.name, args);
            }
            if member.name == "hash" {
                let recv_ty = match &receiver.kind {
                    hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) => self
                        .function_cx
                        .env
                        .get(*id)
                        .and_then(|local| local.hir_ty)
                        .unwrap_or(receiver.ty),
                    _ => receiver.ty,
                };
                match self.types.kind(recv_ty) {
                    TypeKind::Value(ValueTypeKind::Char) => {
                        return self.codegen_char_method_hash(span, receiver);
                    }
                    TypeKind::Value(ValueTypeKind::Int) => {
                        return self.codegen_int_method_hash(span, receiver);
                    }
                    TypeKind::Value(ValueTypeKind::Float64 | ValueTypeKind::Float32) => {
                        let recv = self.codegen_expr(receiver)?;
                        return self.codegen_float_hash_value(receiver.span, recv);
                    }
                    _ => {}
                }
            }
            if matches!(member.name.as_str(), "abs" | "isNaN" | "isInfinite") {
                let recv = self.codegen_expr(receiver)?;
                return match member.name.as_str() {
                    "abs" => self.codegen_float_abs_value(receiver.span, recv),
                    "isNaN" => self.codegen_float_is_nan_value(receiver.span, recv),
                    "isInfinite" => self.codegen_float_is_infinite_value(receiver.span, recv),
                    _ => unreachable!("filtered by matches!"),
                };
            }

            if let Some(hir::MemberRef::Value { fqn, .. }) = member.resolved.as_ref()
                && let Some((_owner_fqn, variant_name)) = fqn.rsplit_once('.')
                && let Some(CgTy::Enum(enum_ty)) = expected
            {
                let layout = self.cg_enum_layout(span, enum_ty)?;
                if layout
                    .variants
                    .iter()
                    .any(|variant| variant.name == variant_name)
                {
                    return self.codegen_enum_variant_ctor_call(span, enum_ty, variant_name, args);
                }
            }
        }

        if let hir::ExprKind::UnresolvedIdent { name } = &callee.kind {
            let call_site = self.current_call_site(span)?;
            if let Some(site) = self.ctor_call_sites.get(&call_site) {
                return self.codegen_class_ctor_call(
                    span,
                    callee.span,
                    name,
                    args,
                    site,
                    result_ty,
                );
            }

            let Some(CgTy::Enum(enum_ty)) = expected else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "enum variant ctor call without expected enum type",
                    at: callee.span.into(),
                });
            };
            return self.codegen_enum_variant_ctor_call(span, enum_ty, name, args);
        }

        Err(LlvmEmitError::UnsupportedMainBody {
            kind: "call callee",
            at: callee.span.into(),
        })
    }

    pub(in crate::llvm::codegen) fn codegen_top_level_fun_call_impl(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        enum CallableSig<'b> {
            Hir(&'b hir::FunDecl),
            Mir(crate::mir::FunDecl, &'b TypeStore),
        }

        let callable_abi = self.direct_call_abi_identity(fqn);
        let dispatch_fqn = direct_call_dispatch_fqn(fqn);
        let uses_explicit_effect_hidden_abi = callable_abi.uses_effect_bridge_abi();
        let sig_fun = self
            .materialized_pass_view()
            .and_then(|view| {
                view.callable(fqn)
                    .cloned()
                    .map(|fun| CallableSig::Mir(fun, &view.materialized().types))
            })
            .or_else(|| {
                self.materialized_owner_hir_fun_for_callable(fqn)
                    .map(CallableSig::Hir)
            })
            .or_else(|| self.fun_index.get(fqn).copied().map(CallableSig::Hir))
            .or_else(|| {
                if dispatch_fqn == fqn {
                    None
                } else {
                    self.materialized_owner_hir_fun_for_callable(dispatch_fqn)
                        .map(CallableSig::Hir)
                        .or_else(|| {
                            self.fun_index
                                .get(dispatch_fqn)
                                .copied()
                                .map(CallableSig::Hir)
                        })
                }
            })
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "call callee type `{fqn}` is missing from materialized MIR and HIR indexes"
                ),
            })?;
        let signature_owner_fqn = match &sig_fun {
            CallableSig::Hir(_) if dispatch_fqn != fqn => dispatch_fqn,
            _ => fqn,
        };

        let (param_names, mut param_tys, mut return_ty) = match &sig_fun {
            CallableSig::Hir(fun) => {
                let param_names = fun
                    .params
                    .iter()
                    .map(|param| param.name.clone())
                    .collect::<Vec<_>>();
                let fallback_param_tys =
                    fun.params.iter().map(|param| param.ty).collect::<Vec<_>>();
                let fallback_return_ty = fun.return_ty;
                let inferred_param_tys = if fallback_param_tys
                    .iter()
                    .any(|&ty| self.cg_ty_of(ty).is_none())
                {
                    let mut inferred = fallback_param_tys.clone();
                    let mut next_positional = 0usize;
                    let mut changed = false;
                    for arg in args {
                        let (param_index, expr) = match arg {
                            hir::CallArg::Positional(expr) => {
                                let param_index = next_positional;
                                next_positional += 1;
                                if param_index >= inferred.len() {
                                    continue;
                                }
                                (param_index, expr)
                            }
                            hir::CallArg::Named { name, value, .. } => {
                                let Some(param_index) =
                                    fun.params.iter().position(|param| param.name == *name)
                                else {
                                    continue;
                                };
                                (param_index, value)
                            }
                        };
                        let concrete_ty = self.resolve_expr_concrete_type(expr).unwrap_or(expr.ty);
                        if self.cg_ty_of(inferred[param_index]).is_none()
                            && self.cg_ty_of(concrete_ty).is_some()
                        {
                            inferred[param_index] = concrete_ty;
                            changed = true;
                        }
                    }
                    changed.then_some(inferred)
                } else {
                    None
                };
                let needs_published_sig = fallback_param_tys
                    .iter()
                    .any(|&ty| self.cg_ty_of(ty).is_none())
                    || self.cg_ty_of(fallback_return_ty).is_none();
                let (param_tys, return_ty) = if needs_published_sig {
                    self.published_callable_signature(fqn)
                        .or_else(|| {
                            (dispatch_fqn != fqn)
                                .then(|| self.published_callable_signature(dispatch_fqn))
                                .flatten()
                        })
                        .or_else(|| {
                            inferred_param_tys
                                .clone()
                                .map(|param_tys| (param_tys, fallback_return_ty))
                        })
                        .unwrap_or((fallback_param_tys, fallback_return_ty))
                } else {
                    (fallback_param_tys, fallback_return_ty)
                };
                (param_names, param_tys, return_ty)
            }
            CallableSig::Mir(fun, types) => {
                let mir_types = *types;
                let param_names = fun
                    .params
                    .iter()
                    .map(|param| param.name.clone())
                    .collect::<Vec<_>>();
                let fallback_param_tys = fun
                    .params
                    .iter()
                    .map(|param| {
                        self.equivalent_codegen_type_id(mir_types, param.ty)
                            .unwrap_or(param.ty)
                    })
                    .collect::<Vec<_>>();
                let fallback_return_ty = self
                    .equivalent_codegen_type_id(mir_types, fun.return_ty)
                    .unwrap_or(fun.return_ty);
                let needs_published_sig = fallback_param_tys
                    .iter()
                    .any(|&ty| self.cg_ty_of(ty).is_none())
                    || self.cg_ty_of(fallback_return_ty).is_none();
                let (param_tys, return_ty) = if needs_published_sig {
                    self.published_callable_signature(fqn)
                        .or_else(|| {
                            (dispatch_fqn != fqn)
                                .then(|| self.published_callable_signature(dispatch_fqn))
                                .flatten()
                        })
                        .unwrap_or((fallback_param_tys, fallback_return_ty))
                } else {
                    (fallback_param_tys, fallback_return_ty)
                };
                (param_names, param_tys, return_ty)
            }
        };
        for ty in &mut param_tys {
            *ty = self.canonical_builtin_signature_ty(*ty);
        }
        return_ty = self.canonical_builtin_signature_ty(return_ty);
        if param_names.len() != param_tys.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "call signature arity mismatch",
                at: callee_span.into(),
            });
        }

        if args.len() != param_tys.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "call arity mismatch",
                at: span.into(),
            });
        }
        let native_abi = if callable_abi.uses_native_abi() {
            Some(self.classify_direct_extern_native_callable(
                callee_span,
                fqn,
                &param_tys,
                return_ty,
            )?)
        } else {
            None
        };
        let ret_cg = native_abi
            .as_ref()
            .map(|abi| abi.return_abi.cg_ty)
            .or_else(|| match &sig_fun {
                CallableSig::Hir(_) => self.cg_ty_of(return_ty),
                CallableSig::Mir(fun, types) => {
                    let mir_types = *types;
                    self.cg_ty_of_mir_type(mir_types, fun.return_ty)
                        .or_else(|| self.cg_ty_of(return_ty))
                }
            })
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "call return type",
                at: span.into(),
            })?;
        let hidden_sret_result_ty = if native_abi.is_some() {
            None
        } else {
            self.hidden_sret_result_ty(callee_span, ret_cg)?
        };
        let evaluated_args = self.codegen_bound_call_args(
            BoundCallArgsSpec {
                span,
                callee_span,
                kind: "call arg binding",
                abi_mode: if native_abi.is_some() {
                    CallArgAbiMode::Native
                } else {
                    CallArgAbiMode::Ordinary
                },
            },
            &param_names,
            &param_tys,
            args,
        )?;

        let mut llvm_args: Vec<BasicMetadataValueEnum<'ctx>> = Vec::with_capacity(
            evaluated_args.len()
                + usize::from(hidden_sret_result_ty.is_some())
                + self.explicit_effect_hidden_abi_param_count(uses_explicit_effect_hidden_abi)
                    as usize,
        );
        let sret_result_slot = if hidden_sret_result_ty.is_some() {
            let slot = self.create_entry_alloca(callee_span, "call_sret", ret_cg)?;
            llvm_args.push(slot.into());
            Some(slot)
        } else {
            None
        };
        let effect_outcome_slot = if uses_explicit_effect_hidden_abi {
            let slot = self.alloc_effect_outcome_slot(span, "direct_call")?;
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
            .get(fqn)
            .map(|extern_fun| extern_fun.symbol.as_str())
            .unwrap_or(signature_owner_fqn);
        let llvm_fun = match self.module.get_function(llvm_name) {
            Some(function) => function,
            None => match sig_fun {
                CallableSig::Hir(fun) => self.declare_top_level_fun_with_signature_override(
                    fun, llvm_name, &param_tys, return_ty,
                )?,
                CallableSig::Mir(ref fun, _) => {
                    self.declare_materialized_top_level_fun_with_symbol(fun, llvm_name)?
                }
            },
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
                let call_site = cg.builder.build_call(llvm_fun, &llvm_args, "call")?;
                if let Some(result_ty) = hidden_sret_result_ty {
                    cg.add_sret_attribute_to_call(call_site, 0, result_ty);
                }
                call_site.set_call_convention(cg.llvm_call_convention_for_fqn(signature_owner_fqn));
                Ok(call_site)
            })
        };
        self.release_evaluated_call_arg_roots(&evaluated_args);
        let call_site = call_site_result?;
        if let Some(result_ptr) = sret_result_slot {
            self.sync_hidden_sret_result_roots(span, ret_cg, result_ptr, "call_sret")?;
        }
        let deferred_direct_result = if sret_result_slot.is_none() {
            self.defer_direct_call_result(span, ret_cg, call_site, "call_direct_result")?
        } else {
            None
        };
        if let Some(outcome_slot) = effect_outcome_slot {
            self.maybe_record_active_suspend_site_effect_outcome(span, outcome_slot);
            self.emit_ordinary_call_effect_propagation_check_from_outcome(
                span,
                outcome_slot,
                "direct_call_effect",
            )?;
        }

        match ret_cg {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            _ => {
                if let Some(result_ptr) = sret_result_slot {
                    self.load_hidden_sret_result_from_ptr(span, ret_cg, result_ptr, "call_sret")
                } else {
                    self.materialize_deferred_cg_value(
                        span,
                        "call_direct_result_reload",
                        deferred_direct_result.ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "call deferred return value",
                            at: span.into(),
                        })?,
                    )
                }
            }
        }
    }

    pub(in crate::llvm::codegen) fn emit_enter_native_for_extern_call_impl(
        &mut self,
        at: crate::span::Span,
    ) -> Result<(), LlvmEmitError> {
        let slot_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let slots_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let i32_ty = self.context.i32_type();
        let explicit_frame_enabled = self
            .function_cx
            .explicit_frame_layout
            .frame_storage
            .is_some();

        let slots = self
            .collect_conservative_gc_root_slots(at)?
            .into_iter()
            .map(|(id, slot, _, frame_slot)| {
                let root_slot = if explicit_frame_enabled {
                    frame_slot
                } else {
                    slot
                };
                (id, root_slot)
            })
            .collect::<Vec<_>>();

        let (slots_base, slots_len) = if slots.is_empty() {
            (slots_ptr_ty.const_null(), i32_ty.const_zero())
        } else {
            let arr_ty = slot_ptr_ty.array_type(slots.len() as u32);
            let arr_ptr = self.create_entry_alloca_raw(at, "native_root_slots", arr_ty.into())?;
            let base =
                self.builder
                    .build_pointer_cast(arr_ptr, slots_ptr_ty, "native_root_slots_base")?;

            for (idx, (_id, local_ptr)) in slots.iter().enumerate() {
                let slot_ptr = self.builder.build_pointer_cast(
                    *local_ptr,
                    slot_ptr_ty,
                    "native_root_slot_cast",
                )?;
                let idx_v = i32_ty.const_int(idx as u64, false);
                let elem_ptr = unsafe {
                    self.builder.build_in_bounds_gep(
                        slot_ptr_ty,
                        base,
                        &[idx_v],
                        &format!("native_root_slot_gep_{idx}"),
                    )?
                };
                let _ = self.builder.build_store(elem_ptr, slot_ptr)?;
            }

            (base, i32_ty.const_int(slots.len() as u64, false))
        };

        let enter = self.declare_runtime_enter_native();
        let enter_args: [BasicMetadataValueEnum<'ctx>; 2] = [slots_base.into(), slots_len.into()];
        let _ = self
            .builder
            .build_call(enter, &enter_args, "enter_native")?;
        Ok(())
    }

    pub(in crate::llvm::codegen) fn try_codegen_class_vtable_call_impl(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        let dispatch_fqn = direct_call_dispatch_fqn(fqn);
        let Some((owner_fqn, method_name)) = dispatch_fqn.rsplit_once('.') else {
            return Ok(None);
        };

        let Some(slots) = self.class_vtables.get(owner_fqn) else {
            return Ok(None);
        };
        if slots.is_empty() {
            return Ok(None);
        }

        let Some((receiver_arg, _)) = args.split_first() else {
            return Ok(None);
        };
        let hir::CallArg::Positional(receiver_expr) = receiver_arg else {
            return Ok(None);
        };
        if !matches!(
            self.dispatch_call_kind_for_receiver(span, receiver_expr.ty)?,
            Some(hir::DispatchCallKind::Virtual)
        ) {
            return Ok(None);
        }

        let explicit_params_len = args.len().saturating_sub(1) as u32;
        let slot = slots
            .iter()
            .find(|slot| slot.name == method_name && slot.params_len == explicit_params_len)
            .map(|slot| slot.slot);
        let Some(slot) = slot else {
            return Ok(None);
        };

        let sig_fun = self
            .fun_index
            .get(fqn)
            .or_else(|| self.fun_index.get(dispatch_fqn))
            .copied()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "vtable call callee type",
                at: callee_span.into(),
            })?;
        let uses_explicit_effect_hidden_abi =
            self.direct_call_abi_identity(fqn).uses_effect_bridge_abi();

        if args.len() != sig_fun.params.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "vtable call arity mismatch",
                at: span.into(),
            });
        }

        let ret_cg =
            self.cg_ty_of(sig_fun.return_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "vtable call return type",
                    at: span.into(),
                })?;
        let hidden_sret_result_ty = self.hidden_sret_result_ty(callee_span, ret_cg)?;
        let mut llvm_param_tys: Vec<BasicMetadataTypeEnum<'ctx>> = Vec::with_capacity(
            sig_fun.params.len()
                + usize::from(hidden_sret_result_ty.is_some())
                + self.explicit_effect_hidden_abi_param_count(uses_explicit_effect_hidden_abi)
                    as usize,
        );
        if hidden_sret_result_ty.is_some() {
            llvm_param_tys.push(self.context.ptr_type(AddressSpace::default()).into());
        }
        if uses_explicit_effect_hidden_abi {
            self.push_explicit_effect_hidden_abi_param_tys(&mut llvm_param_tys);
        }
        for param in &sig_fun.params {
            llvm_param_tys.push(
                self.ordinary_param_abi(callee_span, param.ty)?
                    .llvm_param_ty(),
            );
        }

        let llvm_fun_ty = match (hidden_sret_result_ty, ret_cg) {
            (Some(_), _) | (None, CgTy::Unit | CgTy::Never) => {
                self.context.void_type().fn_type(&llvm_param_tys, false)
            }
            (None, other) => self
                .llvm_basic_type_of(callee_span, other)?
                .fn_type(&llvm_param_tys, false),
        };

        let param_names: Vec<String> = sig_fun
            .params
            .iter()
            .map(|param| param.name.clone())
            .collect();
        let param_tys: Vec<TypeId> = sig_fun.params.iter().map(|param| param.ty).collect();
        let evaluated_args = self.codegen_bound_call_args(
            BoundCallArgsSpec {
                span,
                callee_span,
                kind: "vtable call arg binding",
                abi_mode: CallArgAbiMode::Ordinary,
            },
            &param_names,
            &param_tys,
            args,
        )?;
        let receiver_ptr = evaluated_args
            .first()
            .and_then(|arg| arg.pointer_value)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "vtable call receiver type",
                at: callee_span.into(),
            })?;
        let deferred_receiver =
            self.defer_gc_ref_pointer(callee_span, "vtable_call_receiver", receiver_ptr)?;
        let mut llvm_args: Vec<BasicMetadataValueEnum<'ctx>> = Vec::with_capacity(
            evaluated_args.len()
                + usize::from(hidden_sret_result_ty.is_some())
                + self.explicit_effect_hidden_abi_param_count(uses_explicit_effect_hidden_abi)
                    as usize,
        );
        let sret_result_slot = if hidden_sret_result_ty.is_some() {
            let slot = self.create_entry_alloca(callee_span, "vtable_call_sret", ret_cg)?;
            llvm_args.push(slot.into());
            Some(slot)
        } else {
            None
        };
        let effect_outcome_slot = if uses_explicit_effect_hidden_abi {
            let slot = self.alloc_effect_outcome_slot(span, "vtable_call")?;
            llvm_args.push(self.current_effect_ctx_arg().into());
            llvm_args.push(self.llvm_gc_i8_ptr_type().const_null().into());
            llvm_args.push(slot.into());
            Some(slot)
        } else {
            None
        };
        llvm_args.extend(evaluated_args.iter().map(|arg| arg.value));

        let receiver_ptr = self.reload_deferred_gc_ref_without_clearing(
            callee_span,
            "vtable_call_receiver_reload",
            &deferred_receiver,
        )?;
        let fn_i8 = self.load_class_vtable_slot_fn_ptr_i8(span, receiver_ptr, slot)?;
        let typed_fn_ptr = self.builder.build_pointer_cast(
            fn_i8,
            self.llvm_ptr_type(AddressSpace::default()),
            "vtable_fn_typed",
        )?;

        let call_site_result = self.with_conservative_gc_local_root_spills(span, |cg| {
            let call_site = cg.builder.build_indirect_call(
                llvm_fun_ty,
                typed_fn_ptr,
                &llvm_args,
                "call_vtable",
            )?;
            if let Some(result_ty) = hidden_sret_result_ty {
                cg.add_sret_attribute_to_call(call_site, 0, result_ty);
            }
            call_site.set_call_convention(cg.llvm_call_convention_for_fqn(fqn));
            Ok(call_site)
        });
        self.release_evaluated_call_arg_roots(&evaluated_args);
        let call_site = call_site_result?;
        if let Some(result_ptr) = sret_result_slot {
            self.sync_hidden_sret_result_roots(span, ret_cg, result_ptr, "vtable_call_sret")?;
        }
        let deferred_direct_result = if sret_result_slot.is_none() {
            self.defer_direct_call_result(span, ret_cg, call_site, "vtable_call_direct_result")?
        } else {
            None
        };
        if let Some(outcome_slot) = effect_outcome_slot {
            self.maybe_record_active_suspend_site_effect_outcome(span, outcome_slot);
            self.emit_ordinary_call_effect_propagation_check_from_outcome(
                span,
                outcome_slot,
                "vtable_call_effect",
            )?;
        }

        match ret_cg {
            CgTy::Unit => Ok(Some(CgValue::unit())),
            CgTy::Never => Ok(Some(CgValue::never())),
            _ => Ok(Some(if let Some(result_ptr) = sret_result_slot {
                self.load_hidden_sret_result_from_ptr(span, ret_cg, result_ptr, "vtable_call_sret")?
            } else {
                self.materialize_deferred_cg_value(
                    span,
                    "vtable_call_direct_result_reload",
                    deferred_direct_result.ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "vtable call deferred return value",
                        at: span.into(),
                    })?,
                )?
            })),
        }
    }

    pub(in crate::llvm::codegen) fn codegen_funptr_value_call_impl(
        &mut self,
        funptr_addr: IntValue<'ctx>,
        funptr_int_ty: IntTy,
        call: FunPtrCallSpec<'_>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let FunPtrCallSpec {
            span,
            callee_span,
            fun_ty,
            args,
        } = call;
        let expected_arity = fun_ty.params.len() + usize::from(fun_ty.receiver.is_some());
        if args.len() != expected_arity {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "funptr call arity mismatch",
                at: span.into(),
            });
        }

        let param_tys = self.callable_value_param_tys(fun_ty);
        let native_abi =
            self.classify_funptr_native_callable(callee_span, &param_tys, fun_ty.return_ty)?;
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
                .build_int_to_ptr(casted_addr, fun_ptr_ty, "funptr_typed")?;

        let mut llvm_args: Vec<BasicMetadataValueEnum<'ctx>> = Vec::with_capacity(args.len());
        let evaluated_args = self.codegen_callable_value_args(
            span,
            callee_span,
            fun_ty,
            args,
            "funptr call arg binding",
            CallArgAbiMode::Native,
        )?;
        for arg in &evaluated_args {
            llvm_args.push(arg.value);
        }

        let call_site_result = self.emit_native_callable_call(
            span,
            &native_abi,
            NativeCallableTarget::Indirect {
                fn_ty: native_abi.fn_ty,
                ptr: typed_fn_ptr,
                call_name: "call_funptr",
            },
            &llvm_args,
        );
        self.release_evaluated_call_arg_roots(&evaluated_args);
        let call_site = call_site_result?;
        let deferred_direct_result =
            self.defer_direct_call_result(span, ret_cg, call_site, "funptr_call_direct_result")?;

        match ret_cg {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            _ => self.materialize_deferred_cg_value(
                span,
                "funptr_call_direct_result_reload",
                deferred_direct_result.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "funptr call deferred return value",
                    at: span.into(),
                })?,
            ),
        }
    }

    pub(in crate::llvm::codegen) fn codegen_function_value_call_impl(
        &mut self,
        local: &CgLocal<'ctx>,
        call: CallableValueCallSpec<'_>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let callee_span = call.callee_span;
        let CgTy::Ref = local.ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "function value local type",
                at: callee_span.into(),
            });
        };

        let llvm_local_ty = self.llvm_basic_type_of(callee_span, local.ty)?;
        let local_ptr = self.local_ptr_for_use(callee_span, *local, "load_closure_obj_slot")?;
        let closure_obj_i8 = self
            .builder
            .build_load(llvm_local_ty, local_ptr, "load_closure_obj")?
            .into_pointer_value();

        self.codegen_function_value_call_from_closure_obj(closure_obj_i8, call)
    }

    pub(in crate::llvm::codegen) fn codegen_function_value_call_from_closure_obj_impl(
        &mut self,
        closure_obj_i8: PointerValue<'ctx>,
        call: CallableValueCallSpec<'_>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let CallableValueCallSpec {
            span,
            callee_span,
            call_may_suspend,
            fun_ty,
            args,
        } = call;
        let expected_arity = fun_ty.params.len() + usize::from(fun_ty.receiver.is_some());
        if args.len() != expected_arity {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "function value call arity mismatch",
                at: span.into(),
            });
        }
        let deferred_closure =
            self.defer_gc_ref_pointer(callee_span, "closure_call_obj", closure_obj_i8)?;

        let closure_ty = self.llvm_closure_object_type();
        let closure_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();

        let ret_cg = self
            .cg_ty_of(fun_ty.return_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "function value call return type",
                at: callee_span.into(),
            })?;
        let hidden_sret_result_ty = self.hidden_sret_result_ty(callee_span, ret_cg)?;

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
            llvm_param_tys.push(
                self.ordinary_param_abi(callee_span, receiver_ty)?
                    .llvm_param_ty(),
            );
        }
        for ty in &fun_ty.params {
            llvm_param_tys.push(self.ordinary_param_abi(callee_span, *ty)?.llvm_param_ty());
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
                "aggregate function-value returns should have been lowered through hidden sret"
            ),
        };

        let mut llvm_args: Vec<BasicMetadataValueEnum<'ctx>> = Vec::with_capacity(
            1 + args.len()
                + usize::from(hidden_sret_result_ty.is_some())
                + self.explicit_effect_hidden_abi_param_count(call_may_suspend) as usize,
        );
        let sret_result_slot = if hidden_sret_result_ty.is_some() {
            let slot = self.create_entry_alloca(callee_span, "closure_call_sret", ret_cg)?;
            llvm_args.push(slot.into());
            Some(slot)
        } else {
            None
        };
        let effect_outcome_slot = if call_may_suspend {
            let slot = self.alloc_effect_outcome_slot(span, "closure_call")?;
            llvm_args.push(self.current_effect_ctx_arg().into());
            llvm_args.push(self.llvm_gc_i8_ptr_type().const_null().into());
            llvm_args.push(slot.into());
            Some(slot)
        } else {
            None
        };
        let evaluated_args = self.codegen_callable_value_args(
            span,
            callee_span,
            fun_ty,
            args,
            "function value call arg binding",
            CallArgAbiMode::Ordinary,
        )?;

        let closure_obj_i8 = self.reload_deferred_gc_ref_without_clearing(
            callee_span,
            "closure_call_obj_reload",
            &deferred_closure,
        )?;
        let closure_ptr =
            self.builder
                .build_pointer_cast(closure_obj_i8, closure_ptr_ty, "closure_obj_ptr")?;
        let env_ptr_gep =
            self.builder
                .build_struct_gep(closure_ty, closure_ptr, 1, "closure_env_gep")?;
        let fn_ptr_gep =
            self.builder
                .build_struct_gep(closure_ty, closure_ptr, 2, "closure_fn_gep")?;
        let env_ptr = self
            .builder
            .build_load(gc_i8_ptr_ty, env_ptr_gep, "closure_env")?
            .into_pointer_value();
        let fn_ptr_raw = self
            .builder
            .build_load(i8_ptr_ty, fn_ptr_gep, "closure_fn")?
            .into_pointer_value();
        let typed_fn_ptr = self.builder.build_pointer_cast(
            fn_ptr_raw,
            self.llvm_ptr_type(AddressSpace::default()),
            "closure_fn_typed",
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
                "call_closure",
            )?;
            if let Some(result_ty) = hidden_sret_result_ty {
                cg.add_sret_attribute_to_call(call_site, 0, result_ty);
            }
            Ok(call_site)
        });
        self.release_evaluated_call_arg_roots(&evaluated_args);
        let call_site = call_site_result?;
        if let Some(result_ptr) = sret_result_slot {
            self.sync_hidden_sret_result_roots(span, ret_cg, result_ptr, "closure_call_sret")?;
        }
        let deferred_direct_result = if sret_result_slot.is_none() {
            self.defer_direct_call_result(span, ret_cg, call_site, "closure_call_direct_result")?
        } else {
            None
        };
        if let Some(outcome_slot) = effect_outcome_slot {
            self.maybe_record_active_suspend_site_effect_outcome(span, outcome_slot);
            self.emit_ordinary_call_effect_propagation_check_from_outcome(
                span,
                outcome_slot,
                "closure_call_effect",
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
                        "closure_call_sret",
                    )
                } else {
                    self.materialize_deferred_cg_value(
                        span,
                        "closure_call_direct_result_reload",
                        deferred_direct_result.ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "function value deferred return value",
                            at: span.into(),
                        })?,
                    )
                }
            }
        }
    }

    pub(in crate::llvm::codegen) fn try_codegen_interface_itable_call_impl(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        let dispatch_fqn = direct_call_dispatch_fqn(fqn);
        let Some((owner_fqn, method_name)) = dispatch_fqn.rsplit_once('.') else {
            return Ok(None);
        };

        let Some(iface) = self.interfaces.get(owner_fqn) else {
            return Ok(None);
        };
        if args.is_empty() {
            return Ok(None);
        }

        let Some((receiver_arg, _)) = args.split_first() else {
            return Ok(None);
        };
        let hir::CallArg::Positional(receiver_expr) = receiver_arg else {
            return Ok(None);
        };
        if !matches!(
            self.dispatch_call_kind_for_receiver(span, receiver_expr.ty)?,
            Some(hir::DispatchCallKind::Interface)
        ) {
            return Ok(None);
        }

        let explicit_params_len = args.len().saturating_sub(1) as u32;
        let known_receiver_subclasses =
            crate::devirtualize::collect_known_receiver_subclasses(self.direct_supertypes);
        if let Some(target_fqn) = crate::devirtualize::try_devirtualize_dispatch_target(
            hir::DispatchCallKind::Interface,
            owner_fqn,
            method_name,
            explicit_params_len as usize,
            receiver_expr.ty,
            self.types,
            crate::devirtualize::DispatchTargetFacts {
                known_receiver_subclasses: &known_receiver_subclasses,
                class_vtables: self.class_vtables,
                interfaces: self.interfaces,
                class_itables: self.class_itables,
            },
        ) {
            return self
                .codegen_top_level_fun_call(span, callee_span, &target_fqn, args)
                .map(Some);
        }

        let mut candidates = iface
            .method_slots
            .iter()
            .filter(|slot| slot.name == method_name && slot.params_len == explicit_params_len);
        let Some(first) = candidates.next() else {
            return Ok(None);
        };
        if candidates.next().is_some() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "itable call slot ambiguous",
                at: callee_span.into(),
            });
        }
        let slot = first.slot;

        let sig_fun = self
            .fun_index
            .get(fqn)
            .or_else(|| self.fun_index.get(dispatch_fqn))
            .copied()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "itable call callee type",
                at: callee_span.into(),
            })?;
        let uses_explicit_effect_hidden_abi =
            self.direct_call_abi_identity(fqn).uses_effect_bridge_abi();

        if args.len() != sig_fun.params.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "itable call arity mismatch",
                at: span.into(),
            });
        }

        let receiver_value = self.codegen_expr(receiver_expr)?;
        let receiver_value = self.coerce_value(callee_span, receiver_value, CgTy::Ref)?;
        let Some(BasicValueEnum::PointerValue(receiver_ptr)) = receiver_value.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "itable call receiver type",
                at: callee_span.into(),
            });
        };
        let deferred_receiver =
            self.defer_gc_ref_pointer(callee_span, "itable_call_receiver", receiver_ptr)?;

        let explicit_param_names = sig_fun.params[1..]
            .iter()
            .map(|param| param.name.clone())
            .collect::<Vec<_>>();
        let explicit_param_tys = sig_fun.params[1..]
            .iter()
            .map(|param| param.ty)
            .collect::<Vec<_>>();
        let evaluated_explicit_args = self.codegen_bound_call_args(
            BoundCallArgsSpec {
                span,
                callee_span,
                kind: "itable call arg binding",
                abi_mode: CallArgAbiMode::Ordinary,
            },
            &explicit_param_names,
            &explicit_param_tys,
            &args[1..],
        )?;
        let explicit_args = evaluated_explicit_args
            .iter()
            .map(|arg| arg.value)
            .collect::<Vec<_>>();

        let receiver_ptr = self.reload_deferred_gc_ref_without_clearing(
            callee_span,
            "itable_call_receiver_reload",
            &deferred_receiver,
        )?;
        let lookup =
            self.lookup_interface_itable_slot(span, receiver_ptr, iface.interface_id, slot)?;
        let result = self.emit_interface_dispatch_indirect_call(
            span,
            callee_span,
            owner_fqn,
            slot,
            sig_fun,
            uses_explicit_effect_hidden_abi,
            receiver_ptr,
            lookup,
            &explicit_args,
        )?;
        self.release_evaluated_call_arg_roots(&evaluated_explicit_args);
        Ok(Some(result))
    }
}
