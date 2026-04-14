impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    fn codegen_handle_expr_unified_stack_reentry_only_multi_resuming_leaf<'hir>(
        &mut self,
        span: crate::span::Span,
        handle: &'hir hir::HandleExpr,
        state_machine_plan: &HandleStateMachinePlan,
        immediate_arms: &[(&'hir hir::HandleArm, hir::SymbolId)],
        sibling_nonresuming_arms: &[&'hir hir::HandleArm],
        out_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        #[derive(Debug)]
        struct MultipleImmediateSitePlan<'hir, 'ctx> {
            site: ImmediateResumeSite<'hir>,
            arm: &'hir hir::HandleArm,
            resume_symbol: hir::SymbolId,
            resume_value_ty: CgTy,
            binder_slots: Vec<ImmediateResumeBinderSlot<'ctx>>,
            resume_used_ptr: PointerValue<'ctx>,
            resume_value_ptr: Option<PointerValue<'ctx>>,
            target_ptr: PointerValue<'ctx>,
            arm_bb: inkwell::basic_block::BasicBlock<'ctx>,
        }

        #[derive(Clone, Copy)]
        struct MultipleImmediateCustomSibling<'hir, 'ctx> {
            arm: &'hir hir::HandleArm,
            frame_ptr: PointerValue<'ctx>,
            catch_bb: inkwell::basic_block::BasicBlock<'ctx>,
            op_tag: u32,
        }

        let immediate_arm_plans =
            Self::build_multi_resuming_immediate_arm_plans(handle, immediate_arms)?;
        let resolved_sites = Self::resolve_multi_resuming_immediate_sites_from_plan(
            handle,
            state_machine_plan,
            immediate_arm_plans.as_slice(),
        )?;

        let sibling_plan = self.collect_sibling_nonresuming_plan(sibling_nonresuming_arms)?;
        let raise_sibling = sibling_plan.raise_arm;

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
        let outer_raise_target = self.current_raise_target();

        let i32_ty = self.context.i32_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let handler_frame_ty = self.llvm_effect_handler_frame_type();

        let dispatch_bb = self
            .context
            .append_basic_block(func, "handle_multi_resume_dispatch");
        let mut state_bbs: Vec<inkwell::basic_block::BasicBlock<'ctx>> = Vec::new();
        for idx in 0..=resolved_sites.len() {
            state_bbs.push(
                self.context
                    .append_basic_block(func, &format!("handle_multi_resume_state_{idx}")),
            );
        }
        let done_bb = self
            .context
            .append_basic_block(func, "handle_multi_resume_done");
        let bad_state_bb = self
            .context
            .append_basic_block(func, "handle_multi_resume_bad_state");
        let finally_bb = self
            .context
            .append_basic_block(func, "handle_multi_resume_finally");
        let finally_unwind_bb = self
            .context
            .append_basic_block(func, "handle_multi_resume_finally_unwind");
        let sibling_dispatch =
            self.build_sibling_nonresuming_dispatch_blocks(func, "handle_multi_resume", &sibling_plan);
        let effect_dispatch_bb = sibling_dispatch.effect_dispatch_bb;
        let effect_dispatch_nomatch_bb = sibling_dispatch.effect_dispatch_nomatch_bb;
        let raise_catch_bb = sibling_dispatch.raise_catch_bb;

        let state_ptr =
            self.create_entry_alloca_raw(span, "handle_multi_resume_state", i32_ty.into())?;
        let result_ptr = if out_ty == CgTy::Unit {
            None
        } else {
            Some(self.create_entry_alloca(span, "handle_multi_resume_result", out_ty)?)
        };

        let mut site_plans: Vec<MultipleImmediateSitePlan<'hir, 'ctx>> = Vec::new();
        for (site_idx, resolved) in resolved_sites.into_iter().enumerate() {
            let site = resolved.site;
            let arm = resolved.arm.arm;
            let resume_symbol = resolved.arm.resume_symbol;
            if !site.resume_path.is_empty() {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed immediate-resume body (only top-level val-bound direct perform supported)",
                    at: site.decl.span.into(),
                });
            }
            let resume_value_ty =
                self.cg_ty_of(site.decl.ty)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle resume perform value type",
                        at: site.decl.span.into(),
                    })?;
            if arm.op.binders.len() != site.args.len() {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle resume binder arity mismatch",
                    at: arm.op.span.into(),
                });
            }

            let target_ptr = self.create_entry_alloca(
                site.decl.span,
                site.decl
                    .name
                    .as_deref()
                    .unwrap_or("handle_multi_resume_value"),
                resume_value_ty,
            )?;
            let resume_used_ptr = self.create_entry_alloca_raw(
                span,
                &format!("handle_multi_resume_used_{site_idx}"),
                self.context.bool_type().into(),
            )?;
            let resume_value_ptr = if resume_value_ty == CgTy::Unit {
                None
            } else {
                Some(self.create_entry_alloca(
                    span,
                    &format!("handle_multi_resume_resume_value_{site_idx}"),
                    resume_value_ty,
                )?)
            };
            let arm_bb = self
                .context
                .append_basic_block(func, &format!("handle_multi_resume_arm_{site_idx}"));

            let mut binder_slots: Vec<ImmediateResumeBinderSlot<'ctx>> = Vec::new();
            for binder in &arm.op.binders {
                let binder_ty =
                    self.cg_ty_of(binder.ty)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle resume binder type",
                            at: binder.span.into(),
                        })?;
                let ptr = self.create_entry_alloca(binder.span, &binder.name, binder_ty)?;
                binder_slots.push(ImmediateResumeBinderSlot {
                    id: binder.id,
                    hir_ty: binder.ty,
                    ty: binder_ty,
                    ptr,
                });
            }

            site_plans.push(MultipleImmediateSitePlan {
                site,
                arm,
                resume_symbol,
                resume_value_ty,
                binder_slots,
                resume_used_ptr,
                resume_value_ptr,
                target_ptr,
                arm_bb,
            });
        }

        let mut custom_siblings: Vec<MultipleImmediateCustomSibling<'hir, 'ctx>> = Vec::new();
        for (idx, custom) in sibling_plan.custom_arms.iter().enumerate() {
            let frame_ptr = self.create_entry_alloca_raw(
                span,
                &format!("handle_multi_resume_custom_frame_{idx}"),
                handler_frame_ty.into(),
            )?;
            let catch_bb = sibling_dispatch.custom_catch_bbs[idx];
            custom_siblings.push(MultipleImmediateCustomSibling {
                arm: custom.arm,
                frame_ptr,
                catch_bb,
                op_tag: custom.op_tag,
            });
        }

        let _ = self.builder.build_store(state_ptr, i32_ty.const_zero())?;
        for site_plan in &site_plans {
            let _ = self.builder.build_store(
                site_plan.resume_used_ptr,
                self.context.bool_type().const_zero(),
            )?;
        }

        let rt_push = self.declare_runtime_effect_handler_stack_push();
        for custom in &custom_siblings {
            let frame_i8 = self
                .builder
                .build_bit_cast(
                    custom.frame_ptr,
                    i8_ptr_ty,
                    "handle_multi_resume_custom_frame_i8",
                )?
                .into_pointer_value();
            let tag_i32 = i32_ty.const_int(custom.op_tag as u64, false);
            let _ = self.builder.build_call(
                rt_push,
                &[frame_i8.into(), tag_i32.into()],
                "handle_multi_resume_custom_push",
            )?;
        }

        let custom_outer_top = if let Some(first) = custom_siblings.first() {
            let prev_ptr = self.builder.build_struct_gep(
                handler_frame_ty,
                first.frame_ptr,
                0,
                "handle_multi_resume_custom_prev_gep",
            )?;
            Some(
                self.builder
                    .build_load(i8_ptr_ty, prev_ptr, "handle_multi_resume_custom_outer_top")?
                    .into_pointer_value(),
            )
        } else {
            None
        };
        let custom_restore_top = if let Some(last) = custom_siblings.last() {
            Some(
                self.builder
                    .build_bit_cast(
                        last.frame_ptr,
                        i8_ptr_ty,
                        "handle_multi_resume_custom_restore_top",
                    )?
                    .into_pointer_value(),
            )
        } else {
            None
        };
        let handler_exit = custom_outer_top
            .map(ImmediateResumeHandlerExit::SwapTop)
            .unwrap_or(ImmediateResumeHandlerExit::None);

        self.builder.build_unconditional_branch(dispatch_bb)?;

        self.builder.position_at_end(dispatch_bb);
        let state = self
            .builder
            .build_load(i32_ty, state_ptr, "handle_multi_resume_state")?
            .into_int_value();
        let mut state_cases: Vec<(IntValue<'ctx>, inkwell::basic_block::BasicBlock<'ctx>)> =
            Vec::new();
        for (idx, bb) in state_bbs.iter().enumerate() {
            state_cases.push((i32_ty.const_int(idx as u64, false), *bb));
        }
        self.builder.build_switch(state, bad_state_bb, &state_cases)?;

        self.builder.position_at_end(bad_state_bb);
        self.emit_exit_with_code(span, 3)?;

        self.env.push_scope();

        for (state_idx, state_bb) in state_bbs.iter().enumerate() {
            self.builder.position_at_end(*state_bb);

            if state_idx > 0 {
                let resumed_site = &site_plans[state_idx - 1];
                if let Some(ptr) = resumed_site.resume_value_ptr {
                    let llvm_ty = self.llvm_basic_type_of(
                        resumed_site.site.decl.span,
                        resumed_site.resume_value_ty,
                    )?;
                    let loaded = self
                        .builder
                        .build_load(llvm_ty, ptr, "handle_multi_resume_loaded_value")?;
                    let resumed_value = CgValue {
                        ty: resumed_site.resume_value_ty,
                        value: Some(loaded),
                    };
                    let _ = self.store_local_value(
                        resumed_site.site.decl.span,
                        resumed_site.target_ptr,
                        resumed_site.resume_value_ty,
                        resumed_value,
                    )?;
                }
            }

            for custom in &custom_siblings {
                self.push_effect_unwind_target(&custom.arm.op.op.fqn, custom.catch_bb);
            }
            self.push_raise_target(effect_dispatch_bb.unwrap_or(finally_unwind_bb));

            let start_idx = if state_idx == 0 {
                0
            } else {
                site_plans[state_idx - 1].site.top_level_stmt_idx + 1
            };
            let next_site = site_plans.get(state_idx);
            let mut value: CgValue<'ctx> = CgValue::unit();
            let mut intercepted = false;

            for (idx, stmt) in handle.body.stmts.iter().enumerate().skip(start_idx) {
                if let Some(site_plan) = next_site
                    && idx == site_plan.site.top_level_stmt_idx
                {
                    let _ = self.codegen_immediate_resume_site_binding(
                        &site_plan.site,
                        site_plan.site.decl,
                        ImmediateResumeArmDispatch {
                            binder_slots: &site_plan.binder_slots,
                            resume_used_ptr: site_plan.resume_used_ptr,
                            arm_bb: site_plan.arm_bb,
                        },
                        Some(site_plan.target_ptr),
                    )?;
                    intercepted = true;
                    break;
                }

                let is_last = idx + 1 == handle.body.stmts.len();
                match &stmt.kind {
                    hir::StmtKind::Empty => {}
                    hir::StmtKind::Val(decl) => {
                        self.codegen_val_decl(decl)?;
                        value = CgValue::unit();
                    }
                    hir::StmtKind::Assign { lhs, eq_span, rhs } => {
                        self.codegen_assign_stmt(*eq_span, lhs, rhs)?;
                        value = CgValue::unit();
                    }
                    hir::StmtKind::Expr(expr) => {
                        let expected = if is_last {
                            Some(out_ty)
                        } else {
                            Some(CgTy::Unit)
                        };
                        let v = self.codegen_expr_in_expected_context(expr, expected)?;
                        value = if is_last { v } else { CgValue::unit() };
                    }
                    hir::StmtKind::Return { .. } => {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "`return` inside handle resume body",
                            at: stmt.span.into(),
                        });
                    }
                    hir::StmtKind::While { .. }
                    | hir::StmtKind::Break { .. }
                    | hir::StmtKind::Continue { .. }
                    | hir::StmtKind::Todo(_) => {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "statement inside handle resume body",
                            at: stmt.span.into(),
                        });
                    }
                }
            }

            self.pop_raise_target();
            for _ in custom_siblings.iter().rev() {
                self.pop_effect_unwind_target();
            }

            if !intercepted {
                self.codegen_immediate_resume_finalize_body(
                    handle.body.span,
                    out_ty,
                    value,
                    result_ptr,
                    handler_exit,
                    finally_bb,
                )?;
            }
        }

        for (site_idx, site_plan) in site_plans.iter().enumerate() {
            self.builder.position_at_end(site_plan.arm_bb);

            if let Some(custom_outer_top) = custom_outer_top {
                let rt_swap = self.declare_runtime_effect_handler_stack_swap_top();
                let _ = self.builder.build_call(
                    rt_swap,
                    &[custom_outer_top.into()],
                    "handle_multi_resume_detach",
                )?;
            }

            self.env.push_scope();
            for slot in &site_plan.binder_slots {
                self.env.insert(
                    slot.id,
                    CgLocal {
                        hir_ty: Some(slot.hir_ty),
                        ty: slot.ty,
                        ptr: slot.ptr,
                        mutable: false,
                    },
                );
            }

            let resume_ctx = ImmediateResumeCtx {
                resume_symbol: site_plan.resume_symbol,
                resume_value_ty: site_plan.resume_value_ty,
                resume_value_ptr: site_plan.resume_value_ptr,
                resume_used_ptr: site_plan.resume_used_ptr,
                state_ptr,
                next_state: (site_idx + 1) as u32,
                _marker: std::marker::PhantomData,
            };
            self.push_immediate_resume_ctx(resume_ctx);
            for custom in &custom_siblings {
                self.push_effect_unwind_target(&custom.arm.op.op.fqn, finally_unwind_bb);
            }
            self.push_raise_target(finally_unwind_bb);
            let _ = self.codegen_expr_in_expected_context(&site_plan.arm.body, Some(CgTy::Unit))?;
            self.pop_raise_target();
            for _ in custom_siblings.iter().rev() {
                self.pop_effect_unwind_target();
            }
            self.pop_immediate_resume_ctx();

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
            let resume_ok_bb = self
                .context
                .append_basic_block(func, &format!("handle_multi_resume_arm_ok_{site_idx}"));
            let resume_missing_bb = self.context.append_basic_block(
                func,
                &format!("handle_multi_resume_arm_missing_{site_idx}"),
            );

            let used = self
                .builder
                .build_load(
                    self.context.bool_type(),
                    site_plan.resume_used_ptr,
                    "handle_multi_resume_used",
                )?
                .into_int_value();
            self.builder
                .build_conditional_branch(used, resume_ok_bb, resume_missing_bb)?;

            self.builder.position_at_end(resume_missing_bb);
            self.emit_exit_with_code(span, 3)?;

            self.builder.position_at_end(resume_ok_bb);
            if let Some(custom_restore_top) = custom_restore_top {
                let rt_swap = self.declare_runtime_effect_handler_stack_swap_top();
                let _ = self.builder.build_call(
                    rt_swap,
                    &[custom_restore_top.into()],
                    "handle_multi_resume_restore",
                )?;
            }
            self.builder.build_unconditional_branch(dispatch_bb)?;

            self.env.pop_scope();
        }

        self.env.pop_scope();

        self.builder.position_at_end(finally_unwind_bb);
        if let Some(custom_outer_top) = custom_outer_top {
            let rt_swap = self.declare_runtime_effect_handler_stack_swap_top();
            let _ = self.builder.build_call(
                rt_swap,
                &[custom_outer_top.into()],
                "handle_multi_resume_finally_unwind_detach",
            )?;
        }
        if let Some(finally) = handle.finally.as_ref() {
            let _ = self.codegen_block_value(finally)?;
        }
        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_none()
        {
            if let Some(target) = outer_raise_target {
                self.builder.build_unconditional_branch(target)?;
            } else {
                let ret_ty =
                    self.current_fun_return_ty
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle multi immediate finally unwind needs function return type",
                            at: span.into(),
                        })?;
                let v = self.default_value(span, ret_ty)?;
                self.emit_return(span, ret_ty, v)?;
            }
        }

        self.builder.position_at_end(finally_bb);
        if let Some(finally) = handle.finally.as_ref() {
            let _ = self.codegen_block_value(finally)?;
        }
        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_none()
        {
            self.builder.build_unconditional_branch(done_bb)?;
        }

        if let Some(effect_dispatch_bb) = effect_dispatch_bb {
            let Some(effect_dispatch_nomatch_bb) = effect_dispatch_nomatch_bb else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle multi immediate sibling dispatch missing no-match block",
                    at: span.into(),
                });
            };

            self.builder.position_at_end(effect_dispatch_bb);
            let rt_read_tag = self.declare_runtime_effect_perform_slot_read_op_tag();
            let tag_call = self.builder.build_call(
                rt_read_tag,
                &[],
                "handle_multi_resume_dispatch_read_op_tag",
            )?;
            let tag_raw =
                tag_call
                    .try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "dispatch read_op_tag return value",
                        at: span.into(),
                    })?;
            let BasicValueEnum::IntValue(slot_tag) = tag_raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "dispatch read_op_tag return type",
                    at: span.into(),
                });
            };

            let mut dispatch_cases: Vec<(IntValue<'ctx>, inkwell::basic_block::BasicBlock<'ctx>)> =
                Vec::new();
            if let Some(raise_catch_bb) = raise_catch_bb {
                let raise_tag = self.effect_op_tag("scoop.core.Raise.raise");
                dispatch_cases.push((i32_ty.const_int(raise_tag as u64, false), raise_catch_bb));
            }
            for custom in &custom_siblings {
                dispatch_cases.push((i32_ty.const_int(custom.op_tag as u64, false), custom.catch_bb));
            }
            self.builder
                .build_switch(slot_tag, effect_dispatch_nomatch_bb, &dispatch_cases)?;

            self.builder.position_at_end(effect_dispatch_nomatch_bb);
            if let Some(custom_outer_top) = custom_outer_top {
                let rt_swap = self.declare_runtime_effect_handler_stack_swap_top();
                let _ = self.builder.build_call(
                    rt_swap,
                    &[custom_outer_top.into()],
                    "handle_multi_resume_dispatch_detach",
                )?;
            }
            self.builder.build_unconditional_branch(finally_unwind_bb)?;
        }

        if let (Some(raise_arm), Some(raise_catch_bb)) = (raise_sibling, raise_catch_bb) {
            let binder = &raise_arm.op.binders[0];
            self.builder.position_at_end(raise_catch_bb);

            if let Some(custom_outer_top) = custom_outer_top {
                let rt_swap = self.declare_runtime_effect_handler_stack_swap_top();
                let _ = self.builder.build_call(
                    rt_swap,
                    &[custom_outer_top.into()],
                    "handle_multi_resume_raise_detach",
                )?;
            }

            let rt_len = self.declare_runtime_effect_perform_slot_read_len_words();
            let call = self
                .builder
                .build_call(rt_len, &[], "multi_resume_raise_read_slot_len_words")?;
            let raw =
                call.try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_len_words return value",
                        at: span.into(),
                    })?;
            let BasicValueEnum::IntValue(len_words_i32) = raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "effect slot_read_len_words return type",
                    at: span.into(),
                });
            };

            let expected_len = self.context.i32_type().const_int(2, false);
            let len_ok = self.builder.build_int_compare(
                IntPredicate::EQ,
                len_words_i32,
                expected_len,
                "multi_resume_raise_slot_len_ok",
            )?;
            let len_ok_bb = self
                .context
                .append_basic_block(func, "multi_resume_raise_slot_len_ok_bb");
            let len_bad_bb = self
                .context
                .append_basic_block(func, "multi_resume_raise_slot_len_bad_bb");
            self.builder
                .build_conditional_branch(len_ok, len_ok_bb, len_bad_bb)?;

            self.builder.position_at_end(len_bad_bb);
            self.emit_exit_with_code(span, 3)?;

            self.builder.position_at_end(len_ok_bb);

            let rt_read_at = self.declare_runtime_effect_perform_slot_read_u64_at();
            let idx0 = self.context.i32_type().const_int(0, false);
            let idx1 = self.context.i32_type().const_int(1, false);

            let kind_call = self.builder.build_call(
                rt_read_at,
                &[idx0.into()],
                "multi_resume_raise_read_slot_word0",
            )?;
            let kind_raw = kind_call.try_as_basic_value().basic().ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "effect slot_read_word0 return value",
                    at: span.into(),
                },
            )?;
            let BasicValueEnum::IntValue(kind_u64) = kind_raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "effect slot_read_word0 return type",
                    at: span.into(),
                });
            };

            let value_call = self.builder.build_call(
                rt_read_at,
                &[idx1.into()],
                "multi_resume_raise_read_slot_word1",
            )?;
            let value_raw = value_call.try_as_basic_value().basic().ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "effect slot_read_word1 return value",
                    at: span.into(),
                },
            )?;
            let BasicValueEnum::IntValue(value_u64) = value_raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "effect slot_read_word1 return type",
                    at: span.into(),
                });
            };

            let rt_clear = self.declare_runtime_effect_clear();
            let _ = self
                .builder
                .build_call(rt_clear, &[], "multi_resume_raise_clear")?;

            self.env.push_scope();

            let binder_cg_ty =
                self.cg_ty_of(binder.ty)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle binder type",
                        at: binder.span.into(),
                    })?;
            let binder_value = match binder_cg_ty {
                CgTy::Int(int_ty) => {
                    let expected = self.context.i64_type().const_int(1, false);
                    let ok = self.builder.build_int_compare(
                        IntPredicate::EQ,
                        kind_u64,
                        expected,
                        "multi_resume_raise_kind_is_int",
                    )?;
                    let ok_bb = self
                        .context
                        .append_basic_block(func, "multi_resume_raise_kind_int_ok");
                    let bad_bb = self
                        .context
                        .append_basic_block(func, "multi_resume_raise_kind_int_bad");
                    self.builder.build_conditional_branch(ok, ok_bb, bad_bb)?;

                    self.builder.position_at_end(bad_bb);
                    self.emit_exit_with_code(span, 3)?;

                    self.builder.position_at_end(ok_bb);
                    let from_u64 = IntTy {
                        bits: 64,
                        signed: false,
                    };
                    let decoded = self.cast_int(value_u64, from_u64, int_ty)?;
                    CgValue::int(decoded, int_ty)
                }
                CgTy::Enum(enum_ty) if self.is_sysroot_runtime_error_enum(enum_ty) => {
                    let expected = self.context.i64_type().const_int(2, false);
                    let ok = self.builder.build_int_compare(
                        IntPredicate::EQ,
                        kind_u64,
                        expected,
                        "multi_resume_raise_kind_is_runtime_error",
                    )?;
                    let ok_bb = self.context.append_basic_block(
                        func,
                        "multi_resume_raise_kind_runtime_error_ok",
                    );
                    let bad_bb = self.context.append_basic_block(
                        func,
                        "multi_resume_raise_kind_runtime_error_bad",
                    );
                    self.builder.build_conditional_branch(ok, ok_bb, bad_bb)?;

                    self.builder.position_at_end(bad_bb);
                    self.emit_exit_with_code(span, 3)?;

                    self.builder.position_at_end(ok_bb);

                    let repr = self.cg_enum_layout(span, enum_ty)?.repr;
                    if !matches!(repr, CgEnumRepr::TaggedUnion) {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "Raise<RuntimeError> niche repr (not supported)",
                            at: span.into(),
                        });
                    }

                    let tag_i32 = self.builder.build_int_truncate(
                        value_u64,
                        self.context.i32_type(),
                        "multi_resume_raise_runtime_error_tag_i32",
                    )?;
                    let payload_word_zero =
                        self.int_type(self.enum_payload_ty()).const_int(0, false);
                    let payload_ptr_zero = self.llvm_gc_i8_ptr_type().const_null();

                    let llvm_enum_ty = self.llvm_enum_value_type(span, enum_ty)?;
                    let llvm_enum_ty = llvm_enum_ty.into_struct_type();
                    let mut agg: AggregateValueEnum<'ctx> = llvm_enum_ty.get_undef().into();
                    agg = self.builder.build_insert_value(
                        agg,
                        tag_i32,
                        0,
                        "multi_resume_raise_runtime_error_tag",
                    )?;
                    agg = self.builder.build_insert_value(
                        agg,
                        payload_word_zero,
                        1,
                        "multi_resume_raise_runtime_error_payload_word",
                    )?;
                    agg = self.builder.build_insert_value(
                        agg,
                        payload_ptr_zero,
                        2,
                        "multi_resume_raise_runtime_error_payload_ptr",
                    )?;
                    CgValue {
                        ty: CgTy::Enum(enum_ty),
                        value: Some(agg.as_basic_value_enum()),
                    }
                }
                _ => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle binder type (Raise payload decode)",
                        at: span.into(),
                    });
                }
            };
            let binder_ptr = self.create_entry_alloca(binder.span, &binder.name, binder_cg_ty)?;
            let _ = self.store_local_value(binder.span, binder_ptr, binder_cg_ty, binder_value)?;
            self.env.insert(
                binder.id,
                CgLocal {
                    hir_ty: Some(binder.ty),
                    ty: binder_cg_ty,
                    ptr: binder_ptr,
                    mutable: false,
                },
            );

            self.push_raise_target(finally_unwind_bb);
            let arm_v = self.codegen_expr_in_expected_context(&raise_arm.body, Some(out_ty))?;
            self.pop_raise_target();
            let arm_v = if out_ty == CgTy::Unit {
                CgValue::unit()
            } else {
                self.coerce_value(raise_arm.body.span, arm_v, out_ty)?
            };

            let catch_end =
                self.builder
                    .get_insert_block()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "builder has no insert block",
                        at: span.into(),
                    })?;
            if catch_end.get_terminator().is_none() {
                if let Some(ptr) = result_ptr {
                    let _ = self.store_local_value(raise_arm.body.span, ptr, out_ty, arm_v)?;
                }
                self.builder.build_unconditional_branch(finally_bb)?;
            }
            self.env.pop_scope();
        }

        for custom in &custom_siblings {
            let arm = custom.arm;
            let binder = &arm.op.binders[0];
            self.builder.position_at_end(custom.catch_bb);

            if let Some(custom_outer_top) = custom_outer_top {
                let rt_swap = self.declare_runtime_effect_handler_stack_swap_top();
                let _ = self.builder.build_call(
                    rt_swap,
                    &[custom_outer_top.into()],
                    "handle_multi_resume_custom_detach",
                )?;
            }

            let rt_len = self.declare_runtime_effect_perform_slot_read_len_words();
            let call = self
                .builder
                .build_call(rt_len, &[], "multi_resume_custom_read_slot_len_words")?;
            let raw =
                call.try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_len_words return value",
                        at: span.into(),
                    })?;
            let BasicValueEnum::IntValue(len_words_i32) = raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "effect slot_read_len_words return type",
                    at: span.into(),
                });
            };

            let expected_len = self.context.i32_type().const_int(1, false);
            let len_ok = self.builder.build_int_compare(
                IntPredicate::EQ,
                len_words_i32,
                expected_len,
                "multi_resume_custom_slot_len_ok",
            )?;
            let len_ok_bb = self
                .context
                .append_basic_block(func, "multi_resume_custom_slot_len_ok_bb");
            let len_bad_bb = self
                .context
                .append_basic_block(func, "multi_resume_custom_slot_len_bad_bb");
            self.builder
                .build_conditional_branch(len_ok, len_ok_bb, len_bad_bb)?;

            self.builder.position_at_end(len_bad_bb);
            self.emit_exit_with_code(span, 3)?;

            self.builder.position_at_end(len_ok_bb);

            let rt_read = self.declare_runtime_effect_perform_slot_read_u64();
            let value_call =
                self.builder
                    .build_call(rt_read, &[], "multi_resume_custom_read_slot_word0")?;
            let value_raw = value_call.try_as_basic_value().basic().ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "effect slot_read_word0 return value",
                    at: span.into(),
                },
            )?;
            let BasicValueEnum::IntValue(value_u64) = value_raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "effect slot_read_word0 return type",
                    at: span.into(),
                });
            };
            let rt_read_gc = self.declare_runtime_effect_perform_slot_read_gc_ref();
            let gc_call = self
                .builder
                .build_call(rt_read_gc, &[], "multi_resume_custom_read_slot_gc_ref")?;
            let gc_raw =
                gc_call
                    .try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_gc_ref return value",
                        at: span.into(),
                    })?;
            let BasicValueEnum::PointerValue(gc_ref_raw) = gc_raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "effect slot_read_gc_ref return type",
                    at: span.into(),
                });
            };

            self.env.push_scope();

            let binder_cg_ty =
                self.cg_ty_of(binder.ty)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle binder type (custom non-resuming)",
                        at: binder.span.into(),
                    })?;
            let binder_value = self.decode_abi_payload_transport(
                binder.span,
                value_u64,
                gc_ref_raw,
                binder_cg_ty,
            )?;

            let binder_ptr = self.create_entry_alloca(binder.span, &binder.name, binder_cg_ty)?;
            let _ = self.store_local_value(binder.span, binder_ptr, binder_cg_ty, binder_value)?;
            self.env.insert(
                binder.id,
                CgLocal {
                    hir_ty: Some(binder.ty),
                    ty: binder_cg_ty,
                    ptr: binder_ptr,
                    mutable: false,
                },
            );

            let rt_clear = self.declare_runtime_effect_clear();
            let _ = self
                .builder
                .build_call(rt_clear, &[], "multi_resume_custom_clear")?;

            self.push_effect_unwind_target(&custom.arm.op.op.fqn, finally_unwind_bb);
            self.push_raise_target(finally_unwind_bb);
            let arm_v = self.codegen_expr_in_expected_context(&arm.body, Some(out_ty))?;
            self.pop_raise_target();
            self.pop_effect_unwind_target();
            let arm_v = if out_ty == CgTy::Unit {
                CgValue::unit()
            } else {
                self.coerce_value(arm.body.span, arm_v, out_ty)?
            };

            let catch_end =
                self.builder
                    .get_insert_block()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "builder has no insert block",
                        at: span.into(),
                    })?;
            if catch_end.get_terminator().is_none() {
                if let Some(ptr) = result_ptr {
                    let _ = self.store_local_value(arm.body.span, ptr, out_ty, arm_v)?;
                }
                self.builder.build_unconditional_branch(finally_bb)?;
            }
            self.env.pop_scope();
        }

        self.builder.position_at_end(done_bb);

        Ok(match out_ty {
            CgTy::Unit => CgValue::unit(),
            CgTy::Never => CgValue::never(),
            CgTy::Bool
            | CgTy::Float64
            | CgTy::Float32
            | CgTy::Int(_)
            | CgTy::String
            | CgTy::Ref
            | CgTy::Tuple(_)
            | CgTy::Struct(_)
            | CgTy::Enum(_) => {
                let Some(ptr) = result_ptr else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle result slot",
                        at: span.into(),
                    });
                };
                let llvm_ty = self.llvm_basic_type_of(span, out_ty)?;
                let loaded = self
                    .builder
                    .build_load(llvm_ty, ptr, "handle_multi_resume_result")?;
                CgValue {
                    ty: out_ty,
                    value: Some(loaded),
                }
            }
        })
    }
}
