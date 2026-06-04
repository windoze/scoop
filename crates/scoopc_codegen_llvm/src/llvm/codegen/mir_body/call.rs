//! MIR call lowering: direct, class-ctor, closure, fun-value, funptr-value, plain-dynamic.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_lir_call(
        &mut self,
        span: crate::span::Span,
        site_id: mir_source::SiteId,
        kind: &LirCallKind,
        args: &[LirCallArg],
        transport: &crate::effect_lowered::LirCallTransportMetadata,
        body: &LirExecutableBody,
        source_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        abi: Option<&ProgramAbiQuery<'ctx>>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match kind {
            LirCallKind::Direct {
                callee,
                generic_type_args,
                ..
            } => {
                let program = self
                    .published_late_lowered_program()
                    .unwrap_or_else(|| panic!("codegen_lir_call: missing published LIR program"));
                let callee_fqn = match callee {
                    scoopc_lir_facts::LirCallableRef::Local(id) => program
                        .callable_by_id(*id)
                        .unwrap_or_else(|| {
                            panic!("codegen_lir_call: LIR verifier accepted unknown callee id")
                        })
                        .root_fqn()
                        .to_string(),
                    scoopc_lir_facts::LirCallableRef::ExternalHash(_) => program
                        .root_for_callable_ref(*callee)
                        .unwrap_or_else(|| {
                            panic!(
                                "codegen_lir_call: LIR verifier accepted unresolved external callee"
                            )
                        })
                        .to_string(),
                };
                self.codegen_lir_direct_call_with_type_args(
                    span,
                    site_id,
                    &callee_fqn,
                    generic_type_args,
                    args,
                    body,
                    source_types,
                    transport,
                    slots,
                    abi,
                )
            }
            LirCallKind::Closure { callee, fn_ptr } => {
                let fun_ty = self
                    .lir_operand_function_type(body, source_types, callee)
                    .unwrap_or_else(|| {
                        panic!("codegen_lir_call: LIR call ABI verifier accepted non-function closure callee")
                    });
                self.codegen_lir_closure_call(span, callee, *fn_ptr, args, &fun_ty, slots)
            }
            LirCallKind::FunValue { callee } => {
                if let Some(callee_key) = self.lir_fun_value_callee_key(body, callee)
                    && matches!(
                        callee_key.as_str(),
                        "scoop.core.byteLength" | "scoop.core.getByte"
                    )
                {
                    let target_cg = self
                        .cg_ty_of_mir_type(source_types, transport.result.source_ty)
                        .or_else(|| {
                            self.equivalent_codegen_type_id(source_types, transport.result.source_ty)
                                .and_then(|ty| self.try_cg_ty_of_type_id(ty))
                        })
                        .unwrap_or_else(|| {
                            panic!("codegen_lir_call: LIR verifier accepted unsupported intrinsic return type")
                        });
                    return match callee_key.as_str() {
                        "scoop.core.byteLength" => self
                            .codegen_lir_core_string_byte_length_call(span, args, target_cg, slots),
                        "scoop.core.getByte" => {
                            self.codegen_lir_core_string_get_byte_call(span, args, target_cg, slots)
                        }
                        _ => unreachable!(),
                    };
                }
                let fun_ty = self
                    .lir_operand_function_type(body, source_types, callee)
                    .unwrap_or_else(|| {
                        panic!("codegen_lir_call: LIR call ABI verifier accepted non-function function-value callee")
                    });
                self.codegen_lir_fun_value_call(span, callee, args, &fun_ty, slots)
            }
            LirCallKind::FunPtr { callee } => {
                let fun_ty = self
                    .lir_operand_funptr_function_type(body, source_types, callee)
                    .unwrap_or_else(|| {
                        panic!("codegen_lir_call: LIR verifier accepted non-FunPtr callee type")
                    });
                self.codegen_lir_funptr_value_call(
                    span,
                    callee,
                    args,
                    &fun_ty,
                    (source_types, slots),
                )
            }
            LirCallKind::Virtual { receiver, .. } | LirCallKind::Interface { receiver, .. } => {
                if let LirCallKind::Interface { dispatch, .. } = kind
                    && dispatch.owner.as_str() == "scoop.core.ToString"
                    && dispatch.member_name == "toString"
                    && let Some(impl_fqn) = self.lir_builtin_to_string_impl_fqn_for_operand(
                        body,
                        source_types,
                        receiver,
                    )
                {
                    let mut direct_args = Vec::with_capacity(args.len() + 1);
                    direct_args.push(LirCallArg {
                        span,
                        name: None,
                        value: receiver.clone(),
                    });
                    direct_args.extend(args.iter().cloned());
                    return self.codegen_lir_direct_call_with_type_args(
                        span,
                        site_id,
                        impl_fqn,
                        &[],
                        &direct_args,
                        body,
                        source_types,
                        transport,
                        slots,
                        abi,
                    );
                }
                self.codegen_lir_plain_dynamic_dispatch_call(
                    span,
                    site_id,
                    receiver,
                    args,
                    body,
                    source_types,
                    slots,
                    matches!(kind, LirCallKind::Interface { .. }),
                )
            }
            LirCallKind::Resume { .. } => {
                panic!("codegen_lir_call: resume call reached plain callable lowering at {span:?}")
            }
        }
    }

    fn codegen_lir_atomic_intrinsic(
        &mut self,
        span: crate::span::Span,
        fqn: &str,
        args: &[LirCallArg],
        body: &LirExecutableBody,
        source_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        match fqn {
            "scoop.unsafe.__atomicRefLoad" => {
                if args.len() != 1 || args[0].name.is_some() {
                    self.panic_verified_intrinsic_contract(
                        "lir_atomic_ref_load",
                        "argument binding drift",
                    );
                }
                let place = self.lir_atomic_ref_place(
                    args[0].span,
                    &args[0].value,
                    body,
                    source_types,
                    slots,
                    false,
                )?;
                let llvm_ty = self.llvm_basic_type_of(args[0].span, place.field_cg)?;
                let loaded = self
                    .builder
                    .build_load(llvm_ty, place.ptr, "atomic_ref_load")?;
                let inst = loaded.as_instruction_value().unwrap_or_else(|| {
                    self.panic_verified_intrinsic_contract(
                        "lir_atomic_ref_load",
                        "load instruction missing",
                    )
                });
                inst.set_atomic_ordering(inkwell::AtomicOrdering::SequentiallyConsistent)
                    .unwrap_or_else(|_| {
                        self.panic_verified_intrinsic_contract(
                            "lir_atomic_ref_load",
                            "failed to set atomic ordering",
                        )
                    });
                Ok(Some(CgValue {
                    ty: place.field_cg,
                    value: Some(loaded),
                }))
            }
            "scoop.unsafe.__atomicRefStore" => {
                if args.len() != 2 || args.iter().any(|arg| arg.name.is_some()) {
                    self.panic_verified_intrinsic_contract(
                        "lir_atomic_ref_store",
                        "argument binding drift",
                    );
                }
                let place = self.lir_atomic_ref_place(
                    args[0].span,
                    &args[0].value,
                    body,
                    source_types,
                    slots,
                    true,
                )?;
                let raw = self.codegen_lir_atomic_ref_operand(
                    args[1].span,
                    &args[1].value,
                    place.field_cg,
                    slots,
                )?;
                let inst = self.builder.build_store(place.ptr, raw)?;
                inst.set_atomic_ordering(inkwell::AtomicOrdering::SequentiallyConsistent)
                    .unwrap_or_else(|_| {
                        self.panic_verified_intrinsic_contract(
                            "lir_atomic_ref_store",
                            "failed to set atomic ordering",
                        )
                    });
                self.promote_gc_pointer_with_write_barrier(args[1].span, raw)?;
                Ok(Some(CgValue::unit()))
            }
            "scoop.unsafe.__atomicRefCompareExchange" => {
                if args.len() != 3 || args.iter().any(|arg| arg.name.is_some()) {
                    self.panic_verified_intrinsic_contract(
                        "lir_atomic_ref_cmpxchg",
                        "argument binding drift",
                    );
                }
                let place = self.lir_atomic_ref_place(
                    args[0].span,
                    &args[0].value,
                    body,
                    source_types,
                    slots,
                    true,
                )?;
                let expected = self.codegen_lir_atomic_ref_operand(
                    args[1].span,
                    &args[1].value,
                    place.field_cg,
                    slots,
                )?;
                let desired = self.codegen_lir_atomic_ref_operand(
                    args[2].span,
                    &args[2].value,
                    place.field_cg,
                    slots,
                )?;
                let cx = self.builder.build_cmpxchg(
                    place.ptr,
                    expected,
                    desired,
                    inkwell::AtomicOrdering::SequentiallyConsistent,
                    inkwell::AtomicOrdering::SequentiallyConsistent,
                )?;
                let success = self.builder.build_extract_value(cx, 1, "cmpxchg_success")?;
                let ok = self.expect_int_value(success, "lir_atomic_ref_cmpxchg success");
                self.codegen_lir_atomic_ref_cas_barrier(args[2].span, ok, desired)?;
                Ok(Some(CgValue::bool(ok)))
            }
            "scoop.unsafe.__atomicIntLoad" => {
                if args.len() != 1 || args[0].name.is_some() {
                    self.panic_verified_intrinsic_contract(
                        "lir_atomic_int_load",
                        "argument binding drift",
                    );
                }
                let (ptr, int_ty) = self.lir_atomic_int_place(
                    args[0].span,
                    &args[0].value,
                    body,
                    source_types,
                    slots,
                    false,
                )?;
                let loaded =
                    self.builder
                        .build_load(self.int_type(int_ty), ptr, "atomic_int_load")?;
                let inst = loaded.as_instruction_value().unwrap_or_else(|| {
                    self.panic_verified_intrinsic_contract(
                        "lir_atomic_int_load",
                        "load instruction missing",
                    )
                });
                inst.set_atomic_ordering(inkwell::AtomicOrdering::SequentiallyConsistent)
                    .unwrap_or_else(|_| {
                        self.panic_verified_intrinsic_contract(
                            "lir_atomic_int_load",
                            "failed to set atomic ordering",
                        )
                    });
                let raw = self.expect_int_value(loaded, "lir_atomic_int_load return");
                Ok(Some(CgValue::int(raw, int_ty)))
            }
            "scoop.unsafe.__atomicIntStore" => {
                if args.len() != 2 || args.iter().any(|arg| arg.name.is_some()) {
                    self.panic_verified_intrinsic_contract(
                        "lir_atomic_int_store",
                        "argument binding drift",
                    );
                }
                let (ptr, int_ty) = self.lir_atomic_int_place(
                    args[0].span,
                    &args[0].value,
                    body,
                    source_types,
                    slots,
                    true,
                )?;
                let raw = self.codegen_lir_atomic_int_operand(
                    args[1].span,
                    &args[1].value,
                    int_ty,
                    slots,
                )?;
                let inst = self.builder.build_store(ptr, raw)?;
                inst.set_atomic_ordering(inkwell::AtomicOrdering::SequentiallyConsistent)
                    .unwrap_or_else(|_| {
                        self.panic_verified_intrinsic_contract(
                            "lir_atomic_int_store",
                            "failed to set atomic ordering",
                        )
                    });
                Ok(Some(CgValue::unit()))
            }
            "scoop.unsafe.__atomicIntCompareExchange" => {
                if args.len() != 3 || args.iter().any(|arg| arg.name.is_some()) {
                    self.panic_verified_intrinsic_contract(
                        "lir_atomic_int_cmpxchg",
                        "argument binding drift",
                    );
                }
                let (ptr, int_ty) = self.lir_atomic_int_place(
                    args[0].span,
                    &args[0].value,
                    body,
                    source_types,
                    slots,
                    true,
                )?;
                let expected = self.codegen_lir_atomic_int_operand(
                    args[1].span,
                    &args[1].value,
                    int_ty,
                    slots,
                )?;
                let desired = self.codegen_lir_atomic_int_operand(
                    args[2].span,
                    &args[2].value,
                    int_ty,
                    slots,
                )?;
                let cx = self.builder.build_cmpxchg(
                    ptr,
                    expected,
                    desired,
                    inkwell::AtomicOrdering::SequentiallyConsistent,
                    inkwell::AtomicOrdering::SequentiallyConsistent,
                )?;
                let success = self.builder.build_extract_value(cx, 1, "cmpxchg_success")?;
                let ok = self.expect_int_value(success, "lir_atomic_int_cmpxchg success");
                Ok(Some(CgValue::bool(ok)))
            }
            _ => {
                let _ = span;
                Ok(None)
            }
        }
    }

    fn lir_atomic_ref_place(
        &mut self,
        span: crate::span::Span,
        operand: &LirOperand,
        body: &LirExecutableBody,
        source_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        require_writable: bool,
    ) -> Result<MirMemberPlace<'ctx>, LlvmEmitError> {
        let LirOperand::Local(local) = operand else {
            self.panic_verified_intrinsic_contract("lir_atomic_ref_place", "target is not local");
        };
        let place = self.lir_atomic_place_for_local(
            span,
            *local,
            body,
            source_types,
            slots,
            require_writable,
        )?;
        if matches!(place.field_cg, CgTy::Ref | CgTy::String) {
            return Ok(place);
        }
        self.panic_verified_intrinsic_contract("lir_atomic_ref_place", "target is not ref storage")
    }

    fn lir_atomic_int_place(
        &mut self,
        span: crate::span::Span,
        operand: &LirOperand,
        body: &LirExecutableBody,
        source_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        require_writable: bool,
    ) -> Result<(inkwell::values::PointerValue<'ctx>, IntTy), LlvmEmitError> {
        let LirOperand::Local(local) = operand else {
            self.panic_verified_intrinsic_contract("lir_atomic_int_place", "target is not local");
        };
        let place = self.lir_atomic_place_for_local(
            span,
            *local,
            body,
            source_types,
            slots,
            require_writable,
        )?;
        let CgTy::Int(int_ty) = place.field_cg else {
            self.panic_verified_intrinsic_contract(
                "lir_atomic_int_place",
                "target is not int storage",
            );
        };
        Ok((place.ptr, int_ty))
    }

    fn lir_atomic_place_for_local(
        &mut self,
        span: crate::span::Span,
        local: crate::effect_lowered::mir_source::LocalId,
        body: &LirExecutableBody,
        source_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        require_writable: bool,
    ) -> Result<MirMemberPlace<'ctx>, LlvmEmitError> {
        if let Some(place) = self.lir_atomic_member_place_for_local(
            local,
            span,
            body,
            source_types,
            slots,
            require_writable,
        )? {
            return Ok(place);
        }
        if let Some(place) =
            self.lir_atomic_top_level_place_for_local(local, body, source_types, require_writable)?
        {
            return Ok(place);
        }
        let slot = self.mir_local_slot(span, slots, local)?;
        Ok(MirMemberPlace {
            ptr: slot.ptr,
            field_cg: slot.cg_ty,
            writable: true,
            packed_alignment: None,
        })
    }

    fn lir_atomic_member_place_for_local(
        &mut self,
        local: crate::effect_lowered::mir_source::LocalId,
        _span: crate::span::Span,
        body: &LirExecutableBody,
        source_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        require_writable: bool,
    ) -> Result<Option<MirMemberPlace<'ctx>>, LlvmEmitError> {
        let mut found = None;
        for state in body.states().states() {
            for stmt in state.body().statements() {
                let LirStatementKind::Assign {
                    target,
                    value:
                        LirRvalue::MemberAccess {
                            receiver, member, ..
                        },
                } = &stmt.kind
                else {
                    continue;
                };
                if *target == local {
                    found = Some((stmt.span, receiver.clone(), member.clone()));
                    break;
                }
            }
            if found.is_some() {
                break;
            }
        }
        if let Some((stmt_span, receiver, member)) = found {
            return self
                .lir_atomic_member_place(
                    stmt_span,
                    &receiver,
                    &member,
                    LirBodyCodegenCtx {
                        body,
                        source_types,
                        slots,
                    },
                    require_writable,
                )
                .map(Some);
        }
        Ok(None)
    }

    fn lir_atomic_member_place(
        &mut self,
        span: crate::span::Span,
        receiver: &LirOperand,
        member: &LirMemberAccessMetadata,
        lir_ctx: LirBodyCodegenCtx<'_, 'ctx>,
        require_writable: bool,
    ) -> Result<MirMemberPlace<'ctx>, LlvmEmitError> {
        let field_key = lir_member_value_key_for_codegen(span, member)?;
        let receiver_type_id = self.lir_member_receiver_codegen_type_id(
            span,
            lir_ctx.body,
            lir_ctx.source_types,
            receiver,
            member,
        )?;
        let receiver_cg = self.cg_ty_of_type_id(receiver_type_id, "LIR atomic member receiver");
        if let Some((class, field_idx, field_cg)) =
            self.lookup_class_field_by_fqn(field_key, span, Some(receiver_type_id))?
            && receiver_cg == CgTy::Ref
        {
            let field = class.fields.get(field_idx as usize).unwrap_or_else(|| {
                self.panic_verified_intrinsic_contract(
                    "lir_atomic_member_place",
                    "class field index drift",
                )
            });
            if require_writable && !field.mutable {
                self.panic_verified_intrinsic_contract(
                    "lir_atomic_member_place",
                    "class field target is not writable",
                );
            }
            let receiver_value =
                self.codegen_lir_operand_expected(span, receiver, lir_ctx.slots, Some(CgTy::Ref))?;
            let receiver_value = self.coerce_value(span, receiver_value, CgTy::Ref)?;
            let obj_ptr = self.expect_cg_pointer(receiver_value, "LIR atomic class receiver");
            let ptr = self.codegen_class_field_ptr(span, &class, obj_ptr, field_idx)?;
            return Ok(MirMemberPlace {
                ptr,
                field_cg,
                writable: field.mutable,
                packed_alignment: None,
            });
        }

        let CgTy::Struct(struct_ty) = receiver_cg else {
            self.panic_verified_intrinsic_contract(
                "lir_atomic_member_place",
                "member receiver is not class ref or struct",
            );
        };
        let (field_idx, field_cg) = self.lookup_struct_field(struct_ty, field_key, span)?;
        let base_ptr = self.lir_atomic_struct_receiver_ptr(
            span,
            receiver,
            struct_ty,
            lir_ctx,
            require_writable,
        )?;
        let llvm_struct_ty = self.llvm_struct_type(span, struct_ty)?;
        let ptr =
            self.builder
                .build_struct_gep(llvm_struct_ty, base_ptr, field_idx, "lir_member_gep")?;
        Ok(MirMemberPlace {
            ptr,
            field_cg,
            writable: true,
            packed_alignment: None,
        })
    }

    fn lir_atomic_struct_receiver_ptr(
        &mut self,
        span: crate::span::Span,
        receiver: &LirOperand,
        struct_ty: MonoTypeId,
        lir_ctx: LirBodyCodegenCtx<'_, 'ctx>,
        require_writable: bool,
    ) -> Result<inkwell::values::PointerValue<'ctx>, LlvmEmitError> {
        let LirOperand::Local(local) = receiver else {
            self.panic_verified_intrinsic_contract(
                "lir_atomic_struct_receiver_ptr",
                "struct receiver operand is not local",
            );
        };
        if let Some(place) = self.lir_atomic_member_place_for_local(
            *local,
            span,
            lir_ctx.body,
            lir_ctx.source_types,
            lir_ctx.slots,
            require_writable,
        )? {
            if place.field_cg != CgTy::Struct(struct_ty) {
                self.panic_verified_intrinsic_contract(
                    "lir_atomic_struct_receiver_ptr",
                    "nested struct receiver type drift",
                );
            }
            return Ok(place.ptr);
        }
        let slot = self.mir_local_slot(span, lir_ctx.slots, *local)?;
        if slot.cg_ty != CgTy::Struct(struct_ty) {
            self.panic_verified_intrinsic_contract(
                "lir_atomic_struct_receiver_ptr",
                "struct receiver slot type drift",
            );
        }
        Ok(slot.ptr)
    }

    fn lir_atomic_top_level_place_for_local(
        &mut self,
        local: crate::effect_lowered::mir_source::LocalId,
        body: &LirExecutableBody,
        source_types: &TypeStore,
        require_writable: bool,
    ) -> Result<Option<MirMemberPlace<'ctx>>, LlvmEmitError> {
        let Some(root_key) = self.lir_local_top_level_ref_key(local, body) else {
            return Ok(None);
        };
        let key = root_key.as_str();
        if self.lir_global_root_has_kind(key, LirGlobalRootKind::ExternGlobal) {
            let root = self
                .expect_lir_global_root_kind(
                    key,
                    LirGlobalRootKind::ExternGlobal,
                    "lir_atomic_top_level_place_for_local",
                )
                .clone();
            let extern_global = root.extern_global.as_ref().unwrap_or_else(|| {
                panic!("lir_atomic_top_level_place_for_local: extern root missing contract")
            });
            if require_writable && !extern_global.mutable {
                self.panic_verified_intrinsic_contract(
                    "lir_atomic_top_level_place_for_local",
                    "extern global target is not writable",
                );
            }
            let cg_ty = self.cg_ty_of_type_id(
                self.lir_global_root_ty(&root, "LIR atomic extern global"),
                "LIR atomic extern global",
            );
            let gv = self.declare_lir_extern_global(&root)?;
            return Ok(Some(MirMemberPlace {
                ptr: gv.as_pointer_value(),
                field_cg: cg_ty,
                writable: extern_global.mutable,
                packed_alignment: None,
            }));
        }
        if !self.lir_global_root_has_kind(key, LirGlobalRootKind::TopLevelMutableVar) {
            return Ok(None);
        }
        let root = self
            .expect_lir_global_root_kind(
                key,
                LirGlobalRootKind::TopLevelMutableVar,
                "lir_atomic_top_level_place_for_local",
            )
            .clone();
        let cg_ty = self
            .cg_ty_of_mir_type(
                source_types,
                self.lir_global_root_ty(&root, "LIR atomic global"),
            )
            .unwrap_or_else(|| {
                self.cg_ty_of_type_id(
                    self.lir_global_root_ty(&root, "LIR atomic global"),
                    "LIR atomic global",
                )
            });
        let gv = self.declare_lir_top_level_var_global(&root)?;
        Ok(Some(MirMemberPlace {
            ptr: gv.as_pointer_value(),
            field_cg: cg_ty,
            writable: true,
            packed_alignment: None,
        }))
    }

    fn lir_local_top_level_ref_key(
        &self,
        local: crate::effect_lowered::mir_source::LocalId,
        body: &LirExecutableBody,
    ) -> Option<scoopc_lir_facts::LirGlobalRootKey> {
        for state in body.states().states() {
            for stmt in state.body().statements() {
                let LirStatementKind::Assign { target, value } = &stmt.kind else {
                    continue;
                };
                if *target != local {
                    continue;
                }
                let LirRvalue::TopLevelRef(crate::effect_lowered::LirTopLevelRef {
                    target: LirTopLevelRefTarget::Global(root),
                    ..
                }) = value
                else {
                    return None;
                };
                return Some(root.clone());
            }
        }
        None
    }

    fn codegen_lir_atomic_ref_operand(
        &mut self,
        span: crate::span::Span,
        operand: &LirOperand,
        storage_ty: CgTy,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<inkwell::values::PointerValue<'ctx>, LlvmEmitError> {
        let value = self.codegen_lir_operand_expected(span, operand, slots, Some(storage_ty))?;
        let value = self.coerce_value(span, value, storage_ty)?;
        Ok(self.expect_cg_pointer(value, "lir_atomic_ref_operand"))
    }

    fn codegen_lir_atomic_int_operand(
        &mut self,
        span: crate::span::Span,
        operand: &LirOperand,
        int_ty: IntTy,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<inkwell::values::IntValue<'ctx>, LlvmEmitError> {
        let value =
            self.codegen_lir_operand_expected(span, operand, slots, Some(CgTy::Int(int_ty)))?;
        let value = self.coerce_value(span, value, CgTy::Int(int_ty))?;
        let (raw, from) = self.expect_cg_int(value, "lir_atomic_int_operand");
        self.cast_int(raw, from, int_ty)
    }

    fn codegen_lir_atomic_ref_cas_barrier(
        &mut self,
        span: crate::span::Span,
        success: inkwell::values::IntValue<'ctx>,
        desired: inkwell::values::PointerValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let function = self.expect_current_function("lir_atomic_ref_cas_barrier");
        let barrier_bb = self
            .context
            .append_basic_block(function, "atomic_ref_cas_barrier");
        let cont_bb = self
            .context
            .append_basic_block(function, "atomic_ref_cas_cont");
        self.builder
            .build_conditional_branch(success, barrier_bb, cont_bb)?;
        self.builder.position_at_end(barrier_bb);
        self.promote_gc_pointer_with_write_barrier(span, desired)?;
        self.builder.build_unconditional_branch(cont_bb)?;
        self.builder.position_at_end(cont_bb);
        Ok(())
    }

    fn codegen_lir_gc_handle_intrinsic(
        &mut self,
        span: crate::span::Span,
        fqn: &str,
        args: &[LirCallArg],
        source_types: &TypeStore,
        transport: &crate::effect_lowered::LirCallTransportMetadata,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        match fqn {
            "scoop.core.GC.handleNew" => {
                let arg = self.expect_lir_positional_arg(args, 1, 0, "LIR GC.handleNew lowering");
                let Some(CgTy::Struct(handle_ty)) =
                    self.cg_ty_of_mir_type(source_types, transport.result.source_ty)
                else {
                    self.panic_verified_intrinsic_contract(
                        "LIR GC.handleNew lowering",
                        "missing expected GcHandle result type",
                    );
                };
                let (field_idx, field_cg_ty) =
                    self.lookup_struct_field(handle_ty, "scoop.core.GcHandle.raw", span)?;
                let CgTy::Int(field_int_ty) = field_cg_ty else {
                    self.panic_verified_intrinsic_contract(
                        "LIR GC.handleNew lowering",
                        "GcHandle.raw field is not an integer",
                    );
                };
                let obj_v = self.codegen_lir_operand_expected(
                    arg.span,
                    &arg.value,
                    slots,
                    Some(CgTy::Ref),
                )?;
                let obj_ref = self.coerce_value(arg.span, obj_v, CgTy::Ref)?;
                let obj_ptr = self.expect_cg_pointer(obj_ref, "LIR GC.handleNew argument");

                let rt_handle_new = self.declare_runtime_gc_handle_new();
                let call = self.builder.build_call(
                    rt_handle_new,
                    &[obj_ptr.into()],
                    "lir_gc_handle_new",
                )?;
                let raw = self.expect_basic_value(call, "LIR GC.handleNew runtime return");
                let handle_i64 = self.expect_int_value(raw, "LIR GC.handleNew runtime return");
                let ok_cond = self.builder.build_int_compare(
                    inkwell::IntPredicate::NE,
                    handle_i64,
                    self.context.i64_type().const_zero(),
                    "lir_gc_handle_new_ok",
                )?;
                let func = self.expect_current_function("LIR GC.handleNew branch blocks");
                let ok_bb = self
                    .context
                    .append_basic_block(func, "lir_gc_handle_new_ok_bb");
                let err_bb = self
                    .context
                    .append_basic_block(func, "lir_gc_handle_new_err_bb");
                let cont_bb = self
                    .context
                    .append_basic_block(func, "lir_gc_handle_new_cont_bb");
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
                let mut agg: inkwell::values::AggregateValueEnum<'ctx> =
                    llvm_struct_ty.get_undef().into();
                agg = self.builder.build_insert_value(
                    agg,
                    handle_word.as_basic_value_enum(),
                    field_idx,
                    "lir_gc_handle_raw",
                )?;
                self.builder.build_unconditional_branch(cont_bb)?;
                self.builder.position_at_end(cont_bb);
                Ok(Some(CgValue {
                    ty: CgTy::Struct(handle_ty),
                    value: Some(agg.as_basic_value_enum()),
                }))
            }
            "scoop.core.GC.handleGet" => {
                let arg = self.expect_lir_positional_arg(args, 1, 0, "LIR GC.handleGet lowering");
                let handle_v = self.codegen_lir_operand(arg.span, &arg.value, slots)?;
                let CgTy::Struct(handle_ty) = handle_v.ty else {
                    self.panic_verified_intrinsic_contract(
                        "LIR GC.handleGet lowering",
                        "argument is not a GcHandle struct",
                    );
                };
                let handle_i64 = self.extract_lir_gc_handle_word(arg.span, handle_v, handle_ty)?;
                let rt_handle_get = self.declare_runtime_gc_handle_get();
                let call = self.builder.build_call(
                    rt_handle_get,
                    &[handle_i64.into()],
                    "lir_gc_handle_get",
                )?;
                let raw = self.expect_basic_value(call, "LIR GC.handleGet runtime return");
                let obj_ptr = self.expect_pointer_value(raw, "LIR GC.handleGet runtime return");
                let obj_is_null = self
                    .builder
                    .build_is_null(obj_ptr, "lir_gc_handle_get_is_null")?;
                let ok_cond = self
                    .builder
                    .build_not(obj_is_null, "lir_gc_handle_get_ok")?;
                let func = self.expect_current_function("LIR GC.handleGet branch blocks");
                let ok_bb = self
                    .context
                    .append_basic_block(func, "lir_gc_handle_get_ok_bb");
                let err_bb = self
                    .context
                    .append_basic_block(func, "lir_gc_handle_get_err_bb");
                let cont_bb = self
                    .context
                    .append_basic_block(func, "lir_gc_handle_get_cont_bb");
                self.builder
                    .build_conditional_branch(ok_cond, ok_bb, err_bb)?;
                self.builder.position_at_end(err_bb);
                self.emit_exit_with_code(span, 3)?;
                self.builder.position_at_end(ok_bb);
                self.builder.build_unconditional_branch(cont_bb)?;
                self.builder.position_at_end(cont_bb);
                Ok(Some(CgValue {
                    ty: CgTy::Ref,
                    value: Some(obj_ptr.as_basic_value_enum()),
                }))
            }
            "scoop.core.GC.handleDrop" => {
                let arg = self.expect_lir_positional_arg(args, 1, 0, "LIR GC.handleDrop lowering");
                let handle_v = self.codegen_lir_operand(arg.span, &arg.value, slots)?;
                let CgTy::Struct(handle_ty) = handle_v.ty else {
                    self.panic_verified_intrinsic_contract(
                        "LIR GC.handleDrop lowering",
                        "argument is not a GcHandle struct",
                    );
                };
                let handle_i64 = self.extract_lir_gc_handle_word(arg.span, handle_v, handle_ty)?;
                let rt_handle_drop = self.declare_runtime_gc_handle_drop();
                let call = self.builder.build_call(
                    rt_handle_drop,
                    &[handle_i64.into()],
                    "lir_gc_handle_drop",
                )?;
                let raw = self.expect_basic_value(call, "LIR GC.handleDrop runtime return");
                let ok_i32 = self.expect_int_value(raw, "LIR GC.handleDrop runtime return");
                let ok_cond = self.builder.build_int_compare(
                    inkwell::IntPredicate::NE,
                    ok_i32,
                    self.context.i32_type().const_zero(),
                    "lir_gc_handle_drop_ok",
                )?;
                let func = self.expect_current_function("LIR GC.handleDrop branch blocks");
                let ok_bb = self
                    .context
                    .append_basic_block(func, "lir_gc_handle_drop_ok_bb");
                let err_bb = self
                    .context
                    .append_basic_block(func, "lir_gc_handle_drop_err_bb");
                let cont_bb = self
                    .context
                    .append_basic_block(func, "lir_gc_handle_drop_cont_bb");
                self.builder
                    .build_conditional_branch(ok_cond, ok_bb, err_bb)?;
                self.builder.position_at_end(err_bb);
                self.emit_exit_with_code(span, 3)?;
                self.builder.position_at_end(ok_bb);
                self.builder.build_unconditional_branch(cont_bb)?;
                self.builder.position_at_end(cont_bb);
                Ok(Some(CgValue::unit()))
            }
            "scoop.core.GC.pin" => Ok(Some(self.codegen_lir_gc_pin(
                span,
                args,
                source_types,
                transport,
                slots,
            )?)),
            "scoop.core.GC.unpin" => Ok(Some(self.codegen_lir_gc_unpin(span, args, slots)?)),
            _ => Ok(None),
        }
    }

    fn codegen_lir_gc_pin(
        &mut self,
        span: crate::span::Span,
        args: &[LirCallArg],
        source_types: &TypeStore,
        transport: &crate::effect_lowered::LirCallTransportMetadata,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let arg = self.expect_lir_positional_arg(args, 1, 0, "LIR GC.pin lowering");
        let target_cg = self
            .cg_ty_of_mir_type(source_types, transport.result.source_ty)
            .or_else(|| {
                self.equivalent_codegen_type_id(source_types, transport.result.source_ty)
                    .and_then(|ty| self.try_cg_ty_of_type_id(ty))
            })
            .unwrap_or_else(|| {
                self.panic_verified_intrinsic_contract(
                    "LIR GC.pin lowering",
                    "missing expected Pinned result type",
                )
            });
        let CgTy::Struct(pinned_ty) = target_cg else {
            self.panic_verified_intrinsic_contract(
                "LIR GC.pin lowering",
                "target is not a Pinned struct",
            );
        };
        let (field_idx, field_cg_ty) =
            self.lookup_struct_field(pinned_ty, "scoop.core.Pinned.value", span)?;
        let obj =
            self.codegen_lir_operand_expected(arg.span, &arg.value, slots, Some(field_cg_ty))?;
        let obj = self.coerce_value(arg.span, obj, field_cg_ty)?;
        let obj_ref = self.coerce_value(arg.span, obj, CgTy::Ref)?;
        let obj_ptr = self.expect_cg_pointer(obj_ref, "LIR GC.pin argument");

        let rt_pin = self.declare_runtime_gc_pin();
        let call = self
            .builder
            .build_call(rt_pin, &[obj_ptr.into()], "lir_gc_pin")?;
        let raw = self.expect_basic_value(call, "LIR GC.pin runtime return");
        let ok_i32 = self.expect_int_value(raw, "LIR GC.pin runtime return");
        let ok_cond = self.builder.build_int_compare(
            inkwell::IntPredicate::NE,
            ok_i32,
            self.context.i32_type().const_zero(),
            "lir_gc_pin_ok",
        )?;
        let func = self.expect_current_function("LIR GC.pin branch blocks");
        let ok_bb = self.context.append_basic_block(func, "lir_gc_pin_ok_bb");
        let err_bb = self.context.append_basic_block(func, "lir_gc_pin_err_bb");
        let cont_bb = self.context.append_basic_block(func, "lir_gc_pin_cont_bb");
        self.builder
            .build_conditional_branch(ok_cond, ok_bb, err_bb)?;
        self.builder.position_at_end(err_bb);
        self.emit_exit_with_code(span, 3)?;

        self.builder.position_at_end(ok_bb);
        let llvm_struct_ty = self.llvm_struct_type(span, pinned_ty)?;
        let mut agg: inkwell::values::AggregateValueEnum<'ctx> = llvm_struct_ty.get_undef().into();
        let raw_field: inkwell::values::BasicValueEnum<'ctx> = match field_cg_ty {
            CgTy::Unit => self.context.i8_type().const_int(0, false).into(),
            _ => self.expect_cg_value(obj, "LIR GC.pin Pinned.value field"),
        };
        agg = self
            .builder
            .build_insert_value(agg, raw_field, field_idx, "lir_pinned_value")?;
        self.builder.build_unconditional_branch(cont_bb)?;
        self.builder.position_at_end(cont_bb);
        Ok(CgValue {
            ty: target_cg,
            value: Some(agg.as_basic_value_enum()),
        })
    }

    fn codegen_lir_gc_unpin(
        &mut self,
        span: crate::span::Span,
        args: &[LirCallArg],
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let arg = self.expect_lir_positional_arg(args, 1, 0, "LIR GC.unpin lowering");
        let pinned = self.codegen_lir_operand(arg.span, &arg.value, slots)?;
        let CgTy::Struct(pinned_ty) = pinned.ty else {
            self.panic_verified_intrinsic_contract(
                "LIR GC.unpin lowering",
                "argument is not a Pinned struct",
            );
        };
        let raw = self.expect_cg_value(pinned, "LIR GC.unpin argument");
        let struct_v = self.expect_struct_value(raw, "LIR GC.unpin argument");
        let (field_idx, field_cg_ty) =
            self.lookup_struct_field(pinned_ty, "scoop.core.Pinned.value", arg.span)?;
        let extracted =
            self.builder
                .build_extract_value(struct_v, field_idx, "lir_pinned_value")?;
        let field = self.cg_value_from_loaded(arg.span, field_cg_ty, extracted)?;
        let field_ref = self.coerce_value(arg.span, field, CgTy::Ref)?;
        let obj_ptr = self.expect_cg_pointer(field_ref, "LIR GC.unpin Pinned.value field");

        let rt_unpin = self.declare_runtime_gc_unpin();
        let call = self
            .builder
            .build_call(rt_unpin, &[obj_ptr.into()], "lir_gc_unpin")?;
        let raw = self.expect_basic_value(call, "LIR GC.unpin runtime return");
        let ok_i32 = self.expect_int_value(raw, "LIR GC.unpin runtime return");
        let ok_cond = self.builder.build_int_compare(
            inkwell::IntPredicate::NE,
            ok_i32,
            self.context.i32_type().const_zero(),
            "lir_gc_unpin_ok",
        )?;
        let func = self.expect_current_function("LIR GC.unpin branch blocks");
        let ok_bb = self.context.append_basic_block(func, "lir_gc_unpin_ok_bb");
        let err_bb = self.context.append_basic_block(func, "lir_gc_unpin_err_bb");
        let cont_bb = self
            .context
            .append_basic_block(func, "lir_gc_unpin_cont_bb");
        self.builder
            .build_conditional_branch(ok_cond, ok_bb, err_bb)?;
        self.builder.position_at_end(err_bb);
        self.emit_exit_with_code(span, 3)?;
        self.builder.position_at_end(ok_bb);
        self.builder.build_unconditional_branch(cont_bb)?;
        self.builder.position_at_end(cont_bb);
        Ok(CgValue::unit())
    }

    fn expect_lir_positional_arg<'b>(
        &self,
        args: &'b [LirCallArg],
        expected_len: usize,
        index: usize,
        context: &'static str,
    ) -> &'b LirCallArg {
        if args.len() != expected_len || args.iter().any(|arg| arg.name.is_some()) {
            self.panic_verified_intrinsic_contract(context, "argument binding drift");
        }
        args.get(index)
            .unwrap_or_else(|| self.panic_verified_intrinsic_contract(context, "missing argument"))
    }

    fn extract_lir_gc_handle_word(
        &mut self,
        span: crate::span::Span,
        handle_v: CgValue<'ctx>,
        handle_ty: crate::ty::MonoTypeId,
    ) -> Result<inkwell::values::IntValue<'ctx>, LlvmEmitError> {
        let raw = self.expect_cg_value(handle_v, "LIR GC handle argument");
        let struct_v = self.expect_struct_value(raw, "LIR GC handle argument");
        let (field_idx, field_cg_ty) =
            self.lookup_struct_field(handle_ty, "scoop.core.GcHandle.raw", span)?;
        let extracted =
            self.builder
                .build_extract_value(struct_v, field_idx, "lir_gc_handle_raw")?;
        let field_v = self.cg_value_from_loaded(span, field_cg_ty, extracted)?;
        let CgTy::Int(field_int_ty) = field_cg_ty else {
            self.panic_verified_intrinsic_contract(
                "extract_lir_gc_handle_word",
                "GcHandle.raw field is not an integer",
            );
        };
        let field_raw = self.expect_cg_value(field_v, "LIR GC handle raw field");
        let handle_word = self.expect_int_value(field_raw, "LIR GC handle raw field");
        self.cast_int(
            handle_word,
            field_int_ty,
            IntTy {
                bits: 64,
                signed: false,
            },
        )
    }

    fn lir_builtin_to_string_impl_fqn_for_operand(
        &self,
        body: &LirExecutableBody,
        source_types: &TypeStore,
        operand: &LirOperand,
    ) -> Option<&'static str> {
        let ty = self.lir_operand_type_id(body, operand)?;
        match source_types.kind(ty) {
            TypeKind::Value(ValueTypeKind::Bool) => Some("scoop.core.Bool.toString"),
            TypeKind::Value(ValueTypeKind::Char) => Some("scoop.core.Char.toString"),
            TypeKind::Value(ValueTypeKind::Float64) => Some("scoop.core.Float64.toString"),
            TypeKind::Value(ValueTypeKind::Float32) => Some("scoop.core.Float32.toString"),
            TypeKind::Value(ValueTypeKind::Int) => Some("scoop.core.Int.toString"),
            TypeKind::Ref(RefTypeKind::String) => Some("scoop.core.String.toString"),
            TypeKind::Ref(RefTypeKind::Nominal(nominal)) if nominal.fqn == "scoop.core.String" => {
                Some("scoop.core.String.toString")
            }
            _ => None,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_mir_direct_call_with_policy(
        &mut self,
        span: crate::span::Span,
        site_id: Option<mir_source::SiteId>,
        fqn: &str,
        generic_type_args: &[TypeId],
        args: &[mir_source::CallArg],
        transport: &mir_source::CallTransportMetadata,
        body: &mir_source::Body,
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
        let exact_callee = source_site.and_then(|site| site.contract.exact_callee.clone());
        let concrete_fqn = if let Some(exact) = exact_callee.as_ref() {
            exact.root_fqn.clone()
        } else if !generic_type_args.is_empty() {
            return Err(frontend_error(format!(
                "generic direct call `{fqn}` lacks published exact callee binding"
            )));
        } else {
            fqn.to_string()
        };
        let concrete_fqn = concrete_fqn.as_str();
        let named_intrinsic_entry = source_site
            .and_then(|site| site.named_entry_name.as_deref())
            .map(str::to_string)
            .or_else(|| self.published_named_intrinsic_entry_name(concrete_fqn));
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
                "codegen_mir_direct_call_with_policy: MIR call ABI verifier accepted direct call arity mismatch for `{concrete_fqn}`: args={}, params={}",
                args.len(),
                param_tys.len()
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
            source_types,
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

        let llvm_name = exact_callee
            .as_ref()
            .map(|exact| exact.abi_symbol.as_str())
            .ok_or_else(|| {
                frontend_error(format!(
                    "direct call `{concrete_fqn}` lacks target-bound ABI symbol binding"
                ))
            })?;
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

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_lir_direct_call_with_type_args(
        &mut self,
        span: crate::span::Span,
        site_id: mir_source::SiteId,
        fqn: &str,
        generic_type_args: &[TypeId],
        args: &[LirCallArg],
        body: &LirExecutableBody,
        source_types: &TypeStore,
        transport: &crate::effect_lowered::LirCallTransportMetadata,
        slots: &[MirLocalSlot<'ctx>],
        abi: Option<&ProgramAbiQuery<'ctx>>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let source_site = self.published_lir_source_call_site(site_id);
        if source_site.is_none() && !generic_type_args.is_empty() {
            return Err(frontend_error(format!(
                "generic LIR direct call `{fqn}` lacks published LIR source call-site contract"
            )));
        }
        let plain_site = if source_site.is_none() {
            self.published_lir_plain_call_site(site_id)
        } else {
            None
        };
        let exact_callee = source_site
            .and_then(|site| site.contract.exact_callee.clone())
            .or_else(|| plain_site.and_then(|site| site.contract.exact_callee.clone()));
        let concrete_fqn = if let Some(exact) = exact_callee.as_ref() {
            exact.root_fqn.clone()
        } else if !generic_type_args.is_empty() {
            return Err(frontend_error(format!(
                "generic LIR direct call `{fqn}` lacks published exact callee binding"
            )));
        } else {
            fqn.to_string()
        };
        let concrete_fqn = concrete_fqn.as_str();
        let named_intrinsic_entry = source_site
            .and_then(|site| site.named_entry_name.as_deref())
            .map(str::to_string)
            .or_else(|| self.published_named_intrinsic_entry_name(concrete_fqn));
        if let Some(entry_name) = named_intrinsic_entry
            && let Some(value) = self.try_codegen_named_intrinsic_lir_direct_call(
                span,
                &entry_name,
                args,
                body,
                source_types,
                transport.array.as_ref(),
                slots,
            )?
        {
            return Ok(value);
        }
        if let Some(value) =
            self.codegen_lir_atomic_intrinsic(span, concrete_fqn, args, body, source_types, slots)?
        {
            return Ok(value);
        }
        if let Some(value) = self.codegen_lir_gc_handle_intrinsic(
            span,
            concrete_fqn,
            args,
            source_types,
            transport,
            slots,
        )? {
            return Ok(value);
        }
        if concrete_fqn == "scoop.unsafe.invoke" || fqn == "scoop.unsafe.invoke" {
            return self.codegen_lir_funptr_invoke_call(span, args, body, source_types, slots);
        }
        if let Some(value) = self.codegen_lir_top_level_funptr_direct_call(
            concrete_fqn,
            span,
            args,
            source_types,
            slots,
        )? {
            return Ok(value);
        }
        if let Some(fun_ty) = self.lir_top_level_function_value_type(concrete_fqn) {
            return self.codegen_lir_top_level_function_value_direct_call(
                concrete_fqn,
                span,
                args,
                &fun_ty,
                slots,
            );
        }
        let callable_abi = self.direct_call_abi_identity(concrete_fqn);
        let uses_effect_step_surface = callable_abi.uses_effect_bridge_abi();
        let (signature_types, param_names, source_param_tys, source_return_ty) = self
            .published_callable_signature_with_names(concrete_fqn)
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "LIR direct call `{concrete_fqn}` 缺少 LIR callable signature facts"
                ),
            })?;
        let (param_tys, return_ty_for_codegen) = self
            .published_signature_tys_as_codegen_tys(
                signature_types,
                source_param_tys.clone(),
                source_return_ty,
            )
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "LIR direct call `{concrete_fqn}` 的 LIR callable signature 无法映射到 LLVM codegen TypeStore"
                ),
            })?;
        if param_names.len() != param_tys.len() || args.len() != param_tys.len() {
            panic!(
                "codegen_lir_direct_call_with_type_args: LIR call ABI verifier accepted arity mismatch for `{concrete_fqn}`: params={} args={} names={}",
                param_tys.len(),
                args.len(),
                param_names.len()
            );
        }
        let mut uses_call_site_signature_tys = false;
        let declaration_param_tys = source_param_tys
            .iter()
            .copied()
            .enumerate()
            .map(|(idx, param_ty)| {
                if self.mir_type_contains_param(signature_types, param_ty)
                    && let Some(arg_ty) = args
                        .get(idx)
                        .and_then(|arg| self.lir_operand_type_id(body, &arg.value))
                {
                    uses_call_site_signature_tys = true;
                    arg_ty
                } else {
                    param_ty
                }
            })
            .collect::<Vec<_>>();
        let declaration_return_ty =
            if self.mir_type_contains_param(signature_types, source_return_ty) {
                uses_call_site_signature_tys = true;
                transport.result.source_ty
            } else {
                source_return_ty
            };
        let declaration_types = if uses_call_site_signature_tys {
            source_types
        } else {
            signature_types
        };

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
            .or_else(|| self.cg_ty_of_mir_type(signature_types, source_return_ty))
            .or_else(|| self.try_cg_ty_of_type_id(return_ty_for_codegen))
            .or_else(|| self.cg_ty_of_mir_type(source_types, transport.result.source_ty))
            .or_else(|| {
                self.equivalent_codegen_type_id(source_types, transport.result.source_ty)
                    .and_then(|ty| self.try_cg_ty_of_type_id(ty))
            })
            .unwrap_or_else(|| {
                panic!("codegen_lir_direct_call_with_type_args: LIR call ABI verifier accepted unsupported direct call return type")
            });
        let hidden_sret_result_ty = if native_abi.is_some() {
            None
        } else {
            self.hidden_sret_result_ty(span, ret_cg)?
        };
        let published_abi_symbol = self
            .published_late_lowered_program()
            .and_then(|program| program.abi_symbol_for_root(concrete_fqn))
            .map(|symbol| symbol.symbol.clone());
        let exact_symbol = exact_callee
            .as_ref()
            .map(|exact| exact.abi_symbol.as_str())
            .or(published_abi_symbol.as_deref())
            .unwrap_or("");
        let llvm_name = exact_callee
            .as_ref()
            .map(|exact| exact.abi_symbol.as_str().to_string())
            .or_else(|| published_abi_symbol.clone())
            .unwrap_or_else(|| {
                self.exported_abi_symbol_for_lir_callable(concrete_fqn)
                    .unwrap_or_else(|err| {
                        panic!("codegen_lir_direct_call_with_type_args: missing ABI symbol: {err}")
                    })
            });
        if concrete_fqn == "scoop.core.byteLength"
            || fqn == "scoop.core.byteLength"
            || exact_symbol.contains("scoop_core_byteLength")
            || llvm_name.contains("scoop_core_byteLength")
        {
            return self.codegen_lir_core_string_byte_length_call(span, args, ret_cg, slots);
        }
        if concrete_fqn == "scoop.core.getByte"
            || fqn == "scoop.core.getByte"
            || exact_symbol.contains("scoop_core_getByte")
            || llvm_name.contains("scoop_core_getByte")
        {
            return self.codegen_lir_core_string_get_byte_call(span, args, ret_cg, slots);
        }
        if uses_effect_step_surface
            && native_abi.is_none()
            && !callable_abi.is_extern()
            && let Some(abi) = abi
        {
            return self.codegen_lir_pure_effect_step_direct_call(
                span,
                abi,
                concrete_fqn,
                args,
                body,
                source_types,
                slots,
                ret_cg,
            );
        }
        let evaluated_args = self.codegen_bound_lir_call_args_from_signature(
            span,
            &param_names,
            &param_tys,
            args,
            body,
            source_types,
            slots,
            native_abi.is_some(),
            self.types,
        )?;

        let uses_explicit_effect_hidden_abi = uses_effect_step_surface;
        let mut llvm_args: Vec<BasicMetadataValueEnum<'ctx>> = Vec::with_capacity(
            evaluated_args.len()
                + usize::from(hidden_sret_result_ty.is_some())
                + self.explicit_effect_hidden_abi_param_count(uses_explicit_effect_hidden_abi)
                    as usize,
        );
        let sret_result_slot = if hidden_sret_result_ty.is_some() {
            let slot = self.create_entry_alloca(span, "lir_direct_call_sret", ret_cg)?;
            llvm_args.push(slot.into());
            Some(slot)
        } else {
            None
        };
        let effect_outcome_slot = if uses_explicit_effect_hidden_abi {
            let slot = self.alloc_effect_outcome_slot(span, "lir_direct_call")?;
            llvm_args.push(self.current_effect_ctx_arg().into());
            llvm_args.push(self.llvm_gc_i8_ptr_type().const_null().into());
            llvm_args.push(slot.into());
            Some(slot)
        } else {
            None
        };
        llvm_args.extend(evaluated_args.iter().map(|arg| arg.value));

        let llvm_fun = match self.module.get_function(&llvm_name) {
            Some(function) => function,
            None => {
                let declaration_surface = if callable_abi.is_extern() {
                    LlvmFunctionDeclarationSurface::RuntimeOrNativeImport
                } else {
                    LlvmFunctionDeclarationSurface::ExportedAbi
                };
                self.declare_lir_plain_fun_with_symbol(
                    &llvm_name,
                    declaration_surface,
                    concrete_fqn,
                    &declaration_param_tys,
                    declaration_return_ty,
                    declaration_types,
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
                let call_site = cg
                    .builder
                    .build_call(llvm_fun, &llvm_args, "lir_direct_call")?;
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
            self.sync_hidden_sret_result_roots(span, ret_cg, result_ptr, "lir_direct_call_sret")?;
        }
        let deferred_direct_result = if sret_result_slot.is_none() {
            self.defer_direct_call_result(span, ret_cg, call_site, "lir_direct_call_result")?
        } else {
            None
        };
        if let Some(outcome_slot) = effect_outcome_slot {
            self.maybe_record_active_suspend_site_effect_outcome(span, outcome_slot);
            self.emit_ordinary_call_effect_propagation_check_from_outcome(
                span,
                outcome_slot,
                "lir_direct_call_effect",
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
                        "lir_direct_call_sret",
                    )
                } else {
                    self.materialize_deferred_cg_value(
                        span,
                        "lir_direct_call_result_reload",
                        deferred_direct_result.unwrap_or_else(|| {
                            panic!("codegen_lir_direct_call_with_type_args: LIR call ABI verifier accepted missing deferred return value")
                        }),
                    )
                }
            }
        }
    }

    fn codegen_lir_top_level_funptr_direct_call(
        &mut self,
        callable_fqn: &str,
        span: crate::span::Span,
        args: &[LirCallArg],
        source_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        let Some(fun_ty) = self.lir_top_level_funptr_function_type(callable_fqn) else {
            return Ok(None);
        };
        if !fun_ty.effects.is_pure() {
            panic!(
                "codegen_lir_top_level_funptr_direct_call: effect-typed top-level FunPtr `{callable_fqn}` reached plain LIR direct call at {span:?}"
            );
        }

        let value = self
            .top_level_immutable_values
            .get(callable_fqn)
            .cloned()
            .unwrap_or_else(|| {
                panic!(
                    "codegen_lir_top_level_funptr_direct_call: LIR verifier accepted missing FunPtr metadata for `{callable_fqn}` at {span:?}"
                )
            });
        let funptr = self.codegen_top_level_immutable_value_access(span, &value)?;
        let (funptr_addr, funptr_int_ty) = funptr.as_int().unwrap_or_else(|| {
            panic!(
                "codegen_lir_top_level_funptr_direct_call: LIR verifier accepted non-int top-level FunPtr value"
            )
        });
        let expected_arity = fun_ty.params.len() + usize::from(fun_ty.receiver.is_some());
        if args.len() != expected_arity {
            panic!(
                "codegen_lir_top_level_funptr_direct_call: LIR verifier accepted FunPtr arity mismatch for `{callable_fqn}`"
            );
        }

        let param_tys = self.callable_value_param_tys(&fun_ty);
        let native_abi =
            self.classify_funptr_native_callable(span, &param_tys, fun_ty.return_ty)?;
        let ret_cg = native_abi.return_abi.cg_ty;
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
        let typed_fn_ptr = self.builder.build_int_to_ptr(
            casted_addr,
            self.llvm_ptr_type(AddressSpace::default()),
            "lir_top_level_funptr_typed",
        )?;

        let evaluated_args =
            self.codegen_lir_funptr_value_args(span, &fun_ty, args, source_types, slots)?;
        let llvm_args = evaluated_args
            .iter()
            .map(|arg| arg.value)
            .collect::<Vec<_>>();
        let call_site_result = self.emit_native_callable_call(
            span,
            &native_abi,
            NativeCallableTarget::Indirect {
                fn_ty: native_abi.fn_ty,
                ptr: typed_fn_ptr,
                call_name: "lir_top_level_funptr_call",
            },
            &llvm_args,
        );
        self.release_evaluated_call_arg_roots(&evaluated_args);
        let call_site = call_site_result?;
        let deferred_direct_result = self.defer_direct_call_result(
            span,
            ret_cg,
            call_site,
            "lir_top_level_funptr_call_result",
        )?;

        match ret_cg {
            CgTy::Unit => Ok(Some(CgValue::unit())),
            CgTy::Never => Ok(Some(CgValue::never())),
            _ => Ok(Some(self.materialize_deferred_cg_value(
                span,
                "lir_top_level_funptr_call_result_reload",
                deferred_direct_result.unwrap_or_else(|| {
                    panic!(
                        "codegen_lir_top_level_funptr_direct_call: LIR verifier accepted missing deferred FunPtr return value"
                    )
                }),
            )?)),
        }
    }

    fn codegen_lir_top_level_function_value_direct_call(
        &mut self,
        callable_fqn: &str,
        span: crate::span::Span,
        args: &[LirCallArg],
        fun_ty: &crate::ty::FunctionType,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if !fun_ty.effects.is_pure() {
            panic!(
                "codegen_lir_top_level_function_value_direct_call: effect-typed top-level function value `{callable_fqn}` reached plain LIR direct call at {span:?}"
            );
        }
        let value = self
            .top_level_immutable_values
            .get(callable_fqn)
            .cloned()
            .unwrap_or_else(|| {
                panic!(
                    "codegen_lir_top_level_function_value_direct_call: LIR verifier accepted missing function-value metadata for `{callable_fqn}` at {span:?}"
                )
            });
        let callee = self.codegen_top_level_immutable_value_access(span, &value)?;
        let callee = self.coerce_value(span, callee, CgTy::Ref)?;
        let closure_obj_i8 = self.expect_cg_pointer(callee, "top-level LIR function-value value");
        self.codegen_lir_function_value_call_from_closure_obj(
            span,
            closure_obj_i8,
            fun_ty,
            false,
            args,
            slots,
        )
    }

    fn lir_top_level_funptr_function_type(
        &self,
        callable_fqn: &str,
    ) -> Option<crate::ty::FunctionType> {
        let root = self.lir_global_root(callable_fqn)?;
        if root.kind != LirGlobalRootKind::TopLevelImmutableVal {
            return None;
        }
        match self.types.kind(root.ty?) {
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
            | TypeKind::Value(ValueTypeKind::Nominal(nominal))
                if nominal.fqn == "scoop.unsafe.FunPtr" && nominal.args.len() == 1 =>
            {
                let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = self.types.kind(nominal.args[0])
                else {
                    return None;
                };
                Some(fun_ty.clone())
            }
            _ => None,
        }
    }

    fn lir_top_level_function_value_type(
        &self,
        callable_fqn: &str,
    ) -> Option<crate::ty::FunctionType> {
        let root = self.lir_global_root(callable_fqn)?;
        if root.kind != LirGlobalRootKind::TopLevelImmutableVal {
            return None;
        }
        let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = self.types.kind(root.ty?) else {
            return None;
        };
        Some(fun_ty.clone())
    }

    fn lir_string_like_pointer(
        &mut self,
        span: crate::span::Span,
        value: CgValue<'ctx>,
        kind: &'static str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let value = match value.ty {
            CgTy::String | CgTy::Ref => value,
            _ => self.coerce_value(span, value, CgTy::String)?,
        };
        Ok(self.expect_cg_pointer(value, kind))
    }

    fn codegen_lir_core_string_byte_length_call(
        &mut self,
        span: crate::span::Span,
        args: &[LirCallArg],
        target_cg: CgTy,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 || args[0].name.is_some() {
            self.panic_verified_intrinsic_contract(
                "LIR core.byteLength",
                "argument count or named argument drift",
            );
        }

        let receiver = self.codegen_lir_operand_expected(
            args[0].span,
            &args[0].value,
            slots,
            Some(CgTy::String),
        )?;
        let receiver_ptr =
            self.lir_string_like_pointer(args[0].span, receiver, "core byteLength receiver value")?;
        let len_ptr = self.builder.build_struct_gep(
            self.llvm_scoop_string_type(),
            receiver_ptr,
            1,
            "core_byte_length_gep",
        )?;
        let raw = self
            .builder
            .build_load(self.context.i64_type(), len_ptr, "core_byte_length")?;
        let result = self.expect_int_value(raw, "core byteLength load type");
        let value = CgValue::int(
            result,
            IntTy {
                bits: 64,
                signed: true,
            },
        );
        self.coerce_value(span, value, target_cg)
    }

    fn codegen_lir_core_string_get_byte_call(
        &mut self,
        span: crate::span::Span,
        args: &[LirCallArg],
        target_cg: CgTy,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 2 || args.iter().any(|arg| arg.name.is_some()) {
            self.panic_verified_intrinsic_contract(
                "LIR core.getByte",
                "argument count or named argument drift",
            );
        }

        let receiver = self.codegen_lir_operand_expected(
            args[0].span,
            &args[0].value,
            slots,
            Some(CgTy::String),
        )?;
        let receiver_ptr =
            self.lir_string_like_pointer(args[0].span, receiver, "core getByte receiver value")?;
        let index = self.codegen_lir_operand_expected(
            args[1].span,
            &args[1].value,
            slots,
            Some(CgTy::Int(IntTy {
                bits: 64,
                signed: true,
            })),
        )?;
        let index_int = self.expect_cg_int(index, "core getByte index value").0;

        let i64_ty = self.context.i64_type();
        let i8_ty = self.context.i8_type();
        let len_ptr = self.builder.build_struct_gep(
            self.llvm_scoop_string_type(),
            receiver_ptr,
            1,
            "core_get_byte_len_gep",
        )?;
        let len_val = self
            .builder
            .build_load(i64_ty, len_ptr, "core_get_byte_len")?
            .into_int_value();
        let data_ptr_ptr = self.builder.build_struct_gep(
            self.llvm_scoop_string_type(),
            receiver_ptr,
            2,
            "core_get_byte_data_gep",
        )?;
        let data_ptr = self
            .builder
            .build_load(self.llvm_i8_ptr_type(), data_ptr_ptr, "core_get_byte_data")?
            .into_pointer_value();

        let current_fn = self
            .builder
            .get_insert_block()
            .unwrap()
            .get_parent()
            .unwrap();
        let in_bounds_bb = self
            .context
            .append_basic_block(current_fn, "getByte_in_bounds");
        let out_of_bounds_bb = self
            .context
            .append_basic_block(current_fn, "getByte_out_of_bounds");
        let merge_bb = self.context.append_basic_block(current_fn, "getByte_merge");

        let is_negative = self.builder.build_int_compare(
            inkwell::IntPredicate::SLT,
            index_int,
            i64_ty.const_zero(),
            "getByte_negative",
        )?;
        let not_negative_bb = self
            .context
            .append_basic_block(current_fn, "getByte_not_negative");
        self.builder
            .build_conditional_branch(is_negative, out_of_bounds_bb, not_negative_bb)?;

        self.builder.position_at_end(not_negative_bb);
        let is_ge_len = self.builder.build_int_compare(
            inkwell::IntPredicate::SGE,
            index_int,
            len_val,
            "getByte_ge_len",
        )?;
        self.builder
            .build_conditional_branch(is_ge_len, out_of_bounds_bb, in_bounds_bb)?;

        self.builder.position_at_end(out_of_bounds_bb);
        let zero_val = i64_ty.const_zero();
        self.builder.build_unconditional_branch(merge_bb)?;

        self.builder.position_at_end(in_bounds_bb);
        let byte_ptr = unsafe {
            self.builder.build_in_bounds_gep(
                i8_ty,
                data_ptr,
                &[index_int],
                "core_get_byte_elem_gep",
            )?
        };
        let byte_val = self
            .builder
            .build_load(i8_ty, byte_ptr, "core_get_byte_val")?
            .into_int_value();
        let byte_i64 = self
            .builder
            .build_int_z_extend(byte_val, i64_ty, "core_get_byte_zext")?;
        self.builder.build_unconditional_branch(merge_bb)?;

        self.builder.position_at_end(merge_bb);
        let phi = self.builder.build_phi(i64_ty, "core_get_byte_result")?;
        phi.add_incoming(&[(&zero_val, out_of_bounds_bb), (&byte_i64, in_bounds_bb)]);
        let value = CgValue::int(
            phi.as_basic_value().into_int_value(),
            IntTy {
                bits: 64,
                signed: true,
            },
        );
        self.coerce_value(span, value, target_cg)
    }

    pub(in crate::llvm::codegen) fn codegen_mir_class_ctor_call_at_site(
        &mut self,
        span: crate::span::Span,
        _class_layout_key: &hir::ClassInstanceKey,
        _args: &[mir_source::CallArg],
        _slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        Err(frontend_error(format!(
            "direct class ctor call at {span:?} reached MIR direct-call lowering without Rvalue::ClassCtor metadata"
        )))
    }

    pub(in crate::llvm::codegen) fn codegen_mir_closure_call(
        &mut self,
        span: crate::span::Span,
        callee: &mir_source::Operand,
        fn_ptr: &str,
        args: &[mir_source::CallArg],
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

    pub(in crate::llvm::codegen) fn codegen_lir_closure_call(
        &mut self,
        span: crate::span::Span,
        callee: &LirOperand,
        fn_ptr: LirCallableId,
        args: &[LirCallArg],
        fun_ty: &crate::ty::FunctionType,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let call_may_suspend = self
            .published_late_lowered_program()
            .and_then(|program| program.callable_by_id(fn_ptr))
            .map(|callable| callable.effect_step_abi().is_some())
            .unwrap_or_else(|| {
                self.managed_callable_abi_identity_from_fun_ty(fun_ty)
                    .uses_effect_bridge_abi()
            });
        let callee_value =
            self.codegen_lir_operand_expected(span, callee, slots, Some(CgTy::Ref))?;
        let callee_value = self.coerce_value(span, callee_value, CgTy::Ref)?;
        let Some(BasicValueEnum::PointerValue(closure_obj_i8)) = callee_value.value else {
            panic!(
                "codegen_lir_closure_call: LIR call ABI verifier accepted non-pointer closure callee"
            )
        };
        self.codegen_lir_function_value_call_from_closure_obj(
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
        callee: &mir_source::Operand,
        args: &[mir_source::CallArg],
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

    pub(in crate::llvm::codegen) fn codegen_lir_fun_value_call(
        &mut self,
        span: crate::span::Span,
        callee: &LirOperand,
        args: &[LirCallArg],
        fun_ty: &crate::ty::FunctionType,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let callee_value =
            self.codegen_lir_operand_expected(span, callee, slots, Some(CgTy::Ref))?;
        let callee_value = self.coerce_value(span, callee_value, CgTy::Ref)?;
        let Some(BasicValueEnum::PointerValue(closure_obj_i8)) = callee_value.value else {
            panic!(
                "codegen_lir_fun_value_call: LIR call ABI verifier accepted non-pointer function-value callee"
            );
        };
        self.codegen_lir_function_value_call_from_closure_obj(
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
        callee: &mir_source::Operand,
        args: &[mir_source::CallArg],
        fun_ty: &crate::ty::FunctionType,
        mir_ctx: (&mir_source::Body, &TypeStore, &[MirLocalSlot<'ctx>]),
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
    pub(in crate::llvm::codegen) fn codegen_lir_funptr_value_call(
        &mut self,
        span: crate::span::Span,
        callee: &LirOperand,
        args: &[LirCallArg],
        fun_ty: &crate::ty::FunctionType,
        lir_ctx: (&TypeStore, &[MirLocalSlot<'ctx>]),
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let (source_types, slots) = lir_ctx;
        let callee_value = self.codegen_lir_operand(span, callee, slots)?;
        let (funptr_addr, funptr_int_ty) = callee_value.as_int().unwrap_or_else(|| {
            panic!(
                "codegen_lir_funptr_value_call: LIR verifier accepted non-int FunPtr callee value"
            )
        });
        let expected_arity = fun_ty.params.len() + usize::from(fun_ty.receiver.is_some());
        if args.len() != expected_arity {
            panic!("codegen_lir_funptr_value_call: LIR verifier accepted arity mismatch");
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
                .build_int_to_ptr(casted_addr, fun_ptr_ty, "lir_funptr_typed")?;

        let mut llvm_args: Vec<BasicMetadataValueEnum<'ctx>> = Vec::with_capacity(args.len());
        let evaluated_args =
            self.codegen_lir_funptr_value_args(span, fun_ty, args, source_types, slots)?;
        for arg in &evaluated_args {
            llvm_args.push(arg.value);
        }

        let call_site_result = self.emit_native_callable_call(
            span,
            &native_abi,
            NativeCallableTarget::Indirect {
                fn_ty: native_abi.fn_ty,
                ptr: typed_fn_ptr,
                call_name: "lir_call_funptr",
            },
            &llvm_args,
        );
        self.release_evaluated_call_arg_roots(&evaluated_args);
        let call_site = call_site_result?;
        let deferred_direct_result =
            self.defer_direct_call_result(span, ret_cg, call_site, "lir_funptr_call_result")?;

        match ret_cg {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            _ => self.materialize_deferred_cg_value(
                span,
                "lir_funptr_call_result_reload",
                deferred_direct_result.unwrap_or_else(|| {
                    panic!(
                        "codegen_lir_funptr_value_call: LIR verifier accepted missing deferred return value"
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
        args: &[mir_source::CallArg],
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
    pub(in crate::llvm::codegen) fn codegen_lir_function_value_call_from_closure_obj(
        &mut self,
        span: crate::span::Span,
        closure_obj_i8: PointerValue<'ctx>,
        fun_ty: &crate::ty::FunctionType,
        call_may_suspend: bool,
        args: &[LirCallArg],
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let expected_arity = fun_ty.params.len() + usize::from(fun_ty.receiver.is_some());
        if args.len() != expected_arity {
            panic!(
                "codegen_lir_function_value_call_from_closure_obj: LIR call ABI verifier accepted function-value arity mismatch"
            );
        }
        let deferred_closure =
            self.defer_gc_ref_pointer(span, "lir_function_value_closure", closure_obj_i8)?;

        let closure_ty = self.llvm_closure_object_type();
        let closure_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let ret_cg = self.try_cg_ty_of_type_id(fun_ty.return_ty).unwrap_or_else(|| {
            panic!("codegen_lir_function_value_call_from_closure_obj: LIR call ABI verifier accepted unsupported function-value return type")
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
                "aggregate LIR function-value returns should have been lowered through hidden sret"
            ),
        };

        let mut llvm_args: Vec<BasicMetadataValueEnum<'ctx>> = Vec::with_capacity(
            1 + args.len()
                + usize::from(hidden_sret_result_ty.is_some())
                + self.explicit_effect_hidden_abi_param_count(call_may_suspend) as usize,
        );
        let sret_result_slot = if hidden_sret_result_ty.is_some() {
            let slot = self.create_entry_alloca(span, "lir_closure_call_sret", ret_cg)?;
            llvm_args.push(slot.into());
            Some(slot)
        } else {
            None
        };
        let effect_outcome_slot = if call_may_suspend {
            let slot = self.alloc_effect_outcome_slot(span, "lir_closure_call")?;
            llvm_args.push(self.current_effect_ctx_arg().into());
            llvm_args.push(self.llvm_gc_i8_ptr_type().const_null().into());
            llvm_args.push(slot.into());
            Some(slot)
        } else {
            None
        };
        let evaluated_args = self.codegen_lir_callable_value_args(span, fun_ty, args, slots)?;

        let closure_obj_i8 = self.reload_deferred_gc_ref_without_clearing(
            span,
            "lir_closure_call_obj_reload",
            &deferred_closure,
        )?;
        let closure_ptr = self.builder.build_pointer_cast(
            closure_obj_i8,
            closure_ptr_ty,
            "lir_closure_obj_ptr",
        )?;
        let env_ptr_gep =
            self.builder
                .build_struct_gep(closure_ty, closure_ptr, 1, "lir_closure_env_gep")?;
        let fn_ptr_gep =
            self.builder
                .build_struct_gep(closure_ty, closure_ptr, 2, "lir_closure_fn_gep")?;
        let env_ptr = self
            .builder
            .build_load(gc_i8_ptr_ty, env_ptr_gep, "lir_closure_env")?
            .into_pointer_value();
        let fn_ptr_raw = self
            .builder
            .build_load(i8_ptr_ty, fn_ptr_gep, "lir_closure_fn")?
            .into_pointer_value();
        let typed_fn_ptr = self.builder.build_pointer_cast(
            fn_ptr_raw,
            self.llvm_ptr_type(AddressSpace::default()),
            "lir_closure_fn_typed",
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
                "lir_call_closure",
            )?;
            if let Some(result_ty) = hidden_sret_result_ty {
                cg.add_sret_attribute_to_call(call_site, 0, result_ty);
            }
            Ok(call_site)
        });
        self.release_evaluated_call_arg_roots(&evaluated_args);
        let call_site = call_site_result?;
        if let Some(result_ptr) = sret_result_slot {
            self.sync_hidden_sret_result_roots(span, ret_cg, result_ptr, "lir_closure_call_sret")?;
        }
        let deferred_direct_result = if sret_result_slot.is_none() {
            self.defer_direct_call_result(span, ret_cg, call_site, "lir_closure_call_result")?
        } else {
            None
        };
        if let Some(outcome_slot) = effect_outcome_slot {
            self.maybe_record_active_suspend_site_effect_outcome(span, outcome_slot);
            self.emit_ordinary_call_effect_propagation_check_from_outcome(
                span,
                outcome_slot,
                "lir_closure_call_effect",
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
                        "lir_closure_call_sret",
                    )
                } else {
                    self.materialize_deferred_cg_value(
                        span,
                        "lir_closure_call_result_reload",
                        deferred_direct_result.unwrap_or_else(|| {
                            panic!("codegen_lir_function_value_call_from_closure_obj: LIR call ABI verifier accepted missing deferred return value")
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
        site_id: Option<mir_source::SiteId>,
        kind: &mir_source::CallKind,
        args: &[mir_source::CallArg],
        body: &mir_source::Body,
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
        site_id: Option<mir_source::SiteId>,
        kind: &mir_source::CallKind,
        args: &[mir_source::CallArg],
        body: &mir_source::Body,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
        allow_effect_typed_dispatch_signature: bool,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match kind {
            mir_source::CallKind::Closure { callee, fn_ptr } => {
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
            mir_source::CallKind::FunValue { callee } => {
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
            mir_source::CallKind::FunPtr { callee } => {
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
            mir_source::CallKind::Virtual { receiver, .. } => {
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
            mir_source::CallKind::Interface { receiver, .. } => {
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
            mir_source::CallKind::Direct { .. } | mir_source::CallKind::Resume { .. } => {
                panic!(
                    "codegen_mir_plain_dynamic_call_with_policy: MIR call ABI verifier accepted unsupported plain dynamic call kind"
                )
            }
        }
    }

    pub(in crate::llvm::codegen) fn codegen_mir_plain_function_value_call(
        &mut self,
        span: crate::span::Span,
        callee: &mir_source::Operand,
        args: &[mir_source::CallArg],
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
        args: &[mir_source::CallArg],
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
