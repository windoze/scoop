//! MIR member access and member place lowering.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn codegen_mir_member_access(
        &mut self,
        span: crate::span::Span,
        receiver: &crate::mir::Operand,
        member: &crate::mir::MemberAccessMetadata,
        mir_ctx: MirBodyCodegenCtx<'_, 'ctx>,
        target_cg: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if let Some(fqn) = self.mir_member_resolved_top_level_value_fqn(member) {
            let value = if self.lookup_object_property_by_fqn(fqn).is_some() {
                self.codegen_object_property_access(span, fqn)?
            } else if let Some(value) =
                self.try_codegen_qualified_enum_unit_variant_value(span, fqn)?
            {
                value
            } else {
                self.codegen_top_level_value_ref(span, fqn)?
            };
            return self.coerce_value(span, value, target_cg);
        }
        let place = self.codegen_mir_member_place(span, receiver, member, mir_ctx, false)?;
        let same_layout = self.cg_ty_layout_equivalent(place.field_cg, target_cg);
        if !same_layout {
            return Err(frontend_error(format!(
                "pass MIR member access result type drift: field={} target={}",
                self.describe_cg_ty(place.field_cg),
                self.describe_cg_ty(target_cg),
            )));
        }
        if place.field_cg == CgTy::Unit {
            return self.coerce_value(span, CgValue::unit(), target_cg);
        }
        let llvm_ty = self.llvm_basic_type_of(span, place.field_cg)?;
        let loaded = self
            .builder
            .build_load(llvm_ty, place.ptr, "pass_mir_member_load")?;
        if let Some(alignment) = place.packed_alignment
            && let Some(inst) = loaded.as_instruction_value()
        {
            inst.set_alignment(alignment)?;
        }
        let value = self.cg_value_from_loaded(span, place.field_cg, loaded)?;
        if place.field_cg != target_cg {
            return Ok(CgValue {
                ty: target_cg,
                value: value.value,
            });
        }
        self.coerce_value(span, value, target_cg)
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_mir_enum_variant_ctor_call(
        &mut self,
        span: crate::span::Span,
        enum_ty: TypeId,
        variant_name: &str,
        args: &[crate::mir::CallArg],
        payload: &crate::mir::AggregateTransportMetadata,
        _body: &crate::mir::Body,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let enum_ty = self
            .equivalent_codegen_type_id(mir_types, enum_ty)
            .unwrap_or_else(|| panic!("codegen_mir_enum_variant_ctor_call: verifier accepted enum ctor TypeStore drift"));
        let layout = self.cg_enum_layout(span, enum_ty)?;
        let variant = layout
            .variants
            .iter()
            .find(|variant| variant.name == variant_name)
            .unwrap_or_else(|| panic!("codegen_mir_enum_variant_ctor_call: verifier accepted unknown enum variant `{variant_name}`"))
            .clone();
        if variant.fields.len() != args.len() {
            panic!("codegen_mir_enum_variant_ctor_call: verifier accepted enum ctor arity drift");
        }
        if !self.mir_enum_payload_schema_matches(mir_types, enum_ty, &variant, args, payload) {
            panic!(
                "codegen_mir_enum_variant_ctor_call: verifier accepted enum payload schema drift"
            );
        }
        let mut field_values = Vec::with_capacity(args.len());
        for (idx, (field_cg, arg)) in variant.fields.iter().copied().zip(args).enumerate() {
            if arg.name.is_some() {
                panic!("codegen_mir_enum_variant_ctor_call: verifier accepted named enum ctor arg");
            }
            let value =
                self.codegen_mir_operand_expected(arg.span, &arg.value, slots, Some(field_cg))?;
            let coerced = self.coerce_value(arg.span, value, field_cg)?;
            let deferred = self.defer_gc_sensitive_cg_value(
                arg.span,
                &format!("pass_mir_enum_ctor_field_{idx}"),
                coerced,
            )?;
            field_values.push((arg.span, field_cg, deferred));
        }
        self.build_enum_variant_value_from_field_values(span, enum_ty, variant_name, &field_values)
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_mir_store_member(
        &mut self,
        span: crate::span::Span,
        receiver: &crate::mir::Operand,
        member: &crate::mir::MemberAccessMetadata,
        value: &crate::mir::Operand,
        value_ty: TypeId,
        continuation_route: &crate::mir::StoredContinuationRoutePublication,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<(), LlvmEmitError> {
        mir_store_member_continuation_route_is_lowerable(span, body, continuation_route)?;

        let mir_ctx = MirBodyCodegenCtx {
            body,
            mir_types,
            slots,
        };
        let place = self.codegen_mir_member_place(span, receiver, member, mir_ctx, true)?;
        if !place.writable {
            // User-facing non-writable target errors are owned by typecheck.
            unreachable!(
                "typecheck must reject non-writable MIR member store targets before LLVM codegen"
            );
        }
        let value_cg = self
            .cg_ty_of_mir_type(mir_types, value_ty)
            .unwrap_or_else(|| panic!("codegen_mir_store_member: verifier accepted non-codegen member store value type"));
        let operand_cg = self
            .mir_operand_cg_ty(body, mir_types, value)
            .unwrap_or_else(|| {
                panic!(
                    "codegen_mir_store_member: verifier accepted missing member store operand type"
                )
            });
        if !self.cg_ty_layout_equivalent(value_cg, operand_cg) {
            panic!(
                "codegen_mir_store_member: verifier accepted member store value/operand type drift"
            );
        }
        if !self.cg_ty_layout_equivalent(value_cg, place.field_cg) {
            panic!(
                "codegen_mir_store_member: verifier accepted member store field/value type drift"
            );
        }

        let value = self.codegen_mir_operand_expected(span, value, slots, Some(place.field_cg))?;
        let stored = self.coerce_value(span, value, place.field_cg)?;
        let _ = self.store_local_value(span, place.ptr, place.field_cg, stored)?;
        Ok(())
    }

    pub(in crate::llvm::codegen) fn codegen_mir_store_top_level_var(
        &mut self,
        span: crate::span::Span,
        fqn: &str,
        value: &crate::mir::Operand,
        _value_ty: TypeId,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<(), LlvmEmitError> {
        if let Some(global) = self.materialized_extern_global_root(fqn).cloned() {
            if !global.mutable {
                panic!(
                    "codegen_mir_store_top_level_var: verifier accepted immutable MIR extern global store target"
                );
            }
            let target_cg = self.expect_cg_ty_of(global.ty, "MIR extern global store target type");
            let raw = self.codegen_mir_operand_expected(span, value, slots, Some(target_cg))?;
            let stored = self.coerce_value(span, raw, target_cg)?;
            let global = self.declare_mir_extern_global(&global)?;
            let _ = self.store_local_value(span, global.as_pointer_value(), target_cg, stored)?;
            return Ok(());
        }

        if let Some(global) = self.extern_globals.get(fqn).cloned() {
            if !global.mutable {
                panic!(
                    "codegen_mir_store_top_level_var: verifier accepted immutable HIR extern global store target"
                );
            }
            let target_cg = self.expect_cg_ty_of(global.ty, "HIR extern global store target type");
            let raw = self.codegen_mir_operand_expected(span, value, slots, Some(target_cg))?;
            let stored = self.coerce_value(span, raw, target_cg)?;
            let global = self.declare_extern_global(&global)?;
            let _ = self.store_local_value(span, global.as_pointer_value(), target_cg, stored)?;
            return Ok(());
        }

        let var = self
            .top_level_vars
            .get(fqn)
            .unwrap_or_else(|| panic!("codegen_mir_store_top_level_var: verifier accepted missing top-level var store target `{fqn}`"));
        let target_cg = self.expect_cg_ty_of(var.ty, "MIR top-level var store target type");
        let raw = self.codegen_mir_operand_expected(span, value, slots, Some(target_cg))?;
        let stored = self.coerce_value(span, raw, target_cg)?;
        let global = self.declare_top_level_var_global(var)?;
        let _ = self.store_local_value(span, global.as_pointer_value(), target_cg, stored)?;
        Ok(())
    }

    pub(in crate::llvm::codegen) fn codegen_mir_member_place(
        &mut self,
        span: crate::span::Span,
        receiver: &crate::mir::Operand,
        member: &crate::mir::MemberAccessMetadata,
        mir_ctx: MirBodyCodegenCtx<'_, 'ctx>,
        require_writable: bool,
    ) -> Result<MirMemberPlace<'ctx>, LlvmEmitError> {
        let field_fqn = mir_member_value_fqn_for_codegen(span, member)?;
        let receiver_type_id = self.mir_member_receiver_codegen_type_id(
            span,
            mir_ctx.body,
            mir_ctx.mir_types,
            receiver,
            member,
        )?;
        if let Some((class, field_idx, field_cg)) =
            self.lookup_class_field_by_fqn(field_fqn, span, Some(receiver_type_id))?
        {
            let receiver_cg = self
                .mir_operand_cg_ty(mir_ctx.body, mir_ctx.mir_types, receiver)
                .unwrap_or_else(|| panic!("codegen_mir_member_place: verifier accepted missing class member receiver operand type"));
            if receiver_cg == CgTy::Ref {
                let field = class.fields.get(field_idx as usize).unwrap_or_else(|| {
                    panic!(
                        "codegen_mir_member_place: member verifier accepted class field index drift"
                    )
                });
                if require_writable && !field.mutable {
                    // User-facing immutable member-store errors are owned by typecheck.
                    unreachable!(
                        "typecheck must reject immutable class member stores before LLVM codegen"
                    );
                }
                let receiver_value = self.codegen_mir_operand_expected(
                    span,
                    receiver,
                    mir_ctx.slots,
                    Some(CgTy::Ref),
                )?;
                let receiver_value = self.coerce_value(span, receiver_value, CgTy::Ref)?;
                let raw = self.expect_cg_value(receiver_value, "MIR class member receiver");
                let obj_ptr = self.expect_pointer_value(raw, "MIR class member receiver");
                let ptr = self.codegen_class_field_ptr(span, &class, obj_ptr, field_idx)?;
                return Ok(MirMemberPlace {
                    ptr,
                    field_cg,
                    writable: field.mutable,
                    packed_alignment: None,
                });
            }
        }

        let receiver_cg = self.expect_cg_ty_of(receiver_type_id, "MIR member receiver type");
        let CgTy::Struct(struct_ty) = receiver_cg else {
            return Err(frontend_error(format!(
                "pass MIR member field target `{field_fqn}` receiver_ty=t{} receiver_cg={}",
                receiver_type_id.as_u32(),
                self.describe_cg_ty(receiver_cg),
            )));
        };
        let (field_idx, field_cg) = self.lookup_struct_field(struct_ty, field_fqn, span)?;
        let crate::mir::Operand::Local(local) = receiver else {
            panic!("codegen_mir_member_place: verifier accepted non-local member store receiver");
        };
        let slot = self.mir_local_slot(span, mir_ctx.slots, *local)?;
        if slot.cg_ty != CgTy::Struct(struct_ty) {
            panic!("codegen_mir_member_place: verifier accepted member receiver slot type drift");
        }
        let local_ptr = self.local_ptr_for_use(
            span,
            CgLocal {
                hir_ty: None,
                call_may_suspend: false,
                ty: slot.cg_ty,
                ptr: slot.ptr,
                frame_backing_ptr: None,
                mutable: false,
            },
            "pass_mir_member_base",
        )?;
        let llvm_struct_ty = self.llvm_struct_type(span, struct_ty)?;
        let ptr = self.builder.build_struct_gep(
            llvm_struct_ty,
            local_ptr,
            field_idx,
            "pass_mir_member_gep",
        )?;
        let packed_alignment = if let Some(pack_n) = self
            .struct_clayout(struct_ty)
            .and_then(|layout| layout.packed)
        {
            if require_writable {
                panic!(
                    "codegen_mir_member_place: verifier accepted packed value-type member store"
                );
            }
            let field_ty = self.llvm_basic_type_of(span, field_cg)?;
            let natural = self.target_data.get_abi_alignment(&field_ty);
            Some(std::cmp::min(natural, pack_n))
        } else {
            None
        };
        Ok(MirMemberPlace {
            ptr,
            field_cg,
            writable: matches!(receiver, crate::mir::Operand::Local(_)),
            packed_alignment,
        })
    }

    pub(in crate::llvm::codegen) fn codegen_mir_operand(
        &mut self,
        span: crate::span::Span,
        operand: &crate::mir::Operand,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_mir_operand_expected(span, operand, slots, None)
    }

    pub(in crate::llvm::codegen) fn codegen_mir_operand_expected(
        &mut self,
        span: crate::span::Span,
        operand: &crate::mir::Operand,
        slots: &[MirLocalSlot<'ctx>],
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match operand {
            crate::mir::Operand::Local(local) => {
                let slot = self.mir_local_slot(span, slots, *local)?;
                self.load_mir_local(span, slot)
            }
            crate::mir::Operand::Const(value) => self.codegen_mir_const(span, value, expected),
        }
    }

    pub(in crate::llvm::codegen) fn codegen_mir_sysroot_gc_handle_new(
        &mut self,
        span: crate::span::Span,
        args: &[crate::mir::CallArg],
        slots: &[MirLocalSlot<'ctx>],
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let arg = self.expect_mir_positional_intrinsic_arg(args, 1, 0, "MIR GC.handleNew lowering");
        let Some(CgTy::Struct(handle_ty)) = expected else {
            self.panic_verified_intrinsic_contract(
                "MIR GC.handleNew lowering",
                "missing expected GcHandle result type",
            );
        };
        let (field_idx, field_cg_ty) =
            self.lookup_struct_field(handle_ty, "scoop.core.GcHandle.raw", span)?;
        let CgTy::Int(field_int_ty) = field_cg_ty else {
            self.panic_verified_intrinsic_contract(
                "MIR GC.handleNew lowering",
                "GcHandle.raw field is not an integer",
            );
        };

        let obj_v =
            self.codegen_mir_operand_expected(arg.span, &arg.value, slots, Some(CgTy::Ref))?;
        let obj_ref = self.coerce_value(arg.span, obj_v, CgTy::Ref)?;
        let obj_ptr = self.expect_cg_pointer(obj_ref, "MIR GC.handleNew argument");

        let rt_handle_new = self.declare_runtime_gc_handle_new();
        let call =
            self.builder
                .build_call(rt_handle_new, &[obj_ptr.into()], "mir_gc_handle_new")?;
        let raw = self.expect_basic_value(call, "MIR GC.handleNew runtime return");
        let handle_i64 = self.expect_int_value(raw, "MIR GC.handleNew runtime return");
        let ok_cond = self.builder.build_int_compare(
            IntPredicate::NE,
            handle_i64,
            self.context.i64_type().const_zero(),
            "mir_gc_handle_new_ok",
        )?;
        let func = self.expect_current_function("MIR GC.handleNew branch blocks");
        let ok_bb = self
            .context
            .append_basic_block(func, "mir_gc_handle_new_ok_bb");
        let err_bb = self
            .context
            .append_basic_block(func, "mir_gc_handle_new_err_bb");
        let cont_bb = self
            .context
            .append_basic_block(func, "mir_gc_handle_new_cont_bb");
        self.builder
            .build_conditional_branch(ok_cond, ok_bb, err_bb)?;
        self.builder.position_at_end(err_bb);
        self.emit_exit_with_code(span, 3)?;
        self.builder.position_at_end(ok_bb);
        let handle_word = self.cast_int(
            handle_i64,
            IntTy {
                bits: 64,
                signed: false,
            },
            field_int_ty,
        )?;
        let llvm_struct_ty = self.llvm_struct_type(span, handle_ty)?;
        let mut agg: AggregateValueEnum<'ctx> = llvm_struct_ty.get_undef().into();
        agg = self.builder.build_insert_value(
            agg,
            handle_word.as_basic_value_enum(),
            field_idx,
            "mir_gc_handle_raw",
        )?;
        self.builder.build_unconditional_branch(cont_bb)?;
        self.builder.position_at_end(cont_bb);
        Ok(CgValue {
            ty: CgTy::Struct(handle_ty),
            value: Some(agg.as_basic_value_enum()),
        })
    }

    pub(in crate::llvm::codegen) fn codegen_mir_sysroot_gc_handle_get(
        &mut self,
        span: crate::span::Span,
        args: &[crate::mir::CallArg],
        slots: &[MirLocalSlot<'ctx>],
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let arg = self.expect_mir_positional_intrinsic_arg(args, 1, 0, "MIR GC.handleGet lowering");
        if expected.is_some_and(|ty| ty != CgTy::Ref) {
            self.panic_verified_intrinsic_contract(
                "MIR GC.handleGet lowering",
                "target type is not Ref",
            );
        }

        let handle_v = self.codegen_mir_operand(arg.span, &arg.value, slots)?;
        let CgTy::Struct(handle_ty) = handle_v.ty else {
            self.panic_verified_intrinsic_contract(
                "MIR GC.handleGet lowering",
                "argument is not a GcHandle struct",
            );
        };
        let raw = self.expect_cg_value(handle_v, "MIR GC.handleGet argument");
        let struct_v = self.expect_struct_value(raw, "MIR GC.handleGet argument");
        let (field_idx, field_cg_ty) =
            self.lookup_struct_field(handle_ty, "scoop.core.GcHandle.raw", arg.span)?;
        let extracted =
            self.builder
                .build_extract_value(struct_v, field_idx, "mir_gc_handle_raw")?;
        let field_v = self.cg_value_from_loaded(arg.span, field_cg_ty, extracted)?;
        let CgTy::Int(field_int_ty) = field_cg_ty else {
            self.panic_verified_intrinsic_contract(
                "MIR GC.handleGet lowering",
                "GcHandle.raw field is not an integer",
            );
        };
        let field_raw = self.expect_cg_value(field_v, "MIR GC.handleGet raw handle field");
        let handle_word = self.expect_int_value(field_raw, "MIR GC.handleGet raw handle field");
        let handle_i64 = self.cast_int(
            handle_word,
            field_int_ty,
            IntTy {
                bits: 64,
                signed: false,
            },
        )?;
        let rt_handle_get = self.declare_runtime_gc_handle_get();
        let call =
            self.builder
                .build_call(rt_handle_get, &[handle_i64.into()], "mir_gc_handle_get")?;
        let raw = self.expect_basic_value(call, "MIR GC.handleGet runtime return");
        let obj_ptr = self.expect_pointer_value(raw, "MIR GC.handleGet runtime return");

        let obj_is_null = self
            .builder
            .build_is_null(obj_ptr, "mir_gc_handle_get_is_null")?;
        let ok_cond = self
            .builder
            .build_not(obj_is_null, "mir_gc_handle_get_ok")?;
        let func = self.expect_current_function("MIR GC.handleGet branch blocks");
        let ok_bb = self
            .context
            .append_basic_block(func, "mir_gc_handle_get_ok_bb");
        let err_bb = self
            .context
            .append_basic_block(func, "mir_gc_handle_get_err_bb");
        let cont_bb = self
            .context
            .append_basic_block(func, "mir_gc_handle_get_cont_bb");
        self.builder
            .build_conditional_branch(ok_cond, ok_bb, err_bb)?;
        self.builder.position_at_end(err_bb);
        self.emit_exit_with_code(span, 3)?;
        self.builder.position_at_end(ok_bb);
        self.builder.build_unconditional_branch(cont_bb)?;
        self.builder.position_at_end(cont_bb);

        Ok(CgValue {
            ty: CgTy::Ref,
            value: Some(obj_ptr.as_basic_value_enum()),
        })
    }

    pub(in crate::llvm::codegen) fn codegen_mir_sysroot_gc_handle_drop(
        &mut self,
        span: crate::span::Span,
        args: &[crate::mir::CallArg],
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let arg =
            self.expect_mir_positional_intrinsic_arg(args, 1, 0, "MIR GC.handleDrop lowering");
        let handle_v = self.codegen_mir_operand(arg.span, &arg.value, slots)?;
        let CgTy::Struct(handle_ty) = handle_v.ty else {
            self.panic_verified_intrinsic_contract(
                "MIR GC.handleDrop lowering",
                "argument is not a GcHandle struct",
            );
        };
        let raw = self.expect_cg_value(handle_v, "MIR GC.handleDrop argument");
        let struct_v = self.expect_struct_value(raw, "MIR GC.handleDrop argument");
        let (field_idx, field_cg_ty) =
            self.lookup_struct_field(handle_ty, "scoop.core.GcHandle.raw", arg.span)?;
        let extracted =
            self.builder
                .build_extract_value(struct_v, field_idx, "mir_gc_handle_raw")?;
        let field_v = self.cg_value_from_loaded(arg.span, field_cg_ty, extracted)?;
        let CgTy::Int(field_int_ty) = field_cg_ty else {
            self.panic_verified_intrinsic_contract(
                "MIR GC.handleDrop lowering",
                "GcHandle.raw field is not an integer",
            );
        };
        let field_raw = self.expect_cg_value(field_v, "MIR GC.handleDrop raw handle field");
        let handle_word = self.expect_int_value(field_raw, "MIR GC.handleDrop raw handle field");
        let handle_i64 = self.cast_int(
            handle_word,
            field_int_ty,
            IntTy {
                bits: 64,
                signed: false,
            },
        )?;
        let rt_handle_drop = self.declare_runtime_gc_handle_drop();
        let call =
            self.builder
                .build_call(rt_handle_drop, &[handle_i64.into()], "mir_gc_handle_drop")?;
        let raw = self.expect_basic_value(call, "MIR GC.handleDrop runtime return");
        let ok_i32 = self.expect_int_value(raw, "MIR GC.handleDrop runtime return");
        let ok_cond = self.builder.build_int_compare(
            IntPredicate::NE,
            ok_i32,
            self.context.i32_type().const_zero(),
            "mir_gc_handle_drop_ok",
        )?;
        let func = self.expect_current_function("MIR GC.handleDrop branch blocks");
        let ok_bb = self
            .context
            .append_basic_block(func, "mir_gc_handle_drop_ok_bb");
        let err_bb = self
            .context
            .append_basic_block(func, "mir_gc_handle_drop_err_bb");
        let cont_bb = self
            .context
            .append_basic_block(func, "mir_gc_handle_drop_cont_bb");
        self.builder
            .build_conditional_branch(ok_cond, ok_bb, err_bb)?;
        self.builder.position_at_end(err_bb);
        self.emit_exit_with_code(span, 3)?;
        self.builder.position_at_end(ok_bb);
        self.builder.build_unconditional_branch(cont_bb)?;
        self.builder.position_at_end(cont_bb);
        Ok(CgValue::unit())
    }
}
