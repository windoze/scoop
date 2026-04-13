impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(super) fn current_raise_target(&self) -> Option<inkwell::basic_block::BasicBlock<'ctx>> {
        self.raise_target_stack.last().copied()
    }

    pub(super) fn push_raise_target(&mut self, target: inkwell::basic_block::BasicBlock<'ctx>) {
        self.raise_target_stack.push(target);
    }

    pub(super) fn pop_raise_target(&mut self) {
        let _ = self.raise_target_stack.pop();
    }

    pub(super) fn current_effect_unwind_target(
        &self,
        op_fqn: &str,
    ) -> Option<inkwell::basic_block::BasicBlock<'ctx>> {
        self.effect_unwind_target_stack
            .iter()
            .rev()
            .find(|t| t.op_fqn == op_fqn)
            .map(|t| t.target)
    }

    pub(super) fn push_effect_unwind_target(
        &mut self,
        op_fqn: &str,
        target: inkwell::basic_block::BasicBlock<'ctx>,
    ) {
        self.effect_unwind_target_stack.push(EffectUnwindTarget {
            op_fqn: op_fqn.to_string(),
            target,
        });
    }

    pub(super) fn pop_effect_unwind_target(&mut self) {
        let _ = self.effect_unwind_target_stack.pop();
    }

    pub(super) fn current_immediate_resume_ctx(&self) -> Option<ImmediateResumeCtx<'ctx>> {
        self.immediate_resume_ctx_stack.last().copied()
    }

    pub(super) fn push_immediate_resume_ctx(&mut self, ctx: ImmediateResumeCtx<'ctx>) {
        self.immediate_resume_ctx_stack.push(ctx);
    }

    pub(super) fn pop_immediate_resume_ctx(&mut self) {
        let _ = self.immediate_resume_ctx_stack.pop();
    }

    fn collect_sibling_nonresuming_plan<'hir>(
        &mut self,
        sibling_nonresuming_arms: &[&'hir hir::HandleArm],
    ) -> Result<SiblingNonresumingPlan<'hir>, LlvmEmitError> {
        let mut raise_arm: Option<&'hir hir::HandleArm> = None;
        let mut custom_arms: Vec<SiblingNonresumingArm<'hir>> = Vec::new();
        for arm in sibling_nonresuming_arms {
            if arm.op.binders.len() != 1 {
                let kind = if arm.op.op.fqn == "scoop.core.Raise.raise" {
                    "handle binder count (only 1 supported)"
                } else {
                    "handle binder count (custom non-resuming, only single payload supported)"
                };
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind,
                    at: arm.op.span.into(),
                });
            }
            if arm.op.op.fqn == "scoop.core.Raise.raise" {
                if raise_arm.is_some() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle mixed Raise arms (only 1 supported)",
                        at: arm.span.into(),
                    });
                }
                raise_arm = Some(*arm);
                continue;
            }
            custom_arms.push(SiblingNonresumingArm {
                arm,
                op_tag: self.effect_op_tag(&arm.op.op.fqn),
            });
        }
        Ok(SiblingNonresumingPlan {
            raise_arm,
            custom_arms,
        })
    }

    fn build_sibling_nonresuming_dispatch_blocks(
        &self,
        func: FunctionValue<'ctx>,
        prefix: &str,
        plan: &SiblingNonresumingPlan<'_>,
    ) -> SiblingNonresumingDispatchBlocks<'ctx> {
        let effect_dispatch_bb = if plan.has_any() {
            Some(
                self.context
                    .append_basic_block(func, &format!("{prefix}_effect_dispatch")),
            )
        } else {
            None
        };
        let effect_dispatch_nomatch_bb = if plan.has_any() {
            Some(
                self.context
                    .append_basic_block(func, &format!("{prefix}_effect_dispatch_nomatch")),
            )
        } else {
            None
        };
        let raise_catch_bb = if plan.raise_arm.is_some() {
            Some(
                self.context
                    .append_basic_block(func, &format!("{prefix}_raise_catch")),
            )
        } else {
            None
        };
        let mut custom_catch_bbs: Vec<inkwell::basic_block::BasicBlock<'ctx>> = Vec::new();
        for idx in 0..plan.custom_arms.len() {
            custom_catch_bbs.push(
                self.context
                    .append_basic_block(func, &format!("{prefix}_custom_catch_{idx}")),
            );
        }
        SiblingNonresumingDispatchBlocks {
            effect_dispatch_bb,
            effect_dispatch_nomatch_bb,
            raise_catch_bb,
            custom_catch_bbs,
        }
    }

    fn collect_escape_capture_metas_from_plan(
        &mut self,
        span: crate::span::Span,
        handle: &hir::HandleExpr,
        capture_ids: &HashSet<hir::SymbolId>,
        type_kind: &'static str,
        missing_kind: &'static str,
    ) -> Result<(Vec<EscapeCaptureMeta>, Vec<EscapeCaptureMeta>), LlvmEmitError> {
        let decl_map = Self::collect_escape_decl_map(handle);
        let mut sorted_ids = capture_ids.iter().copied().collect::<Vec<_>>();
        sorted_ids.sort_by_key(|id| id.as_u32());

        let mut outer_visible_supported: Vec<EscapeCaptureMeta> = Vec::new();
        let mut body_visible_supported: Vec<EscapeCaptureMeta> = Vec::new();

        for id in sorted_ids {
            if let Some(local) = self.env.get(id) {
                if self.escape_capture_storage_kind(span, local.ty)?.is_none() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: type_kind,
                        at: span.into(),
                    });
                }
                outer_visible_supported.push(EscapeCaptureMeta {
                    id,
                    hir_ty: local.hir_ty,
                    ty: local.ty,
                    mutable: local.mutable,
                });
                continue;
            }

            let Some(info) = decl_map.get(&id) else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: missing_kind,
                    at: span.into(),
                });
            };
            let decl = info.decl;
            let decl_ty = self
                .cg_ty_of(decl.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: type_kind,
                    at: decl.span.into(),
                })?;
            if self.escape_capture_storage_kind(decl.span, decl_ty)?.is_none() {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: type_kind,
                    at: decl.span.into(),
                });
            }
            body_visible_supported.push(EscapeCaptureMeta {
                id,
                hir_ty: Some(decl.ty),
                ty: decl_ty,
                mutable: decl.mutable,
            });
        }

        Ok((outer_visible_supported, body_visible_supported))
    }

    fn build_escape_handle_blocks(
        &self,
        func: FunctionValue<'ctx>,
        prefix: &str,
        with_dispatch: bool,
        has_finally: bool,
    ) -> EscapeHandleBlocks<'ctx> {
        EscapeHandleBlocks {
            body_bb: self
                .context
                .append_basic_block(func, &format!("{prefix}_body")),
            dispatch_bb: if with_dispatch {
                Some(
                    self.context
                        .append_basic_block(func, &format!("{prefix}_dispatch")),
                )
            } else {
                None
            },
            dispatch_nomatch_bb: if with_dispatch {
                Some(
                    self.context
                        .append_basic_block(func, &format!("{prefix}_dispatch_nomatch")),
                )
            } else {
                None
            },
            arm_bb: self
                .context
                .append_basic_block(func, &format!("{prefix}_arm")),
            done_bb: self
                .context
                .append_basic_block(func, &format!("{prefix}_done")),
            finally_bb: if has_finally {
                Some(
                    self.context
                        .append_basic_block(func, &format!("{prefix}_finally")),
                )
            } else {
                None
            },
            finally_unwind_bb: if has_finally {
                Some(
                    self.context
                        .append_basic_block(func, &format!("{prefix}_finally_unwind")),
                )
            } else {
                None
            },
        }
    }

    fn build_mixed_escape_resume_blocks(
        &self,
        func: FunctionValue<'ctx>,
        prefix: &str,
    ) -> MixedEscapeResumeBlocks<'ctx> {
        MixedEscapeResumeBlocks {
            dispatch_bb: self
                .context
                .append_basic_block(func, &format!("{prefix}_resume_dispatch")),
            state0_bb: self
                .context
                .append_basic_block(func, &format!("{prefix}_resume_state0")),
            state1_bb: self
                .context
                .append_basic_block(func, &format!("{prefix}_resume_state1")),
            arm_bb: self
                .context
                .append_basic_block(func, &format!("{prefix}_resume_arm")),
            done_bb: self
                .context
                .append_basic_block(func, &format!("{prefix}_done")),
            bad_state_bb: self
                .context
                .append_basic_block(func, &format!("{prefix}_bad_state")),
            finally_bb: self
                .context
                .append_basic_block(func, &format!("{prefix}_finally")),
            finally_unwind_bb: self
                .context
                .append_basic_block(func, &format!("{prefix}_finally_unwind")),
        }
    }

    fn escape_capture_storage_kind(
        &mut self,
        at: crate::span::Span,
        ty: CgTy,
    ) -> Result<Option<EscapeCaptureStorageKind>, LlvmEmitError> {
        Ok(match ty {
            CgTy::Bool | CgTy::Float64 | CgTy::Float32 | CgTy::Int(_) => {
                Some(EscapeCaptureStorageKind::Word)
            }
            CgTy::Ref | CgTy::String => Some(EscapeCaptureStorageKind::GcRef),
            CgTy::Enum(enum_ty) => {
                let layout = self.cg_enum_layout(at, enum_ty)?;
                if matches!(
                    layout.repr,
                    CgEnumRepr::Niche {
                        storage: NicheStorage::Pointer,
                        ..
                    }
                ) {
                    Some(EscapeCaptureStorageKind::GcRef)
                } else {
                    None
                }
            }
            CgTy::Unit | CgTy::Never | CgTy::Tuple(_) | CgTy::Struct(_) => None,
        })
    }

    fn restore_escape_capture_local_from_state(
        &mut self,
        at: crate::span::Span,
        field_ptr: PointerValue<'ctx>,
        ty: CgTy,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let ptr = self.create_entry_alloca(at, name, ty)?;
        match self.escape_capture_storage_kind(at, ty)? {
            Some(EscapeCaptureStorageKind::Word) => {
                let loaded = self
                    .builder
                    .build_load(self.context.i64_type(), field_ptr, "escape_cap_word")?
                    .into_int_value();
                let restored = self.decode_u64_word_to_cg_value(at, loaded, ty)?;
                let _ = self.store_local_value(at, ptr, ty, restored)?;
            }
            Some(EscapeCaptureStorageKind::GcRef) => {
                let loaded = self
                    .builder
                    .build_load(self.llvm_gc_i8_ptr_type(), field_ptr, "escape_cap_gc_ref")?
                    .into_pointer_value();
                let _ = self.store_local_value(
                    at,
                    ptr,
                    ty,
                    CgValue {
                        ty,
                        value: Some(loaded.into()),
                    },
                )?;
            }
            None => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle escape capture local type",
                    at: at.into(),
                });
            }
        }
        Ok(ptr)
    }

    fn write_escape_capture_local_to_state(
        &mut self,
        at: crate::span::Span,
        field_ptr: PointerValue<'ctx>,
        local_ptr: PointerValue<'ctx>,
        ty: CgTy,
    ) -> Result<(), LlvmEmitError> {
        match self.escape_capture_storage_kind(at, ty)? {
            Some(EscapeCaptureStorageKind::Word) => {
                let llvm_ty = self.llvm_basic_type_of(at, ty)?;
                let loaded = self
                    .builder
                    .build_load(llvm_ty, local_ptr, "escape_cap_load")?;
                let loaded_v = self.cg_value_from_loaded(at, ty, loaded)?;
                let word = self.coerce_u64_word(at, loaded_v)?;
                let _ = self.builder.build_store(field_ptr, word)?;
            }
            Some(EscapeCaptureStorageKind::GcRef) => {
                let llvm_ty = self.llvm_basic_type_of(at, ty)?;
                let loaded = self
                    .builder
                    .build_load(llvm_ty, local_ptr, "escape_cap_load_gc")?;
                let BasicValueEnum::PointerValue(ptr) = loaded else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle escape capture value type (ptr)",
                        at: at.into(),
                    });
                };
                let casted = self.builder.build_pointer_cast(
                    ptr,
                    self.llvm_gc_i8_ptr_type(),
                    "escape_cap_gc_ref_i8",
                )?;
                let _ = self.store_local_value(
                    at,
                    field_ptr,
                    ty,
                    CgValue {
                        ty,
                        value: Some(casted.into()),
                    },
                )?;
            }
            None => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle escape capture local type",
                    at: at.into(),
                });
            }
        }
        Ok(())
    }

    fn zero_init_escape_capture_state_field(
        &mut self,
        at: crate::span::Span,
        field_ptr: PointerValue<'ctx>,
        ty: CgTy,
    ) -> Result<(), LlvmEmitError> {
        match self.escape_capture_storage_kind(at, ty)? {
            Some(EscapeCaptureStorageKind::Word) => {
                let _ = self
                    .builder
                    .build_store(field_ptr, self.context.i64_type().const_zero())?;
            }
            Some(EscapeCaptureStorageKind::GcRef) => {
                let _ = self
                    .builder
                    .build_store(field_ptr, self.llvm_gc_i8_ptr_type().const_null())?;
            }
            None => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle escape capture local type",
                    at: at.into(),
                });
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn capture_escape_state_with_pc(
        &mut self,
        at: crate::span::Span,
        state_ty: inkwell::types::StructType<'ctx>,
        state_ptr: PointerValue<'ctx>,
        outer_visible_supported: &[EscapeCaptureMeta],
        outer_field_base: u32,
        body_visible_supported: &[EscapeCaptureMeta],
        body_field_base: u32,
        pc_field_idx: u32,
        next_pc: usize,
    ) -> Result<(), LlvmEmitError> {
        for (idx, cap) in outer_visible_supported.iter().enumerate() {
            let field_idx = outer_field_base.saturating_add(idx as u32);
            let field_ptr = self.builder.build_struct_gep(
                state_ty,
                state_ptr,
                field_idx,
                "capture_escape_state_outer_gep",
            )?;
            let local = self
                .env
                .get(cap.id)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape capture local not found",
                    at: at.into(),
                })?;
            if local.ty != cap.ty {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape capture local type mismatch",
                    at: at.into(),
                });
            }
            self.write_escape_capture_local_to_state(at, field_ptr, local.ptr, cap.ty)?;
        }

        for (idx, cap) in body_visible_supported.iter().enumerate() {
            let field_idx = body_field_base.saturating_add(idx as u32);
            let field_ptr = self.builder.build_struct_gep(
                state_ty,
                state_ptr,
                field_idx,
                "capture_escape_state_body_gep",
            )?;
            let Some(local) = self.env.get(cap.id) else {
                continue;
            };
            if local.ty != cap.ty {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi escape capture local type mismatch",
                    at: at.into(),
                });
            }
            self.write_escape_capture_local_to_state(at, field_ptr, local.ptr, cap.ty)?;
        }

        let pc_ptr = self.builder.build_struct_gep(
            state_ty,
            state_ptr,
            pc_field_idx,
            "capture_escape_state_pc_gep",
        )?;
        let _ = self.builder.build_store(
            pc_ptr,
            self.context.i32_type().const_int(next_pc as u64, false),
        )?;
        Ok(())
    }


    /// 读取运行时 TLS effect flag，并返回 `i1`（是否 active）。
    ///
    /// 说明：这里直接调用 runtime C ABI（`scoop_effect_is_active`），避免把该读取当作"普通函数调用"
    /// 从而触发递归插桩（call site 检查 flag → 再调用 is_active → 再检查...）。
    pub(super) fn emit_effect_is_active_i1(
        &mut self,
        at: crate::span::Span,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let rt = self.declare_runtime_effect_is_active();
        let call = self.builder.build_call(rt, &[], "effect_is_active")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "effect is_active return value",
                at: at.into(),
            })?;
        let BasicValueEnum::IntValue(active_i32) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "effect is_active return type",
                at: at.into(),
            });
        };
        Ok(self.builder.build_int_compare(
            IntPredicate::NE,
            active_i32,
            self.context.i32_type().const_zero(),
            "effect_active",
        )?)
    }

    fn abi_payload_box_name(payload_ty: CgTy) -> String {
        use std::hash::{Hash, Hasher};

        let mut h = std::collections::hash_map::DefaultHasher::new();
        format!("{payload_ty:?}").hash(&mut h);
        format!("scoop.runtime.AbiPayloadBox__{}", h.finish())
    }

    fn get_or_create_abi_payload_box_type(
        &mut self,
        at: crate::span::Span,
        payload_ty: CgTy,
    ) -> Result<inkwell::types::StructType<'ctx>, LlvmEmitError> {
        let box_ty_name = Self::abi_payload_box_name(payload_ty);
        if let Some(existing) = self.context.get_struct_type(&box_ty_name) {
            return Ok(existing);
        }

        let payload_llvm_ty = self.llvm_basic_type_of(at, payload_ty)?;
        let header_ty = self.llvm_gc_object_header_type();
        let box_ty = self.context.opaque_struct_type(&box_ty_name);
        box_ty.set_body(&[header_ty.into(), payload_llvm_ty], false);
        Ok(box_ty)
    }

    fn encode_abi_payload_transport(
        &mut self,
        at: crate::span::Span,
        value: CgValue<'ctx>,
    ) -> Result<AbiPayloadTransport<'ctx>, LlvmEmitError> {
        let i64_ty = self.context.i64_type();
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();

        match value.ty {
            CgTy::Unit | CgTy::Never => Ok(AbiPayloadTransport {
                word: i64_ty.const_zero(),
                gc_ref: None,
            }),
            CgTy::Bool | CgTy::Float64 | CgTy::Float32 | CgTy::Int(_) => {
                let word = self.coerce_u64_word(at, value)?;
                Ok(AbiPayloadTransport { word, gc_ref: None })
            }
            CgTy::String | CgTy::Ref => {
                let Some(raw) = value.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "ABI payload encode gc ref value",
                        at: at.into(),
                    });
                };
                let BasicValueEnum::PointerValue(ptr) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "ABI payload encode gc ref type",
                        at: at.into(),
                    });
                };
                let ptr_i8 =
                    self.builder
                        .build_pointer_cast(ptr, gc_i8_ptr_ty, "abi_payload_gc_ref_i8")?;
                Ok(AbiPayloadTransport {
                    word: i64_ty.const_zero(),
                    gc_ref: Some(ptr_i8),
                })
            }
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                let Some(raw) = value.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "ABI payload encode aggregate value",
                        at: at.into(),
                    });
                };

                let box_ty = self.get_or_create_abi_payload_box_type(at, value.ty)?;
                let box_ty_name = Self::abi_payload_box_name(value.ty);
                let box_size = self.target_data.get_store_size(&box_ty);
                let trace_start = self
                    .target_data
                    .offset_of_element(&box_ty, 1)
                    .unwrap_or(box_size);
                let box_desc_name = format!("__scoop_type_desc_abi_payload_box__{box_ty_name}");
                let box_desc = self.get_or_create_type_descriptor_global(TypeDescriptorSpec {
                    at,
                    global_name: &box_desc_name,
                    canonical_name: &box_ty_name,
                    obj_ty: box_ty,
                    trace_start_offset_bytes: trace_start,
                    parent: None,
                    itable: None,
                    vtable: None,
                })?;

                let rt_alloc = self.declare_runtime_alloc_typed();
                let size_v = i64_ty.const_int(box_size, false);
                let desc_i8 = self.builder.build_pointer_cast(
                    box_desc.as_pointer_value(),
                    i8_ptr_ty,
                    "abi_payload_box_desc_i8",
                )?;
                let alloc_call = self.builder.build_call(
                    rt_alloc,
                    &[desc_i8.into(), size_v.into()],
                    "abi_payload_box_alloc",
                )?;
                let box_raw = alloc_call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "ABI payload box alloc return",
                        at: at.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(box_gc_ptr) = box_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "ABI payload box alloc return type",
                        at: at.into(),
                    });
                };

                let box_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
                let box_typed = self.builder.build_pointer_cast(
                    box_gc_ptr,
                    box_ptr_ty,
                    "abi_payload_box_typed",
                )?;
                let payload_ptr = self.builder.build_struct_gep(
                    box_ty,
                    box_typed,
                    1,
                    "abi_payload_box_payload_gep",
                )?;
                let _ = self.builder.build_store(payload_ptr, raw)?;
                let box_i8 = self.builder.build_pointer_cast(
                    box_gc_ptr,
                    gc_i8_ptr_ty,
                    "abi_payload_box_i8",
                )?;
                Ok(AbiPayloadTransport {
                    word: i64_ty.const_zero(),
                    gc_ref: Some(box_i8),
                })
            }
        }
    }

    pub(super) fn decode_abi_payload_transport(
        &mut self,
        at: crate::span::Span,
        word: IntValue<'ctx>,
        gc_ref: PointerValue<'ctx>,
        ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match ty {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            CgTy::Bool | CgTy::Float64 | CgTy::Float32 | CgTy::Int(_) => {
                self.decode_u64_word_to_cg_value(at, word, ty)
            }
            CgTy::String => {
                let str_ptr_ty = self.llvm_scoop_string_ptr_type();
                let s =
                    self.builder
                        .build_pointer_cast(gc_ref, str_ptr_ty, "abi_payload_string")?;
                Ok(CgValue {
                    ty: CgTy::String,
                    value: Some(s.into()),
                })
            }
            CgTy::Ref => Ok(CgValue {
                ty: CgTy::Ref,
                value: Some(gc_ref.into()),
            }),
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                let payload_llvm_ty = self.llvm_basic_type_of(at, ty)?;
                let box_ty = self.get_or_create_abi_payload_box_type(at, ty)?;
                let box_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
                let box_ptr =
                    self.builder
                        .build_pointer_cast(gc_ref, box_ptr_ty, "abi_payload_box_ptr")?;
                let payload_ptr = self.builder.build_struct_gep(
                    box_ty,
                    box_ptr,
                    1,
                    "abi_payload_box_payload_gep",
                )?;
                let loaded = self.builder.build_load(
                    payload_llvm_ty,
                    payload_ptr,
                    "abi_payload_box_payload",
                )?;
                Ok(CgValue {
                    ty,
                    value: Some(loaded),
                })
            }
        }
    }

    pub(super) fn llvm_callee_suspend_state_prefix_type(&self) -> inkwell::types::StructType<'ctx> {
        const TY_NAME: &str = "scoop.runtime.CalleeSuspendStatePrefix";
        if let Some(existing) = self.context.get_struct_type(TY_NAME) {
            return existing;
        }

        let ty = self.context.opaque_struct_type(TY_NAME);
        let header_ty = self.llvm_gc_object_header_type();
        let i64_ty = self.context.i64_type();
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        ty.set_body(
            &[header_ty.into(), i64_ty.into(), gc_i8_ptr_ty.into()],
            false,
        );
        ty
    }

    pub(super) fn callee_suspend_first_local_field_index() -> u32 {
        3
    }

    pub(super) fn get_or_create_callee_suspend_state_type(
        &mut self,
        at: crate::span::Span,
        state_ty_name: &str,
        saved_locals: &[CalleeSuspendLocal],
    ) -> Result<inkwell::types::StructType<'ctx>, LlvmEmitError> {
        if let Some(existing) = self.context.get_struct_type(state_ty_name) {
            return Ok(existing);
        }

        let ty = self.context.opaque_struct_type(state_ty_name);
        let header_ty = self.llvm_gc_object_header_type();
        let i64_ty = self.context.i64_type();
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let mut fields: Vec<BasicTypeEnum<'ctx>> = Vec::new();
        fields.push(header_ty.into()); // 0: GC header
        fields.push(i64_ty.into()); // 1: resume_word
        fields.push(gc_i8_ptr_ty.into()); // 2: resume_gc_ref / boxed aggregate payload
        for local in saved_locals {
            fields.push(match local.cg_ty {
                CgTy::Ref | CgTy::String => gc_i8_ptr_ty.into(),
                CgTy::Bool | CgTy::Float64 | CgTy::Float32 | CgTy::Int(_) => i64_ty.into(),
                _ => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "callee suspend local type",
                        at: at.into(),
                    });
                }
            });
        }
        ty.set_body(&fields, false);
        Ok(ty)
    }

    pub(super) fn callee_suspend_trace_start_offset(
        &self,
        state_ty: inkwell::types::StructType<'ctx>,
    ) -> u64 {
        let size_bytes = self.target_data.get_store_size(&state_ty);
        self.target_data
            .offset_of_element(&state_ty, 2)
            .unwrap_or(size_bytes)
    }

    /// 在"最近 handler boundary"存在时跳转到 catch；否则返回默认值向外传播。
    ///
    /// 用途：
    /// - 普通函数调用返回后：callee 可能执行 `Raise.raise`，因此返回后需要检查 flag 并决定是否 unwind。
    pub(super) fn emit_effect_unwind_if_active(
        &mut self,
        at: crate::span::Span,
    ) -> Result<(), LlvmEmitError> {
        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: at.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: at.into(),
            })?;

        let cont_bb = self.context.append_basic_block(func, "effect_unwind_cont");
        let is_active = self.emit_effect_is_active_i1(at)?;

        if let Some(target) = self.current_raise_target() {
            self.builder
                .build_conditional_branch(is_active, target, cont_bb)?;
        } else {
            let ret_bb = self
                .context
                .append_basic_block(func, "effect_unwind_return");
            self.builder
                .build_conditional_branch(is_active, ret_bb, cont_bb)?;

            self.builder.position_at_end(ret_bb);
            let ret_ty = self
                .current_fun_return_ty
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "effect unwind needs function return type",
                    at: at.into(),
                })?;
            let v = self.default_value(at, ret_ty)?;
            self.emit_return(at, ret_ty, v)?;
        }

        self.builder.position_at_end(cont_bb);
        Ok(())
    }

    /// T1606f-2: Save callee locals to a heap CalleeSuspendState before flag propagation return.
    ///
    /// Called from `codegen_perform_expr_nonresuming_single_payload` when the function is "suspendable"
    /// and there's no local handler — i.e., the perform will propagate through flag propagation.
    fn emit_callee_suspend_state_save(
        &mut self,
        at: crate::span::Span,
        ctx: &CalleeSuspendSaveCtx,
    ) -> Result<(), LlvmEmitError> {
        let i64_ty = self.context.i64_type();
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();

        // Build (or reuse) the CalleeSuspendState struct type.
        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: at.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: at.into(),
            })?;
        let func_name = func.get_name().to_str().unwrap_or("anon");
        let func_name_san = sanitize_llvm_ident(func_name);
        let state_ty_name = format!("scoop.runtime.CalleeSuspendState__{func_name_san}");
        let state_ty =
            self.get_or_create_callee_suspend_state_type(at, &state_ty_name, &ctx.saved_locals)?;

        // Create type descriptor for GC.
        let size_bytes = self.target_data.get_store_size(&state_ty);
        let trace_start = self.callee_suspend_trace_start_offset(state_ty);
        let desc_name = format!("__scoop_type_desc_callee_suspend__{func_name_san}");
        let state_desc = self.get_or_create_type_descriptor_global(TypeDescriptorSpec {
            at,
            global_name: &desc_name,
            canonical_name: &state_ty_name,
            obj_ty: state_ty,
            trace_start_offset_bytes: trace_start,
            parent: None,
            itable: None,
            vtable: None,
        })?;

        // Allocate CalleeSuspendState.
        let rt_alloc = self.declare_runtime_alloc_typed();
        let total_size = i64_ty.const_int(size_bytes, false);
        let desc_i8 = self.builder.build_pointer_cast(
            state_desc.as_pointer_value(),
            i8_ptr_ty,
            "callee_state_desc_i8",
        )?;
        let alloc_call = self.builder.build_call(
            rt_alloc,
            &[desc_i8.into(), total_size.into()],
            "callee_state_alloc",
        )?;
        let state_raw = alloc_call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "callee state alloc return",
                at: at.into(),
            })?
            .into_pointer_value();

        // PIN the state to prevent GC from moving/collecting it.
        let pin = self.declare_runtime_gc_pin();
        let _ = self
            .builder
            .build_call(pin, &[state_raw.into()], "callee_state_pin")?;

        let state_ptr_ty = self.context.ptr_type(self.gc_address_space());
        let state_ptr =
            self.builder
                .build_pointer_cast(state_raw, state_ptr_ty, "callee_state_ptr")?;

        // Zero-initialize resume_word (field 1).
        let rw_ptr =
            self.builder
                .build_struct_gep(state_ty, state_ptr, 1, "callee_state_rw_gep")?;
        let _ = self.builder.build_store(rw_ptr, i64_ty.const_zero())?;
        let rg_ptr =
            self.builder
                .build_struct_gep(state_ty, state_ptr, 2, "callee_state_rg_gep")?;
        let _ = self
            .builder
            .build_store(rg_ptr, gc_i8_ptr_ty.const_null())?;

        // Save locals to state.
        for (idx, local_info) in ctx.saved_locals.iter().enumerate() {
            let field_idx = Self::callee_suspend_first_local_field_index() + idx as u32;
            let field_ptr = self.builder.build_struct_gep(
                state_ty,
                state_ptr,
                field_idx,
                &format!("callee_save_{}", local_info.name),
            )?;

            let local = self
                .env
                .get(local_info.id)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "callee suspend save: local not found",
                    at: at.into(),
                })?;

            match local_info.cg_ty {
                CgTy::Bool | CgTy::Float64 | CgTy::Float32 | CgTy::Int(_) => {
                    let llvm_ty = self.llvm_basic_type_of(at, local_info.cg_ty)?;
                    let loaded =
                        self.builder
                            .build_load(llvm_ty, local.ptr, "callee_save_scalar")?;
                    let encoded = self.coerce_u64_word(
                        at,
                        CgValue {
                            ty: local_info.cg_ty,
                            value: Some(loaded),
                        },
                    )?;
                    let _ = self.builder.build_store(field_ptr, encoded)?;
                }
                CgTy::Ref => {
                    let llvm_ty = self.llvm_basic_type_of(at, CgTy::Ref)?;
                    let loaded = self
                        .builder
                        .build_load(llvm_ty, local.ptr, "callee_save_ref")?
                        .into_pointer_value();
                    let casted = self.builder.build_pointer_cast(
                        loaded,
                        gc_i8_ptr_ty,
                        "callee_save_ref_i8",
                    )?;
                    let _ = self.store_local_value(
                        at,
                        field_ptr,
                        CgTy::Ref,
                        CgValue {
                            ty: CgTy::Ref,
                            value: Some(casted.into()),
                        },
                    )?;
                }
                CgTy::String => {
                    let llvm_ty = self.llvm_basic_type_of(at, CgTy::String)?;
                    let loaded = self
                        .builder
                        .build_load(llvm_ty, local.ptr, "callee_save_str")?
                        .into_pointer_value();
                    let casted = self.builder.build_pointer_cast(
                        loaded,
                        gc_i8_ptr_ty,
                        "callee_save_str_i8",
                    )?;
                    let _ = self.store_local_value(
                        at,
                        field_ptr,
                        CgTy::Ref,
                        CgValue {
                            ty: CgTy::Ref,
                            value: Some(casted.into()),
                        },
                    )?;
                }
                _ => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "callee suspend save: unsupported local type",
                        at: at.into(),
                    });
                }
            }
        }

        // Store state pointer to TLS (cast GC ptr to plain ptr for C ABI).
        let rt_set = self.declare_runtime_callee_suspend_state_set();
        let state_raw_plain = self.builder.build_address_space_cast(
            state_raw,
            i8_ptr_ty,
            "callee_state_raw_plain",
        )?;
        let _ = self
            .builder
            .build_call(rt_set, &[state_raw_plain.into()], "callee_suspend_set")?;

        Ok(())
    }

    pub(super) fn fun_ty_effects_is_pure(&self, ty: TypeId) -> Option<bool> {
        match self.types.kind(ty) {
            TypeKind::Ref(RefTypeKind::Function(fun_ty)) => Some(fun_ty.effects.is_pure()),
            _ => None,
        }
    }

    pub(super) fn expr_may_perform(&self, expr: &hir::Expr) -> bool {
        match &expr.kind {
            hir::ExprKind::Missing
            | hir::ExprKind::Literal(_)
            | hir::ExprKind::VarRef(_)
            | hir::ExprKind::UnresolvedIdent { .. } => false,

            hir::ExprKind::StructLit { fields, .. } => {
                fields.iter().any(|f| self.expr_may_perform(&f.value))
            }
            hir::ExprKind::TupleLit { elements } => {
                elements.iter().any(|e| self.expr_may_perform(e))
            }
            hir::ExprKind::InterpolatedString { parts, .. } => parts.iter().any(|p| match p {
                hir::InterpolatedStringPart::Text { .. } => false,
                hir::InterpolatedStringPart::Expr { expr } => self.expr_may_perform(expr),
            }),

            hir::ExprKind::Unary { expr: inner, .. } => self.expr_may_perform(inner),
            hir::ExprKind::Binary { lhs, rhs, .. } => {
                self.expr_may_perform(lhs) || self.expr_may_perform(rhs)
            }
            hir::ExprKind::TypeCheck { expr: inner, .. } => self.expr_may_perform(inner),

            // `as` 失败会走 `Raise.raise(RuntimeError.ClassCastFailed)` 的语义落点，因此视为 perform 点；
            // `as?` 不会 raise（失败返回 None），仅递归检查 operand。
            hir::ExprKind::Cast {
                expr: inner, op, ..
            } => match op {
                ast::CastOp::As => true,
                ast::CastOp::AsQ => self.expr_may_perform(inner),
            },

            hir::ExprKind::Block(block) => self.block_may_perform(block),
            hir::ExprKind::Closure(_) => false,

            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.expr_may_perform(cond)
                    || self.expr_may_perform(then_branch)
                    || else_branch
                        .as_ref()
                        .is_some_and(|e| self.expr_may_perform(e))
            }

            hir::ExprKind::When { subject, arms } => {
                if self.expr_may_perform(subject) {
                    return true;
                }
                for arm in arms {
                    if arm.guard.as_ref().is_some_and(|g| self.expr_may_perform(g)) {
                        return true;
                    }
                    if self.expr_may_perform(&arm.body) {
                        return true;
                    }
                }
                false
            }

            // member access 本身不 perform，但 receiver 的求值可能 perform。
            hir::ExprKind::MemberAccess { receiver, .. } => self.expr_may_perform(receiver),

            hir::ExprKind::Call { callee, args } => {
                // 实参求值可能包含 perform。
                for arg in args {
                    match arg {
                        hir::CallArg::Positional(e) => {
                            if self.expr_may_perform(e) {
                                return true;
                            }
                        }
                        hir::CallArg::Named { value, .. } => {
                            if self.expr_may_perform(value) {
                                return true;
                            }
                        }
                    }
                }

                // callee 若是已知顶层函数/方法且 effects 为 Pure，则调用点本身不会触发 flag-based unwinding；
                // 其它 callee（closure/local/未解析）先按"可能 perform"保守处理，避免误删 handler。
                let Some(fqn) = self.try_extract_callee_fqn(callee) else {
                    return true;
                };
                let Some(fun) = self.fun_index.get(fqn).copied() else {
                    return true;
                };
                self.fun_ty_effects_is_pure(fun.ty)
                    .map(|pure| !pure)
                    .unwrap_or(true)
            }

            // `perform`/`handle`：直接视为会触发 effect 机制（或其内部可能触发）。
            hir::ExprKind::Perform { .. } => true,
            hir::ExprKind::Handle(_) => true,

            hir::ExprKind::Todo(_) => true,
        }
    }

    pub(super) fn try_extract_callee_fqn<'b>(&self, callee: &'b hir::Expr) -> Option<&'b str> {
        match &callee.kind {
            hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) => Some(fqn.as_str()),
            hir::ExprKind::MemberAccess { member, .. } => match member.resolved.as_ref()? {
                hir::MemberRef::Fun { fqn, .. } => Some(fqn.as_str()),
                hir::MemberRef::ExtensionFun { fqn, .. } => Some(fqn.as_str()),
                _ => None,
            },
            _ => None,
        }
    }

    pub(super) fn block_may_perform(&self, block: &hir::Block) -> bool {
        for stmt in &block.stmts {
            if self.stmt_may_perform(stmt) {
                return true;
            }
        }
        false
    }

    pub(super) fn stmt_may_perform(&self, stmt: &hir::Stmt) -> bool {
        match &stmt.kind {
            hir::StmtKind::Empty => false,
            hir::StmtKind::Expr(expr) => self.expr_may_perform(expr),
            hir::StmtKind::Val(decl) => {
                decl.init.as_ref().is_some_and(|e| self.expr_may_perform(e))
            }
            hir::StmtKind::Assign { lhs, rhs, .. } => {
                self.expr_may_perform(lhs) || self.expr_may_perform(rhs)
            }
            hir::StmtKind::While { cond, body } => {
                self.expr_may_perform(cond) || self.block_may_perform(body)
            }
            // 当前阶段这些语句在 block expression 中不支持；为避免误删 handler，这里保守视为可能 perform。
            hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Return { .. }
            | hir::StmtKind::Todo(_) => true,
        }
    }

    pub(super) fn effect_trace_line_col(
        &self,
        at: crate::span::Span,
    ) -> Result<(u32, u32), LlvmEmitError> {
        // 注意：当前阶段 HIR span 仍是"无 file-id 的 byte offsets"，当 codegen 生成跨文件函数体
        //（例如 stdlib/helper 被内联为可 codegen 的顶层函数）时，span 可能不属于入口文件。
        //
        // T0150b 只把后端的持有形态切到 `SourceMap`，还没有让 span 自带 file-id；
        // 因此这里继续保留“先按入口文件回退”的旧语义。无法映射时降级为 (0, 0)：
        // - 不影响 non-resuming effect 的语义（仍由 flag+slot 决定）；
        // - fixtures 可选择性断言：对入口文件的 raise/perform，line/col 仍可稳定；
        // - 未来当 span 携带 file-id 后，再把这里升级为精确映射。
        let Ok((line, col)) = self.entry_source().offset_to_line_col(at.start) else {
            return Ok((0, 0));
        };
        let line_u32 = line.min(u32::MAX as usize) as u32;
        let col_u32 = col.min(u32::MAX as usize) as u32;
        Ok((line_u32, col_u32))
    }

    /// 将 `Raise.raise(error)` 的 `error` 值编码为 runtime perform slot 的 payload words。
    ///
    /// 当前阶段（T0818）的目标是先把 `Raise<RuntimeError>` 跑通，以支持：
    /// - `x!!` / `x as T` 等"运行期失败 → Raise<RuntimeError>"的语义落点；
    /// - `try/catch` 能读回并匹配 `RuntimeError` 的 unit variants。
    ///
    /// ABI（TODO T0630）：
    /// - payload 使用 2 个 word：`(kind, value)`
    ///   - `kind`：判别信息（union 风格），便于在 handler 边界做断言/调试
    ///   - `value`：实际载荷（按 u64 编码）
    pub(super) fn codegen_raise_error_payload_words(
        &mut self,
        err_expr: &hir::Expr,
    ) -> Result<(IntValue<'ctx>, IntValue<'ctx>), LlvmEmitError> {
        // slot 的 word 固定为 u64（runtime ABI，T0630）。
        let u64_ty = self.context.i64_type();
        let from_u64 = IntTy {
            bits: 64,
            signed: false,
        };

        // payload.kind（用于 union 风格判别；0 表示未初始化）。
        const KIND_INT: u64 = 1;
        const KIND_RUNTIME_ERROR: u64 = 2;

        // 注意：HIR 在早期阶段并不总是为每个表达式标注精确类型（例如 member access 常为 `Any`），
        // 因此这里以 codegen 后的 `CgValue.ty` 为准（避免过度依赖 `hir::Expr.ty`）。
        let err_v = self.codegen_expr(err_expr)?;

        match err_v.ty {
            CgTy::Int(from_ty) => {
                // 整数族：把值编码进 slot 的 u64。
                let (err_raw, _) = err_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "Raise.raise arg value",
                    at: err_expr.span.into(),
                })?;
                let kind = u64_ty.const_int(KIND_INT, false);
                let value = self.cast_int(err_raw, from_ty, from_u64)?;
                Ok((kind, value))
            }
            CgTy::Enum(enum_ty) if self.is_sysroot_runtime_error_enum(enum_ty) => {
                // `RuntimeError`：写入 tag（u32）到 slot（u64）。
                //
                // 注意：当前 `RuntimeError` 的 enum 表示是 tagged union `{ tag: i32, payload: word }`，
                // 其中 payload 为空（unit variants），因此只需要写回 tag 即可。
                let repr = self.cg_enum_layout(err_expr.span, enum_ty)?.repr;
                if !matches!(repr, CgEnumRepr::TaggedUnion) {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Raise<RuntimeError> niche repr (not supported)",
                        at: err_expr.span.into(),
                    });
                }

                let raw = err_v.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "Raise.raise arg value",
                    at: err_expr.span.into(),
                })?;
                let enum_v = raw.into_struct_value();
                let extracted =
                    self.builder
                        .build_extract_value(enum_v, 0, "raise_runtime_error_tag")?;
                let BasicValueEnum::IntValue(tag_i32) = extracted else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Raise<RuntimeError> tag value",
                        at: err_expr.span.into(),
                    });
                };
                let kind = u64_ty.const_int(KIND_RUNTIME_ERROR, false);
                let value = self.builder.build_int_z_extend(
                    tag_i32,
                    u64_ty,
                    "raise_runtime_error_tag_u64",
                )?;
                Ok((kind, value))
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Raise.raise arg type (payload encoding)",
                at: err_expr.span.into(),
            }),
        }
    }

    /// 判断一个 value nominal type 是否是 sysroot 内建的 `scoop.core.RuntimeError`。
    ///
    /// 说明：T0818 只要求打通 `Raise<RuntimeError>`；其它 `Raise<E>` 的复杂 payload ABI 留给 T0630。
    pub(super) fn is_sysroot_runtime_error_enum(&self, ty: TypeId) -> bool {
        matches!(
            self.types.kind(ty),
            TypeKind::Value(ValueTypeKind::Nominal(nominal))
                if nominal.fqn == "scoop.core.RuntimeError"
        )
    }

    pub(super) fn runtime_error_variant_tag(
        &self,
        at: crate::span::Span,
        variant: &str,
    ) -> Result<u64, LlvmEmitError> {
        let layout = self.enum_layouts.get("scoop.core.RuntimeError").ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "RuntimeError enum layout",
                at: at.into(),
            },
        )?;
        let v = layout.variants.iter().find(|v| v.name == variant).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "RuntimeError variant",
                at: at.into(),
            },
        )?;
        Ok(v.tag)
    }

    pub(super) fn emit_raise_runtime_error_variant(
        &mut self,
        at: crate::span::Span,
        variant: &str,
    ) -> Result<(), LlvmEmitError> {
        let tag = self.runtime_error_variant_tag(at, variant)?;
        self.emit_raise_runtime_error_tag(at, tag)
    }

    pub(super) fn emit_raise_runtime_error_tag(
        &mut self,
        span: crate::span::Span,
        tag: u64,
    ) -> Result<(), LlvmEmitError> {
        // 说明：复用 `Raise.raise(RuntimeError.X)` 的最小 ABI 约定（T0818），但避免在这里构造 HIR 节点：
        // - slot: (op_tag=Raise, payload_kind=RuntimeError, payload_value=tag)
        // - set flag 并携带 line/col trace
        const PAYLOAD_KIND_RUNTIME_ERROR: u64 = 2;

        let i32_ty = self.context.i32_type();
        let u64_ty = self.context.i64_type();

        let raise_tag = self.effect_op_tag("scoop.core.Raise.raise");
        let op_tag_i32 = i32_ty.const_int(raise_tag as u64, false);
        let payload_kind_u64 = u64_ty.const_int(PAYLOAD_KIND_RUNTIME_ERROR, false);
        let payload_value_u64 = u64_ty.const_int(tag, false);

        let rt_write = self.declare_runtime_effect_perform_slot_write_u64_2();
        let _ = self.builder.build_call(
            rt_write,
            &[
                op_tag_i32.into(),
                payload_kind_u64.into(),
                payload_value_u64.into(),
            ],
            "runtime_error_write_slot",
        )?;

        let (src_line, src_col) = self.effect_trace_line_col(span)?;
        let src_line_i32 = i32_ty.const_int(src_line as u64, false);
        let src_col_i32 = i32_ty.const_int(src_col as u64, false);

        let rt_set = self.declare_runtime_effect_set_active_with_trace();
        let _ = self.builder.build_call(
            rt_set,
            &[src_line_i32.into(), src_col_i32.into()],
            "runtime_error_set_active",
        )?;

        // 早退：若存在 handler boundary，跳到 catch；否则返回默认值向外传播。
        if let Some(target) = self.current_raise_target() {
            self.builder.build_unconditional_branch(target)?;
        } else {
            let ret_ty = self
                .current_fun_return_ty
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "Raise<RuntimeError> needs function return type",
                    at: span.into(),
                })?;
            let v = self.default_value(span, ret_ty)?;
            self.emit_return(span, ret_ty, v)?;
        }

        // 继续生成后续 IR：把 builder 移到一个"不可达 continuation block"，避免后续插入失败。
        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;
        let dead = self.context.append_basic_block(func, "after_raise_dead");
        self.builder.position_at_end(dead);
        Ok(())
    }

    pub(super) fn coerce_u64_word(
        &mut self,
        at: crate::span::Span,
        value: CgValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        // 将一个可表示为 "word-sized u64 payload" 的值转换为 `i64`（在 ABI 层作为 `uint64_t` 使用）。
        //
        // 注意：这里不引入额外的 tag/布局；更复杂的 payload 由 TODO T0630 扩展。
        let i64_ty = self.context.i64_type();
        match value.ty {
            CgTy::Unit | CgTy::Never => Ok(i64_ty.const_int(0, false)),
            CgTy::Bool => {
                let b = value.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "u64 word from bool",
                    at: at.into(),
                })?;
                Ok(self.builder.build_int_z_extend(b, i64_ty, "bool_to_u64")?)
            }
            CgTy::Int(_) => {
                let (raw, from) = value.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "u64 word from int",
                    at: at.into(),
                })?;
                let to = IntTy {
                    bits: 64,
                    signed: false,
                };
                Ok(self.cast_int(raw, from, to)?)
            }
            CgTy::Float64 => {
                let (raw, _) = value.as_float().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "u64 word from float64",
                    at: at.into(),
                })?;
                Ok(self
                    .builder
                    .build_bit_cast(raw, i64_ty, "f64_to_u64_bits")?
                    .into_int_value())
            }
            CgTy::Float32 => {
                let (raw, _) = value.as_float().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "u64 word from float32",
                    at: at.into(),
                })?;
                let bits32 = self
                    .builder
                    .build_bit_cast(raw, self.context.i32_type(), "f32_to_u32_bits")?
                    .into_int_value();
                Ok(self
                    .builder
                    .build_int_z_extend(bits32, i64_ty, "u32_to_u64_bits")?)
            }
            CgTy::String | CgTy::Ref => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "u64 word from gc pointer (ptr<->int is forbidden)",
                at: at.into(),
            }),
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "u64 word from composite value",
                    at: at.into(),
                })
            }
        }
    }

    pub(super) fn codegen_sysroot_effect_intrinsics(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let value_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        };
        let _handle_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: false,
        };

        match fqn {
            "scoop.core.__scoop_effect_is_active" => {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect is_active arity mismatch",
                        at: span.into(),
                    });
                }

                let rt = self.declare_runtime_effect_is_active();
                let call = self.builder.build_call(rt, &[], "effect_is_active")?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "effect is_active return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(raw_int) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect is_active return type",
                        at: span.into(),
                    });
                };

                let from = IntTy {
                    bits: 32,
                    signed: false,
                };
                let casted = self.cast_int(raw_int, from, value_word)?;
                Ok(CgValue::int(casted, value_word))
            }
            "scoop.core.__scoop_effect_set_active" => {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect set_active arity mismatch",
                        at: span.into(),
                    });
                }

                let rt = self.declare_runtime_effect_set_active();
                let _ = self.builder.build_call(rt, &[], "effect_set_active")?;
                Ok(CgValue::unit())
            }
            "scoop.core.__scoop_effect_clear" => {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect clear arity mismatch",
                        at: span.into(),
                    });
                }

                let rt = self.declare_runtime_effect_clear();
                let _ = self.builder.build_call(rt, &[], "effect_clear")?;
                Ok(CgValue::unit())
            }
            "scoop.core.__scoop_effect_slot_write" => {
                if args.len() != 2 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write arity mismatch",
                        at: span.into(),
                    });
                }

                let hir::CallArg::Positional(tag_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write tag named arg",
                        at: span.into(),
                    });
                };
                let hir::CallArg::Positional(value_expr) = &args[1] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write value named arg",
                        at: span.into(),
                    });
                };

                let tag_v =
                    self.codegen_expr_in_expected_context(tag_expr, Some(CgTy::Int(value_word)))?;
                let tag_v = self.coerce_value(tag_expr.span, tag_v, CgTy::Int(value_word))?;
                let (tag_raw, tag_from) =
                    tag_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write tag value",
                        at: tag_expr.span.into(),
                    })?;
                let tag_to = IntTy {
                    bits: 32,
                    signed: false,
                };
                let tag_i32 = self.cast_int(tag_raw, tag_from, tag_to)?;

                let value_v =
                    self.codegen_expr_in_expected_context(value_expr, Some(CgTy::Int(value_word)))?;
                let value_v = self.coerce_value(value_expr.span, value_v, CgTy::Int(value_word))?;
                let (value_raw, value_from) =
                    value_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write value",
                        at: value_expr.span.into(),
                    })?;
                let value_to = IntTy {
                    bits: 64,
                    signed: false,
                };
                let value_i64 = self.cast_int(value_raw, value_from, value_to)?;

                let rt = self.declare_runtime_effect_perform_slot_write_u64();
                let _ = self.builder.build_call(
                    rt,
                    &[tag_i32.into(), value_i64.into()],
                    "effect_slot_write",
                )?;
                Ok(CgValue::unit())
            }
            "scoop.core.__scoop_effect_slot_write2" => {
                if args.len() != 3 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write2 arity mismatch",
                        at: span.into(),
                    });
                }

                let hir::CallArg::Positional(tag_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write2 tag named arg",
                        at: span.into(),
                    });
                };
                let hir::CallArg::Positional(word0_expr) = &args[1] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write2 word0 named arg",
                        at: span.into(),
                    });
                };
                let hir::CallArg::Positional(word1_expr) = &args[2] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write2 word1 named arg",
                        at: span.into(),
                    });
                };

                let tag_v =
                    self.codegen_expr_in_expected_context(tag_expr, Some(CgTy::Int(value_word)))?;
                let tag_v = self.coerce_value(tag_expr.span, tag_v, CgTy::Int(value_word))?;
                let (tag_raw, tag_from) =
                    tag_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write2 tag value",
                        at: tag_expr.span.into(),
                    })?;
                let tag_to = IntTy {
                    bits: 32,
                    signed: false,
                };
                let tag_i32 = self.cast_int(tag_raw, tag_from, tag_to)?;

                let word0_v =
                    self.codegen_expr_in_expected_context(word0_expr, Some(CgTy::Int(value_word)))?;
                let word0_v = self.coerce_value(word0_expr.span, word0_v, CgTy::Int(value_word))?;
                let (word0_raw, word0_from) =
                    word0_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write2 word0 value",
                        at: word0_expr.span.into(),
                    })?;

                let word1_v =
                    self.codegen_expr_in_expected_context(word1_expr, Some(CgTy::Int(value_word)))?;
                let word1_v = self.coerce_value(word1_expr.span, word1_v, CgTy::Int(value_word))?;
                let (word1_raw, word1_from) =
                    word1_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write2 word1 value",
                        at: word1_expr.span.into(),
                    })?;

                let word_to = IntTy {
                    bits: 64,
                    signed: false,
                };
                let word0_i64 = self.cast_int(word0_raw, word0_from, word_to)?;
                let word1_i64 = self.cast_int(word1_raw, word1_from, word_to)?;

                let rt = self.declare_runtime_effect_perform_slot_write_u64_2();
                let _ = self.builder.build_call(
                    rt,
                    &[tag_i32.into(), word0_i64.into(), word1_i64.into()],
                    "effect_slot_write2",
                )?;
                Ok(CgValue::unit())
            }
            "scoop.core.__scoop_effect_slot_read_op_tag" => {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_op_tag arity mismatch",
                        at: span.into(),
                    });
                }

                let rt = self.declare_runtime_effect_perform_slot_read_op_tag();
                let call = self
                    .builder
                    .build_call(rt, &[], "effect_slot_read_op_tag")?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_op_tag return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(raw_int) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_op_tag return type",
                        at: span.into(),
                    });
                };

                let from = IntTy {
                    bits: 32,
                    signed: false,
                };
                let casted = self.cast_int(raw_int, from, value_word)?;
                Ok(CgValue::int(casted, value_word))
            }
            "scoop.core.__scoop_effect_slot_read_len_words" => {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_len_words arity mismatch",
                        at: span.into(),
                    });
                }

                let rt = self.declare_runtime_effect_perform_slot_read_len_words();
                let call = self
                    .builder
                    .build_call(rt, &[], "effect_slot_read_len_words")?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_len_words return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(raw_int) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_len_words return type",
                        at: span.into(),
                    });
                };

                let from = IntTy {
                    bits: 32,
                    signed: false,
                };
                let casted = self.cast_int(raw_int, from, value_word)?;
                Ok(CgValue::int(casted, value_word))
            }
            "scoop.core.__scoop_effect_slot_read_value" => {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_value arity mismatch",
                        at: span.into(),
                    });
                }

                let rt = self.declare_runtime_effect_perform_slot_read_u64();
                let call = self.builder.build_call(rt, &[], "effect_slot_read_u64")?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_value return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(raw_int) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_value return type",
                        at: span.into(),
                    });
                };

                let from = IntTy {
                    bits: 64,
                    signed: false,
                };
                let casted = self.cast_int(raw_int, from, value_word)?;
                Ok(CgValue::int(casted, value_word))
            }
            "scoop.core.__scoop_effect_slot_read_word" => {
                if args.len() != 1 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_word arity mismatch",
                        at: span.into(),
                    });
                }

                let hir::CallArg::Positional(index_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_word index named arg",
                        at: span.into(),
                    });
                };

                let index_v =
                    self.codegen_expr_in_expected_context(index_expr, Some(CgTy::Int(value_word)))?;
                let index_v = self.coerce_value(index_expr.span, index_v, CgTy::Int(value_word))?;
                let (index_raw, index_from) =
                    index_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_word index value",
                        at: index_expr.span.into(),
                    })?;
                let index_to = IntTy {
                    bits: 32,
                    signed: false,
                };
                let index_i32 = self.cast_int(index_raw, index_from, index_to)?;

                let rt = self.declare_runtime_effect_perform_slot_read_u64_at();
                let call = self.builder.build_call(
                    rt,
                    &[index_i32.into()],
                    "effect_slot_read_word_u64",
                )?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_word return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(raw_int) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_word return type",
                        at: span.into(),
                    });
                };

                let from = IntTy {
                    bits: 64,
                    signed: false,
                };
                let casted = self.cast_int(raw_int, from, value_word)?;
                Ok(CgValue::int(casted, value_word))
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "unknown sysroot effect intrinsic callee",
                at: callee_span.into(),
            }),
        }
    }
}
