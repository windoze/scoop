#[derive(Default)]
struct HandleArmBuckets<'hir> {
    immediate_arms: Vec<(&'hir hir::HandleArm, hir::SymbolId)>,
    escape_arms: Vec<(&'hir hir::HandleArm, hir::SymbolId)>,
    nonresuming_arms: Vec<&'hir hir::HandleArm>,
}

impl<'hir> HandleArmBuckets<'hir> {
    fn lowering_counts(&self) -> SimplifiedArmLoweringCounts {
        SimplifiedArmLoweringCounts {
            flag_unwind: self.nonresuming_arms.len(),
            stack_reenter: self.immediate_arms.len(),
            heap_continuation: self.escape_arms.len(),
        }
    }
}

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    /// codegen 一个 `Raise.raise(e)`（HIR `Perform` 的最小子集）。
    ///
    /// 当前阶段（T0614）约束：
    /// - 只支持 `scoop.core.Raise.raise`；
    /// - `e` 只支持：
    ///   - word-sized `Int`（沿用 T0614 的最小约定）；
    ///   - `RuntimeError`（T0818：写入 enum tag）；
    /// - 不支持 finally / 自定义 effect / `-> resume`。
    pub(super) fn codegen_perform_expr(
        &mut self,
        span: crate::span::Span,
        op: &hir::EffectOpRef,
        args: &[hir::CallArg],
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if op.fqn != "scoop.core.Raise.raise" {
            return self.codegen_perform_expr_nonresuming_single_payload(span, op, args, expected);
        }

        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Raise.raise arity mismatch",
                at: span.into(),
            });
        }
        let hir::CallArg::Positional(err_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Raise.raise named arg",
                at: span.into(),
            });
        };

        // 1) 计算 `error` 值，并编码为 slot 的复合 payload（2 words）。
        let (payload_kind_u64, payload_value_u64) =
            self.codegen_raise_error_payload_words(err_expr)?;

        // 2) 写 slot + set flag。
        let raise_tag = self.effect_op_tag("scoop.core.Raise.raise");
        let tag_i32 = self.context.i32_type().const_int(raise_tag as u64, false);
        let rt_write = self.declare_runtime_effect_perform_slot_write_u64_2();
        let _ = self.builder.build_call(
            rt_write,
            &[
                tag_i32.into(),
                payload_kind_u64.into(),
                payload_value_u64.into(),
            ],
            "raise_write_slot",
        )?;

        let (src_line, src_col) = self.effect_trace_line_col(span)?;
        let rt_set = self.declare_runtime_effect_set_active_with_trace();
        let i32_ty = self.context.i32_type();
        let src_line_i32 = i32_ty.const_int(src_line as u64, false);
        let src_col_i32 = i32_ty.const_int(src_col as u64, false);
        let _ = self.builder.build_call(
            rt_set,
            &[src_line_i32.into(), src_col_i32.into()],
            "raise_set_active",
        )?;

        // 3) "早退"：在 handler boundary 内跳到 catch，否则返回默认值向外传播。
        if let Some(target) = self.current_raise_target() {
            self.builder.build_unconditional_branch(target)?;
        } else {
            let ret_ty = self
                .current_fun_return_ty
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "Raise.raise needs function return type",
                    at: span.into(),
                })?;
            let v = self.default_value(span, ret_ty)?;
            self.emit_return(span, ret_ty, v)?;
        }

        // 4) 继续生成后续 IR：把 builder 移到一个"不可达 continuation block"，避免后续插入失败。
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

        // Raise 的返回类型在类型系统里是 `Nothing`，可用于任意期望类型；
        // 这里返回一个"期望类型的默认值"以保持后续 codegen 可继续推进。
        Ok(match expected {
            Some(ty) => self.default_value(span, ty)?,
            None => CgValue::unit(),
        })
    }

    /// codegen 一个最小自定义 non-resuming effect `perform`（T0625/T2002a）。
    ///
    /// 当前阶段约束：
    /// - 仅支持 `op(arg)` 的单 payload 形式；
    /// - payload ABI 为双通道：标量走 `word0`，GC ref / boxed aggregate 走 `gc_ref`；
    /// - flag-propagation / handler stack dispatch 语义保持不变。
    pub(super) fn codegen_perform_expr_nonresuming_single_payload(
        &mut self,
        span: crate::span::Span,
        op: &hir::EffectOpRef,
        args: &[hir::CallArg],
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "effect op arity mismatch (custom non-resuming)",
                at: span.into(),
            });
        }
        let hir::CallArg::Positional(payload_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "effect op named arg (custom non-resuming)",
                at: span.into(),
            });
        };

        let payload_v = self.codegen_expr(payload_expr)?;
        let payload = self.encode_abi_payload_transport(payload_expr.span, payload_v)?;

        // T1608：使用统一的 op_tag 分配（按 FQN 精确匹配，与 handler_stack_push 一致）。
        let tag = self.effect_op_tag(&op.fqn);
        let op_tag_i32 = self.context.i32_type().const_int(tag as u64, false);
        let rt_write = self.declare_runtime_effect_perform_slot_write_u64_with_gc_ref();
        let gc_ref = payload
            .gc_ref
            .unwrap_or_else(|| self.llvm_gc_i8_ptr_type().const_null());
        let _ = self.builder.build_call(
            rt_write,
            &[op_tag_i32.into(), payload.word.into(), gc_ref.into()],
            "effect_write_slot_payload",
        )?;
        let (src_line, src_col) = self.effect_trace_line_col(span)?;
        let rt_set = self.declare_runtime_effect_set_active_with_trace();
        let i32_ty = self.context.i32_type();
        let src_line_i32 = i32_ty.const_int(src_line as u64, false);
        let src_col_i32 = i32_ty.const_int(src_col as u64, false);
        let _ = self.builder.build_call(
            rt_set,
            &[src_line_i32.into(), src_col_i32.into()],
            "effect_set_active",
        )?;

        // T1608: 跨 effect 传播——清理当前 handler stack 中不匹配的中间帧。
        let rt_unwind = self.declare_runtime_effect_handler_stack_unwind_to_tag();
        let _ = self
            .builder
            .build_call(rt_unwind, &[op_tag_i32.into()], "effect_unwind_to_tag")?;

        if let Some(target) = self.current_effect_unwind_target(&op.fqn) {
            // 同一函数内存在匹配的 handle boundary → 直接跳转到 catch block。
            self.builder.build_unconditional_branch(target)?;
        } else {
            // T1606f-1: 无本函数内 handler boundary → 通过 flag-propagation 向外传播
            // （与 Raise.raise 的"返回默认值"路径一致）。
            // flag 与 slot 已在上方写入；caller 的 emit_effect_unwind_if_active 会检查 flag
            // 并路由到最近的匹配 handler 或继续向外 return。

            // T1606f-2: If this function is "suspendable" (callee_suspend_save_ctx is set),
            // save locals to a heap CalleeSuspendState before returning default.
            if let Some(ctx) = self.callee_suspend_save_ctx.clone() {
                self.emit_callee_suspend_state_save(span, &ctx)?;
            }

            let ret_ty = self
                .current_fun_return_ty
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "effect perform (indirect) needs function return type",
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
        let dead = self.context.append_basic_block(func, "after_effect_dead");
        self.builder.position_at_end(dead);

        Ok(match expected {
            Some(ty) => self.default_value(span, ty)?,
            None => CgValue::unit(),
        })
    }

    fn collect_handle_arm_buckets<'hir>(
        &self,
        handle: &'hir hir::HandleExpr,
    ) -> HandleArmBuckets<'hir> {
        let mut buckets = HandleArmBuckets::default();
        for arm in &handle.arms {
            match arm.kind {
                hir::HandleArmKind::ImmediateResume { resume } => {
                    buckets.immediate_arms.push((arm, resume));
                }
                hir::HandleArmKind::EscapeContinuation { continuation } => {
                    buckets.escape_arms.push((arm, continuation));
                }
                hir::HandleArmKind::NonResuming => buckets.nonresuming_arms.push(arm),
            }
        }
        buckets
    }

    fn ensure_simplification_matches_handle_arms<'hir>(
        &self,
        span: crate::span::Span,
        simplification: &HandleModeSpecificSimplification,
        buckets: &HandleArmBuckets<'hir>,
    ) -> Result<(), LlvmEmitError> {
        let simplified = simplification.arm_lowering_counts();
        let actual = buckets.lowering_counts();
        if simplified == actual {
            return Ok(());
        }

        Err(LlvmEmitError::UnsupportedMainBody {
            kind: "handle simplification classification mismatch",
            at: span.into(),
        })
    }

    /// codegen 一个 `handle { ... } with { Raise.raise(e) -> ... }`（`try/catch` 的 lowering 产物）。
    ///
    /// 当前阶段（T0614）约束：
    /// - 只支持捕获 `scoop.core.Raise.raise`；
    /// - 只支持单个 arm（最小示例）；finally 语义由 T0615 补齐；
    /// - arm body 在"handler scope"之外生成，避免 self-capture（PLAN §6.2）。
    pub(super) fn codegen_handle_expr(
        &mut self,
        span: crate::span::Span,
        handle: &hir::HandleExpr,
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let out_ty = expected.ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "handle needs expected type context",
            at: span.into(),
        })?;
        let state_machine_plan = self.build_handle_state_machine_plan(handle);
        let _plan_signature = state_machine_plan.structural_signature();
        let mode_specific_simplification = state_machine_plan.build_mode_specific_simplification();
        let _simplification_signature = mode_specific_simplification.structural_signature();
        let handle_arm_buckets = self.collect_handle_arm_buckets(handle);
        let has_resuming_arms = !handle_arm_buckets.immediate_arms.is_empty()
            || !handle_arm_buckets.escape_arms.is_empty();

        self.ensure_simplification_matches_handle_arms(
            span,
            &mode_specific_simplification,
            &handle_arm_buckets,
        )?;

        // T2003u4a：对纯 non-resuming handle，`NoSuspendSites` 现在直接信任 unified plan，
        // 不再额外依赖 `block_may_perform` 这个旧 gate。
        if !has_resuming_arms
            && matches!(
                mode_specific_simplification.codegen_entrypoint(),
                SimplifiedCodegenEntrypoint::NoSuspendSites
            )
        {
            return self.codegen_handle_expr_no_perform(span, handle, out_ty);
        }

        // immediate-resume / escape-continuation / mixed-arm 仍保留旧 gate：
        // 它们当前的 specialized emitter 还没有完全吸收“0 matching perform + hidden unwind”
        // 这类边界，待 T2003u4b / T2003u4c 再继续迁移。
        if has_resuming_arms && !self.block_may_perform(&handle.body) {
            return self.codegen_handle_expr_no_perform(span, handle, out_ty);
        }

        let codegen_entrypoint = match mode_specific_simplification.codegen_entrypoint() {
            SimplifiedCodegenEntrypoint::NoSuspendSites if has_resuming_arms => {
                mode_specific_simplification.codegen_entrypoint_from_arm_mix()
            }
            route => route,
        };

        match codegen_entrypoint {
            // T2003u3b：入口选路改由 simplification 驱动；旧 emitter 仍作为过渡实现保留。
            SimplifiedCodegenEntrypoint::NoSuspendSites => {
                return self.codegen_handle_expr_no_perform(span, handle, out_ty);
            }
            SimplifiedCodegenEntrypoint::SingleNonResuming => {}
            SimplifiedCodegenEntrypoint::SingleImmediateResume => {
                let Some((arm, resume)) = handle_arm_buckets.immediate_arms.first().copied() else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle arm dispatch (missing immediate-resume arm)",
                        at: span.into(),
                    });
                };
                let Some(arm_id) = handle
                    .arms
                    .iter()
                    .position(|candidate| std::ptr::eq(candidate, arm))
                else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle arm dispatch (immediate-resume arm id)",
                        at: arm.span.into(),
                    });
                };
                return self.codegen_handle_expr_immediate_resume(
                    span,
                    ImmediateResumeHandleLowering {
                        handle,
                        state_machine_plan: &state_machine_plan,
                        arm_id: arm_id as ArmPlanId,
                        arm,
                        resume_symbol: resume,
                        out_ty,
                    },
                );
            }
            SimplifiedCodegenEntrypoint::SingleEscapeContinuation => {
                let Some((arm, continuation)) =
                    handle_arm_buckets.escape_arms.first().copied()
                else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle arm dispatch (missing escape-continuation arm)",
                        at: span.into(),
                    });
                };
                let Some(arm_id) = handle
                    .arms
                    .iter()
                    .position(|candidate| std::ptr::eq(candidate, arm))
                else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle arm dispatch (escape-continuation arm id)",
                        at: arm.span.into(),
                    });
                };
                let seq = self.escape_continuation_seq;
                self.escape_continuation_seq = self.escape_continuation_seq.saturating_add(1);
                return self.codegen_handle_expr_escape_continuation(
                    span,
                    EscapeContinuationHandleLowering {
                        handle,
                        state_machine_plan: &state_machine_plan,
                        arm_id: arm_id as ArmPlanId,
                        arm,
                        continuation_symbol: continuation,
                        seq,
                        out_ty,
                    },
                );
            }
            SimplifiedCodegenEntrypoint::MultiNonResuming
            | SimplifiedCodegenEntrypoint::MultipleEscapeTopLevelDirect
            | SimplifiedCodegenEntrypoint::MultipleImmediateResumeTopLevel
            | SimplifiedCodegenEntrypoint::ImmediateResumeWithNonResumingSiblings
            | SimplifiedCodegenEntrypoint::EscapeContinuationWithNonResumingSiblings
            | SimplifiedCodegenEntrypoint::ImmediateResumeWithEscapeSibling
            | SimplifiedCodegenEntrypoint::ImmediateResumeWithEscapeAndNonResumingSiblings
            | SimplifiedCodegenEntrypoint::UnsupportedMixedMultipleEscapeWithImmediate
            | SimplifiedCodegenEntrypoint::UnsupportedMixedMultipleImmediateWithEscape => {
                return self.codegen_handle_expr_multi_arm(
                    span,
                    handle,
                    out_ty,
                    &state_machine_plan,
                    &handle_arm_buckets,
                    codegen_entrypoint,
                );
            }
        }

        let Some(arm) = handle_arm_buckets.nonresuming_arms.first().copied() else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle arm dispatch (missing non-resuming arm)",
                at: span.into(),
            });
        };
        if arm.op.op.fqn != "scoop.core.Raise.raise" {
            return self.codegen_handle_expr_nonresuming_single_payload(span, handle, arm, out_ty);
        }
        if arm.op.binders.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle binder count (only 1 supported)",
                at: arm.op.span.into(),
            });
        }
        let binder = &arm.op.binders[0];

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

        // T0913 / T1608：在动态层维护 handler stack（Appendix A），使用统一 op_tag 分配。
        let op_tag_val = self.effect_op_tag(&arm.op.op.fqn);
        let handler_frame_ty = self.llvm_effect_handler_frame_type();
        let handler_frame_ptr =
            self.create_entry_alloca_raw(span, "handle_effect_frame", handler_frame_ty.into())?;

        let outer_raise_target = self.current_raise_target();

        let body_bb = self.context.append_basic_block(func, "handle_body");
        let catch_bb = self.context.append_basic_block(func, "handle_catch");

        // `finally` 语义：保证在"正常路径 / catch 返回 / catch 继续 raise 向外传播"三种情况下都执行一次。
        // - 正常路径与 catch 返回：汇合到 finally_bb 再进入 merge；
        // - catch 内发生 raise：先进入 finally_unwind_bb 执行 finally，然后向外传播 raise（不清 flag/slot）。
        let finally_bb = self.context.append_basic_block(func, "handle_finally");
        let finally_unwind_bb = self
            .context
            .append_basic_block(func, "handle_finally_unwind");
        let merge_bb = self.context.append_basic_block(func, "handle_merge");

        let result_ptr = if out_ty == CgTy::Unit {
            None
        } else {
            Some(self.create_entry_alloca(span, "handle_result", out_ty)?)
        };

        // 进入 handle body：push handler frame（动态上下文）。
        let rt_push = self.declare_runtime_effect_handler_stack_push();
        let i8_ptr_ty = self.context.ptr_type(AddressSpace::default());
        let frame_i8 =
            self.builder
                .build_bit_cast(handler_frame_ptr, i8_ptr_ty, "handle_effect_frame_i8")?;
        let op_tag_i32 = self.context.i32_type().const_int(op_tag_val as u64, false);
        let _ = self.builder.build_call(
            rt_push,
            &[frame_i8.into(), op_tag_i32.into()],
            "handle_effect_push",
        )?;

        // 进入 handle：先执行 body；若发生 Raise，则通过 flag/slot unwind 到 catch_bb。
        self.builder.build_unconditional_branch(body_bb)?;

        // --- body ---
        self.builder.position_at_end(body_bb);
        self.push_raise_target(catch_bb);
        let body_v = self.codegen_block_value(&handle.body)?;
        let body_v = if out_ty == CgTy::Unit {
            CgValue::unit()
        } else {
            self.coerce_value(handle.body.span, body_v, out_ty)?
        };
        self.pop_raise_target();

        // body 正常结束：进入 finally（并保存结果值）。
        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_none()
        {
            if let Some(ptr) = result_ptr {
                let _ = self.store_local_value(handle.body.span, ptr, out_ty, body_v)?;
            }

            // body 正常结束：pop handler frame，使 finally 处于 handler scope 之外（与现有 lowering 一致）。
            let rt_pop = self.declare_runtime_effect_handler_stack_pop();
            let i8_ptr_ty = self.context.ptr_type(AddressSpace::default());
            let frame_i8 = self.builder.build_bit_cast(
                handler_frame_ptr,
                i8_ptr_ty,
                "handle_effect_frame_i8",
            )?;
            let _ = self
                .builder
                .build_call(rt_pop, &[frame_i8.into()], "handle_effect_pop")?;

            self.builder.build_unconditional_branch(finally_bb)?;
        }

        // --- catch ---
        self.builder.position_at_end(catch_bb);

        // 进入 handler arm：pop handler frame（Appendix A.4：arm body 在自身 handler scope 外执行）。
        let rt_pop = self.declare_runtime_effect_handler_stack_pop();
        let i8_ptr_ty = self.context.ptr_type(AddressSpace::default());
        let frame_i8 =
            self.builder
                .build_bit_cast(handler_frame_ptr, i8_ptr_ty, "handle_effect_frame_i8")?;
        let _ = self
            .builder
            .build_call(rt_pop, &[frame_i8.into()], "handle_effect_pop")?;

        // 读取 slot（payload words）并清除 flag/slot。
        //
        // TODO T0630：目前 `Raise.raise` 统一写入 2 个 word（kind + value），这里做运行期断言，
        // 以便快速发现 lowering/codegen/runtime ABI 不一致的问题。
        let rt_len = self.declare_runtime_effect_perform_slot_read_len_words();
        let call = self
            .builder
            .build_call(rt_len, &[], "raise_read_slot_len_words")?;
        let raw = call
            .try_as_basic_value()
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
            "raise_slot_len_ok",
        )?;
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
        let len_ok_bb = self
            .context
            .append_basic_block(func, "raise_slot_len_ok_bb");
        let len_bad_bb = self
            .context
            .append_basic_block(func, "raise_slot_len_bad_bb");
        self.builder
            .build_conditional_branch(len_ok, len_ok_bb, len_bad_bb)?;

        self.builder.position_at_end(len_bad_bb);
        let exit = self.declare_libc_exit();
        let code = self.context.i32_type().const_int(3, false);
        let _ = self
            .builder
            .build_call(exit, &[code.into()], "raise_slot_len_exit")?;
        self.builder.build_unreachable()?;

        self.builder.position_at_end(len_ok_bb);

        let rt_read_at = self.declare_runtime_effect_perform_slot_read_u64_at();
        let idx0 = self.context.i32_type().const_int(0, false);
        let idx1 = self.context.i32_type().const_int(1, false);

        let kind_call =
            self.builder
                .build_call(rt_read_at, &[idx0.into()], "raise_read_slot_word0")?;
        let kind_raw =
            kind_call
                .try_as_basic_value()
                .basic()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "effect slot_read_word0 return value",
                    at: span.into(),
                })?;
        let BasicValueEnum::IntValue(kind_u64) = kind_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "effect slot_read_word0 return type",
                at: span.into(),
            });
        };

        let value_call =
            self.builder
                .build_call(rt_read_at, &[idx1.into()], "raise_read_slot_word1")?;
        let value_raw =
            value_call
                .try_as_basic_value()
                .basic()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "effect slot_read_word1 return value",
                    at: span.into(),
                })?;
        let BasicValueEnum::IntValue(value_u64) = value_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "effect slot_read_word1 return type",
                at: span.into(),
            });
        };

        let rt_clear = self.declare_runtime_effect_clear();
        let _ = self.builder.build_call(rt_clear, &[], "raise_clear")?;

        // binder scope：arm body 在 handler scope 之外执行（因此不 push raise_target）。
        self.env.push_scope();

        let binder_cg_ty = self
            .cg_ty_of(binder.ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "handle binder type",
                at: binder.span.into(),
            })?;
        let binder_value = match binder_cg_ty {
            CgTy::Int(int_ty) => {
                // kind 断言：避免把 `RuntimeError` 等误解码为整数。
                let expected = self.context.i64_type().const_int(1, false);
                let ok = self.builder.build_int_compare(
                    IntPredicate::EQ,
                    kind_u64,
                    expected,
                    "raise_kind_is_int",
                )?;
                let ok_bb = self.context.append_basic_block(func, "raise_kind_int_ok");
                let bad_bb = self.context.append_basic_block(func, "raise_kind_int_bad");
                self.builder.build_conditional_branch(ok, ok_bb, bad_bb)?;

                self.builder.position_at_end(bad_bb);
                let exit = self.declare_libc_exit();
                let code = self.context.i32_type().const_int(3, false);
                let _ = self
                    .builder
                    .build_call(exit, &[code.into()], "raise_kind_int_exit")?;
                self.builder.build_unreachable()?;

                self.builder.position_at_end(ok_bb);

                // 传统路径：`Raise<Int>` —— 直接把 slot 的 u64 解码回整数。
                let from_u64 = IntTy {
                    bits: 64,
                    signed: false,
                };
                let decoded = self.cast_int(value_u64, from_u64, int_ty)?;
                CgValue::int(decoded, int_ty)
            }
            CgTy::Enum(enum_ty) if self.is_sysroot_runtime_error_enum(enum_ty) => {
                // kind 断言：避免把整数误解码为 RuntimeError。
                let expected = self.context.i64_type().const_int(2, false);
                let ok = self.builder.build_int_compare(
                    IntPredicate::EQ,
                    kind_u64,
                    expected,
                    "raise_kind_is_runtime_error",
                )?;
                let ok_bb = self
                    .context
                    .append_basic_block(func, "raise_kind_runtime_error_ok");
                let bad_bb = self
                    .context
                    .append_basic_block(func, "raise_kind_runtime_error_bad");
                self.builder.build_conditional_branch(ok, ok_bb, bad_bb)?;

                self.builder.position_at_end(bad_bb);
                let exit = self.declare_libc_exit();
                let code = self.context.i32_type().const_int(3, false);
                let _ = self.builder.build_call(
                    exit,
                    &[code.into()],
                    "raise_kind_runtime_error_exit",
                )?;
                self.builder.build_unreachable()?;

                self.builder.position_at_end(ok_bb);

                // `Raise<RuntimeError>`：slot 里承载的是 enum tag（u64），这里把它恢复为 enum 值。
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
                    "raise_runtime_error_tag_i32",
                )?;
                let payload_word_zero = self.int_type(self.enum_payload_ty()).const_int(0, false);
                let payload_ptr_zero = self.llvm_gc_i8_ptr_type().const_null();

                let llvm_enum_ty = self.llvm_enum_value_type(span, enum_ty)?;
                let llvm_enum_ty = llvm_enum_ty.into_struct_type();
                let mut agg: AggregateValueEnum<'ctx> = llvm_enum_ty.get_undef().into();
                agg =
                    self.builder
                        .build_insert_value(agg, tag_i32, 0, "raise_runtime_error_tag")?;
                agg = self.builder.build_insert_value(
                    agg,
                    payload_word_zero,
                    1,
                    "raise_runtime_error_payload_word",
                )?;
                agg = self.builder.build_insert_value(
                    agg,
                    payload_ptr_zero,
                    2,
                    "raise_runtime_error_payload_ptr",
                )?;
                CgValue {
                    ty: CgTy::Enum(enum_ty),
                    value: Some(agg.as_basic_value_enum()),
                }
            }
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle binder type (Raise payload decode)",
                    at: binder.span.into(),
                });
            }
        };
        let binder_ptr = self.create_entry_alloca(binder.span, &binder.name, binder_cg_ty)?;
        let _stored =
            self.store_local_value(binder.span, binder_ptr, binder_cg_ty, binder_value)?;
        self.env.insert(
            binder.id,
            CgLocal {
                hir_ty: Some(binder.ty),
                ty: binder_cg_ty,
                ptr: binder_ptr,
                mutable: false,
            },
        );

        // catch body 若再次发生 Raise：先执行 finally，再向外传播（不在本 handler 内消费 slot）。
        self.push_raise_target(finally_unwind_bb);
        let arm_v = self.codegen_expr_in_expected_context(&arm.body, Some(out_ty))?;
        self.pop_raise_target();
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
        let catch_reaches_merge = catch_end.get_terminator().is_none();
        if catch_reaches_merge {
            if let Some(ptr) = result_ptr {
                let _ = self.store_local_value(arm.body.span, ptr, out_ty, arm_v)?;
            }
            self.builder.build_unconditional_branch(finally_bb)?;
        }
        self.env.pop_scope();

        // --- finally_unwind ---
        self.builder.position_at_end(finally_unwind_bb);
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
                            kind: "handle finally unwind needs function return type",
                            at: span.into(),
                        })?;
                let v = self.default_value(span, ret_ty)?;
                self.emit_return(span, ret_ty, v)?;
            }
        }

        // --- finally ---
        self.builder.position_at_end(finally_bb);
        if let Some(finally) = handle.finally.as_ref() {
            let _ = self.codegen_block_value(finally)?;
        }
        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_none()
        {
            self.builder.build_unconditional_branch(merge_bb)?;
        }

        // --- merge ---
        self.builder.position_at_end(merge_bb);

        match out_ty {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
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
                let loaded = self.builder.build_load(llvm_ty, ptr, "handle_result")?;
                Ok(CgValue {
                    ty: out_ty,
                    value: Some(loaded),
                })
            }
        }
    }

    pub(super) fn codegen_handle_expr_no_perform(
        &mut self,
        span: crate::span::Span,
        handle: &hir::HandleExpr,
        out_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // no-perform fast path：不生成 catch/merge CFG，仅保留顺序语义（body -> finally）。
        let result_ptr = if out_ty == CgTy::Unit {
            None
        } else {
            Some(self.create_entry_alloca(span, "handle_noperform_result", out_ty)?)
        };

        // body
        let body_v = self.codegen_block_value_in_expected_context(&handle.body, Some(out_ty))?;
        let body_v = if out_ty == CgTy::Unit {
            CgValue::unit()
        } else {
            self.coerce_value(handle.body.span, body_v, out_ty)?
        };
        if let Some(ptr) = result_ptr {
            let _ = self.store_local_value(handle.body.span, ptr, out_ty, body_v)?;
        }

        // finally（仅在当前路径可达时执行）
        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_none()
            && let Some(finally) = handle.finally.as_ref()
        {
            let _ = self.codegen_block_value(finally)?;
        }

        // 若 body/finally 终止了当前块：为后续 codegen 创建一个 dead block。
        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        if insert_block.get_terminator().is_some() {
            let func = insert_block
                .get_parent()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no parent function",
                    at: span.into(),
                })?;
            let dead = self
                .context
                .append_basic_block(func, "handle_noperform_dead");
            self.builder.position_at_end(dead);
            return Ok(match out_ty {
                CgTy::Unit => CgValue::unit(),
                other => self.default_value(span, other)?,
            });
        }

        match out_ty {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
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
                    .build_load(llvm_ty, ptr, "handle_noperform_value")?;
                Ok(CgValue {
                    ty: out_ty,
                    value: Some(loaded),
                })
            }
        }
    }

    /// codegen 一个最小自定义 non-resuming effect 的 `handle`（T0625/T2002a）。
    ///
    /// 当前阶段约束：
    /// - 仅支持单 arm；
    /// - binder 仅支持单 payload；
    /// - payload ABI：`perform` 往 slot 写 `word0 + gc_ref` 双通道，catch 读取后再清 flag/slot。
    ///
    /// 关键语义（Appendix A.4）：
    /// - handler arm body 在自身 dispatch scope 外执行：因此 arm codegen 期间不在
    ///   `effect_unwind_target_stack` 中保留 `catch_bb` 入口；
    /// - 但为了确保 `finally` 语义（若有）仍然成立，arm body 内若再次 perform 同一 op，
    ///   会先跳到 `finally_unwind_bb` 执行 finally，再向外层 handler 传播。
    pub(super) fn codegen_handle_expr_nonresuming_single_payload(
        &mut self,
        span: crate::span::Span,
        handle: &hir::HandleExpr,
        arm: &hir::HandleArm,
        out_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if arm.op.binders.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle binder count (custom non-resuming, only single payload supported)",
                at: arm.op.span.into(),
            });
        }
        let binder = &arm.op.binders[0];

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

        // T1608：使用统一的 op_tag 分配。
        let tag = self.effect_op_tag(&arm.op.op.fqn);
        let op_tag_i32 = self.context.i32_type().const_int(tag as u64, false);

        let outer_target = self.current_effect_unwind_target(&arm.op.op.fqn);

        let body_bb = self.context.append_basic_block(func, "handle_custom_body");
        let catch_bb = self.context.append_basic_block(func, "handle_custom_catch");

        // `finally` 语义：保证在"正常路径 / catch 返回 / catch 继续 perform 向外传播"三种情况下都执行一次。
        let finally_bb = self
            .context
            .append_basic_block(func, "handle_custom_finally");
        let finally_unwind_bb = self
            .context
            .append_basic_block(func, "handle_custom_finally_unwind");
        let merge_bb = self.context.append_basic_block(func, "handle_custom_merge");

        let result_ptr = if out_ty == CgTy::Unit {
            None
        } else {
            Some(self.create_entry_alloca(span, "handle_custom_result", out_ty)?)
        };

        // handler frame（动态上下文）。
        let handler_frame_ty = self.llvm_effect_handler_frame_type();
        let handler_frame_ptr = self.create_entry_alloca_raw(
            span,
            "handle_custom_effect_frame",
            handler_frame_ty.into(),
        )?;

        // 进入 handle body：push handler frame（动态上下文）。
        let rt_push = self.declare_runtime_effect_handler_stack_push();
        let i8_ptr_ty = self.context.ptr_type(AddressSpace::default());
        let frame_i8 =
            self.builder
                .build_bit_cast(handler_frame_ptr, i8_ptr_ty, "handle_custom_frame_i8")?;
        let _ = self.builder.build_call(
            rt_push,
            &[frame_i8.into(), op_tag_i32.into()],
            "handle_custom_effect_push",
        )?;

        // T1606f-1: 间接 perform（跨函数调用）支持。
        //
        // 当 handle body 内调用的函数执行了匹配的 perform，该函数会通过 flag-propagation 返回
        // 默认值；call-site 的 emit_effect_unwind_if_active 检查 flag → 跳到最近的
        // raise_target_stack 入口。因此需要在 raise_target_stack 中放置一个"dispatch trampoline"，
        // 该 trampoline 读取 op_tag 并：
        //   - 若匹配本 handler 的 op_tag → 跳到 catch_bb（处理 effect）
        //   - 若不匹配（例如 Raise.raise 或其它 effect）→ pop handler frame 并向外传播。
        let dispatch_bb = self
            .context
            .append_basic_block(func, "handle_custom_dispatch");
        let dispatch_no_match_bb = self
            .context
            .append_basic_block(func, "handle_custom_dispatch_nomatch");
        let outer_raise_target = self.current_raise_target();

        // 进入 handle：先执行 body；若发生 perform，则跳到 catch_bb。
        self.builder.build_unconditional_branch(body_bb)?;

        // --- body ---
        self.builder.position_at_end(body_bb);
        self.push_effect_unwind_target(&arm.op.op.fqn, catch_bb);
        self.push_raise_target(dispatch_bb);
        let body_v = self.codegen_block_value_in_expected_context(&handle.body, Some(out_ty))?;
        self.pop_raise_target();
        self.pop_effect_unwind_target();

        let body_v = if out_ty == CgTy::Unit {
            CgValue::unit()
        } else {
            self.coerce_value(handle.body.span, body_v, out_ty)?
        };

        // body 正常结束：进入 finally（并保存结果值）。
        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_none()
        {
            if let Some(ptr) = result_ptr {
                let _ = self.store_local_value(handle.body.span, ptr, out_ty, body_v)?;
            }

            // body 正常结束：pop handler frame，使 finally 处于 handler scope 之外（Appendix A.4）。
            let rt_pop = self.declare_runtime_effect_handler_stack_pop();
            let i8_ptr_ty = self.llvm_i8_ptr_type();
            let frame_i8 = self.builder.build_bit_cast(
                handler_frame_ptr,
                i8_ptr_ty,
                "handle_custom_frame_i8",
            )?;
            let _ =
                self.builder
                    .build_call(rt_pop, &[frame_i8.into()], "handle_custom_effect_pop")?;

            self.builder.build_unconditional_branch(finally_bb)?;
        }

        // --- catch ---
        self.builder.position_at_end(catch_bb);

        // 进入 handler arm：pop handler frame（Appendix A.4：arm body 在自身 handler scope 外执行）。
        let rt_pop = self.declare_runtime_effect_handler_stack_pop();
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let frame_i8 =
            self.builder
                .build_bit_cast(handler_frame_ptr, i8_ptr_ty, "handle_custom_frame_i8")?;
        let _ = self
            .builder
            .build_call(rt_pop, &[frame_i8.into()], "handle_custom_effect_pop")?;

        // 读取 slot（单 payload：word0 + 可选 gc_ref）并清除 flag/slot。
        let rt_len = self.declare_runtime_effect_perform_slot_read_len_words();
        let call = self
            .builder
            .build_call(rt_len, &[], "custom_read_slot_len_words")?;
        let raw = call
            .try_as_basic_value()
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
            "custom_slot_len_ok",
        )?;
        let len_ok_bb = self
            .context
            .append_basic_block(func, "custom_slot_len_ok_bb");
        let len_bad_bb = self
            .context
            .append_basic_block(func, "custom_slot_len_bad_bb");
        self.builder
            .build_conditional_branch(len_ok, len_ok_bb, len_bad_bb)?;

        self.builder.position_at_end(len_bad_bb);
        self.emit_exit_with_code(span, 3)?;

        self.builder.position_at_end(len_ok_bb);

        let rt_read = self.declare_runtime_effect_perform_slot_read_u64();
        let value_call = self
            .builder
            .build_call(rt_read, &[], "custom_read_slot_word0")?;
        let value_raw =
            value_call
                .try_as_basic_value()
                .basic()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "effect slot_read_word0 return value",
                    at: span.into(),
                })?;
        let BasicValueEnum::IntValue(value_u64) = value_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "effect slot_read_word0 return type",
                at: span.into(),
            });
        };
        let rt_read_gc = self.declare_runtime_effect_perform_slot_read_gc_ref();
        let gc_call = self
            .builder
            .build_call(rt_read_gc, &[], "custom_read_slot_gc_ref")?;
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

        // binder scope：arm body 在 handler scope 之外执行（因此不 push effect_unwind_target_stack 的 catch_bb）。
        self.env.push_scope();

        let binder_cg_ty = self
            .cg_ty_of(binder.ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "handle binder type (custom non-resuming)",
                at: binder.span.into(),
            })?;
        let binder_value =
            self.decode_abi_payload_transport(binder.span, value_u64, gc_ref_raw, binder_cg_ty)?;

        let binder_ptr = self.create_entry_alloca(binder.span, &binder.name, binder_cg_ty)?;
        let _stored =
            self.store_local_value(binder.span, binder_ptr, binder_cg_ty, binder_value)?;
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
        let _ = self.builder.build_call(rt_clear, &[], "custom_clear")?;

        // catch body 若再次发生 perform：先执行 finally，再向外传播（不在本 handler 内消费 slot）。
        self.push_effect_unwind_target(&arm.op.op.fqn, finally_unwind_bb);
        let arm_v = self.codegen_expr_in_expected_context(&arm.body, Some(out_ty))?;
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
        let catch_reaches_merge = catch_end.get_terminator().is_none();
        if catch_reaches_merge {
            if let Some(ptr) = result_ptr {
                let _ = self.store_local_value(arm.body.span, ptr, out_ty, arm_v)?;
            }
            self.builder.build_unconditional_branch(finally_bb)?;
        }
        self.env.pop_scope();

        // --- finally_unwind ---
        self.builder.position_at_end(finally_unwind_bb);
        if let Some(finally) = handle.finally.as_ref() {
            let _ = self.codegen_block_value(finally)?;
        }
        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_none()
        {
            if let Some(target) = outer_target {
                // T1608: 跨 effect 传播——清理中间帧后再跳到外层 handler。
                let rt_unwind = self.declare_runtime_effect_handler_stack_unwind_to_tag();
                let _ = self.builder.build_call(
                    rt_unwind,
                    &[op_tag_i32.into()],
                    "effect_unwind_to_tag",
                )?;
                self.builder.build_unconditional_branch(target)?;
            } else {
                // 当前阶段：自定义 effect 在程序边界的处理策略尚未固定；先按运行期错误处理。
                self.emit_exit_with_code(span, 3)?;
            }
        }

        // --- finally ---
        self.builder.position_at_end(finally_bb);
        if let Some(finally) = handle.finally.as_ref() {
            let _ = self.codegen_block_value(finally)?;
        }
        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_none()
        {
            self.builder.build_unconditional_branch(merge_bb)?;
        }

        // --- dispatch trampoline (T1606f-1) ---
        // emit_effect_unwind_if_active 在 body 中的函数调用返回后，若 flag 被设置，
        // 会跳到 dispatch_bb（通过 raise_target_stack）。
        // dispatch_bb 读取 op_tag，判断是否为本 handler 匹配的 effect。
        self.builder.position_at_end(dispatch_bb);
        {
            let rt_read_tag = self.declare_runtime_effect_perform_slot_read_op_tag();
            let tag_call = self
                .builder
                .build_call(rt_read_tag, &[], "dispatch_read_op_tag")?;
            let tag_raw = tag_call.try_as_basic_value().basic().ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "dispatch read_op_tag return value",
                    at: span.into(),
                },
            )?;
            let BasicValueEnum::IntValue(slot_tag) = tag_raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "dispatch read_op_tag return type",
                    at: span.into(),
                });
            };

            let tag_matches = self.builder.build_int_compare(
                IntPredicate::EQ,
                slot_tag,
                op_tag_i32,
                "dispatch_tag_eq",
            )?;
            self.builder
                .build_conditional_branch(tag_matches, catch_bb, dispatch_no_match_bb)?;
        }

        // --- dispatch no match (T1606f-1) ---
        // op_tag 不匹配：pop handler frame，向外传播。
        self.builder.position_at_end(dispatch_no_match_bb);
        {
            let rt_pop = self.declare_runtime_effect_handler_stack_pop();
            let i8_ptr_ty = self.llvm_i8_ptr_type();
            let frame_i8 = self.builder.build_bit_cast(
                handler_frame_ptr,
                i8_ptr_ty,
                "dispatch_nomatch_frame_i8",
            )?;
            let _ = self
                .builder
                .build_call(rt_pop, &[frame_i8.into()], "dispatch_nomatch_pop")?;

            if let Some(outer) = outer_raise_target {
                // 存在外层 raise handler → 传播到外层。
                self.builder.build_unconditional_branch(outer)?;
            } else {
                // 无外层 handler → 返回默认值向外传播（flag 保持 active）。
                let ret_ty =
                    self.current_fun_return_ty
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "dispatch no-match needs function return type",
                            at: span.into(),
                        })?;
                let v = self.default_value(span, ret_ty)?;
                self.emit_return(span, ret_ty, v)?;
            }
        }

        // --- merge ---
        self.builder.position_at_end(merge_bb);

        match out_ty {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
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
                    .build_load(llvm_ty, ptr, "handle_custom_result")?;
                Ok(CgValue {
                    ty: out_ty,
                    value: Some(loaded),
                })
            }
        }
    }

    fn codegen_handle_expr_multi_arm<'hir>(
        &mut self,
        span: crate::span::Span,
        handle: &'hir hir::HandleExpr,
        out_ty: CgTy,
        state_machine_plan: &HandleStateMachinePlan,
        arm_buckets: &HandleArmBuckets<'hir>,
        codegen_entrypoint: SimplifiedCodegenEntrypoint,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let immediate_arms = arm_buckets.immediate_arms.as_slice();
        let escape_arms = arm_buckets.escape_arms.as_slice();
        let nonresuming_arms = arm_buckets.nonresuming_arms.as_slice();

        match codegen_entrypoint {
            SimplifiedCodegenEntrypoint::MultiNonResuming => {
                self.codegen_handle_expr_nonresuming_multi_arm(
                    span,
                    handle,
                    nonresuming_arms,
                    out_ty,
                )
            }
            SimplifiedCodegenEntrypoint::MultipleEscapeTopLevelDirect => {
                self.codegen_handle_expr_multiple_escape_top_level_direct(
                    span,
                    handle,
                    state_machine_plan,
                    escape_arms,
                    nonresuming_arms,
                    out_ty,
                )
            }
            SimplifiedCodegenEntrypoint::MultipleImmediateResumeTopLevel => {
                self.codegen_handle_expr_multiple_immediate_resume_top_level(
                    span,
                    handle,
                    state_machine_plan,
                    immediate_arms,
                    nonresuming_arms,
                    out_ty,
                )
            }
            SimplifiedCodegenEntrypoint::ImmediateResumeWithNonResumingSiblings => {
                let Some((immediate_arm, resume_symbol)) = immediate_arms.first().copied() else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle arm dispatch (missing immediate-resume arm)",
                        at: span.into(),
                    });
                };
                self.codegen_handle_expr_immediate_resume_with_nonresuming_siblings(
                    span,
                    handle,
                    state_machine_plan,
                    (immediate_arm, resume_symbol),
                    nonresuming_arms,
                    out_ty,
                )
            }
            SimplifiedCodegenEntrypoint::EscapeContinuationWithNonResumingSiblings => {
                let Some((escape_arm, continuation_symbol)) = escape_arms.first().copied() else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle arm dispatch (missing escape-continuation arm)",
                        at: span.into(),
                    });
                };
                let Some(escape_arm_id) = handle
                    .arms
                    .iter()
                    .position(|candidate| std::ptr::eq(candidate, escape_arm))
                else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle arm dispatch (escape-continuation arm id)",
                        at: escape_arm.span.into(),
                    });
                };
                self.codegen_handle_expr_escape_with_nonresuming_siblings(
                    span,
                    handle,
                    state_machine_plan,
                    (escape_arm, escape_arm_id as ArmPlanId, continuation_symbol),
                    nonresuming_arms,
                    out_ty,
                )
            }
            SimplifiedCodegenEntrypoint::ImmediateResumeWithEscapeSibling => {
                let Some((immediate_arm, resume_symbol)) = immediate_arms.first().copied() else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle arm dispatch (missing immediate-resume arm)",
                        at: span.into(),
                    });
                };
                let Some((escape_arm, continuation_symbol)) = escape_arms.first().copied() else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle arm dispatch (missing escape-continuation arm)",
                        at: span.into(),
                    });
                };
                self.codegen_handle_expr_immediate_resume_with_escape_sibling(
                    span,
                    handle,
                    state_machine_plan,
                    (immediate_arm, resume_symbol),
                    (escape_arm, continuation_symbol),
                    out_ty,
                )
            }
            SimplifiedCodegenEntrypoint::ImmediateResumeWithEscapeAndNonResumingSiblings => {
                let Some((immediate_arm, resume_symbol)) = immediate_arms.first().copied() else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle arm dispatch (missing immediate-resume arm)",
                        at: span.into(),
                    });
                };
                let Some((escape_arm, continuation_symbol)) = escape_arms.first().copied() else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle arm dispatch (missing escape-continuation arm)",
                        at: span.into(),
                    });
                };
                self.codegen_handle_expr_immediate_resume_with_escape_and_nonresuming_siblings(
                    span,
                    handle,
                    state_machine_plan,
                    (immediate_arm, resume_symbol),
                    (escape_arm, continuation_symbol),
                    nonresuming_arms,
                    out_ty,
                )
            }
            SimplifiedCodegenEntrypoint::UnsupportedMixedMultipleEscapeWithImmediate => {
                let at = escape_arms
                    .get(1)
                    .map(|(arm, _)| arm.span)
                    .unwrap_or(span);
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed multiple escape-continuation arms with immediate-resume not yet supported",
                    at: at.into(),
                })
            }
            SimplifiedCodegenEntrypoint::UnsupportedMixedMultipleImmediateWithEscape => {
                let at = escape_arms
                    .first()
                    .map(|(arm, _)| arm.span)
                    .unwrap_or(span);
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle mixed multiple immediate-resume arms with escape-continuation not yet supported",
                    at: at.into(),
                })
            }
            SimplifiedCodegenEntrypoint::NoSuspendSites
            | SimplifiedCodegenEntrypoint::SingleNonResuming
            | SimplifiedCodegenEntrypoint::SingleImmediateResume
            | SimplifiedCodegenEntrypoint::SingleEscapeContinuation => {
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle multi-arm route mismatch",
                    at: span.into(),
                })
            }
        }
    }

    fn codegen_handle_expr_nonresuming_multi_arm<'hir>(
        &mut self,
        span: crate::span::Span,
        handle: &'hir hir::HandleExpr,
        nonresuming_arms: &[&'hir hir::HandleArm],
        out_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        #[derive(Clone, Copy)]
        struct MultiCustomArm<'hir, 'ctx> {
            arm: &'hir hir::HandleArm,
            frame_ptr: PointerValue<'ctx>,
            catch_bb: inkwell::basic_block::BasicBlock<'ctx>,
            op_tag: u32,
        }

        if nonresuming_arms.is_empty() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle arm count (only 1 supported)",
                at: span.into(),
            });
        }

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

        let body_bb = self
            .context
            .append_basic_block(func, "handle_multi_nonresuming_body");
        let dispatch_bb = self
            .context
            .append_basic_block(func, "handle_multi_nonresuming_dispatch");
        let dispatch_no_match_bb = self
            .context
            .append_basic_block(func, "handle_multi_nonresuming_dispatch_nomatch");
        let finally_bb = self
            .context
            .append_basic_block(func, "handle_multi_nonresuming_finally");
        let finally_unwind_bb = self
            .context
            .append_basic_block(func, "handle_multi_nonresuming_finally_unwind");
        let merge_bb = self
            .context
            .append_basic_block(func, "handle_multi_nonresuming_merge");

        let result_ptr = if out_ty == CgTy::Unit {
            None
        } else {
            Some(self.create_entry_alloca(span, "handle_multi_nonresuming_result", out_ty)?)
        };

        let i32_ty = self.context.i32_type();
        let handler_frame_ty = self.llvm_effect_handler_frame_type();
        let mut raise_arm: Option<(&'hir hir::HandleArm, inkwell::basic_block::BasicBlock<'ctx>)> =
            None;
        let mut custom_arms: Vec<MultiCustomArm<'hir, 'ctx>> = Vec::new();

        for (idx, arm) in nonresuming_arms.iter().enumerate() {
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
                let catch_bb = self
                    .context
                    .append_basic_block(func, "handle_multi_nonresuming_raise_catch");
                raise_arm = Some((arm, catch_bb));
                continue;
            }

            let catch_bb = self.context.append_basic_block(
                func,
                &format!("handle_multi_nonresuming_custom_catch_{idx}"),
            );
            let frame_ptr = self.create_entry_alloca_raw(
                span,
                &format!("handle_multi_nonresuming_custom_frame_{idx}"),
                handler_frame_ty.into(),
            )?;
            custom_arms.push(MultiCustomArm {
                arm,
                frame_ptr,
                catch_bb,
                op_tag: self.effect_op_tag(&arm.op.op.fqn),
            });
        }

        let rt_push = self.declare_runtime_effect_handler_stack_push();
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        for custom in &custom_arms {
            let frame_i8 = self.builder.build_bit_cast(
                custom.frame_ptr,
                i8_ptr_ty,
                "handle_multi_nonresuming_frame_i8",
            )?;
            let tag_i32 = i32_ty.const_int(custom.op_tag as u64, false);
            let _ = self.builder.build_call(
                rt_push,
                &[frame_i8.into(), tag_i32.into()],
                "handle_multi_nonresuming_push",
            )?;
        }

        let custom_outer_top = if let Some(first) = custom_arms.first() {
            let prev_ptr = self.builder.build_struct_gep(
                handler_frame_ty,
                first.frame_ptr,
                0,
                "handle_multi_nonresuming_prev_gep",
            )?;
            Some(
                self.builder
                    .build_load(i8_ptr_ty, prev_ptr, "handle_multi_nonresuming_outer_top")?
                    .into_pointer_value(),
            )
        } else {
            None
        };

        self.builder.build_unconditional_branch(body_bb)?;

        self.builder.position_at_end(body_bb);
        for custom in &custom_arms {
            self.push_effect_unwind_target(&custom.arm.op.op.fqn, custom.catch_bb);
        }
        self.push_raise_target(dispatch_bb);
        let body_v = self.codegen_block_value_in_expected_context(&handle.body, Some(out_ty))?;
        self.pop_raise_target();
        for _ in custom_arms.iter().rev() {
            self.pop_effect_unwind_target();
        }

        let body_v = if out_ty == CgTy::Unit {
            CgValue::unit()
        } else {
            self.coerce_value(handle.body.span, body_v, out_ty)?
        };

        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_none()
        {
            if let Some(custom_outer_top) = custom_outer_top {
                let rt_swap = self.declare_runtime_effect_handler_stack_swap_top();
                let _ = self.builder.build_call(
                    rt_swap,
                    &[custom_outer_top.into()],
                    "handle_multi_nonresuming_body_detach",
                )?;
            }
            if let Some(ptr) = result_ptr {
                let _ = self.store_local_value(handle.body.span, ptr, out_ty, body_v)?;
            }
            self.builder.build_unconditional_branch(finally_bb)?;
        }

        self.builder.position_at_end(dispatch_bb);
        let rt_read_tag = self.declare_runtime_effect_perform_slot_read_op_tag();
        let tag_call = self.builder.build_call(
            rt_read_tag,
            &[],
            "handle_multi_nonresuming_dispatch_read_op_tag",
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
        if let Some((_, catch_bb)) = raise_arm {
            let raise_tag = self.effect_op_tag("scoop.core.Raise.raise");
            dispatch_cases.push((i32_ty.const_int(raise_tag as u64, false), catch_bb));
        }
        for custom in &custom_arms {
            dispatch_cases.push((
                i32_ty.const_int(custom.op_tag as u64, false),
                custom.catch_bb,
            ));
        }
        self.builder
            .build_switch(slot_tag, dispatch_no_match_bb, &dispatch_cases)?;

        self.builder.position_at_end(dispatch_no_match_bb);
        if let Some(custom_outer_top) = custom_outer_top {
            let rt_swap = self.declare_runtime_effect_handler_stack_swap_top();
            let _ = self.builder.build_call(
                rt_swap,
                &[custom_outer_top.into()],
                "handle_multi_nonresuming_dispatch_detach",
            )?;
        }
        self.builder.build_unconditional_branch(finally_unwind_bb)?;

        if let Some((raise_arm, raise_catch_bb)) = raise_arm {
            let binder = &raise_arm.op.binders[0];
            self.builder.position_at_end(raise_catch_bb);

            if let Some(custom_outer_top) = custom_outer_top {
                let rt_swap = self.declare_runtime_effect_handler_stack_swap_top();
                let _ = self.builder.build_call(
                    rt_swap,
                    &[custom_outer_top.into()],
                    "handle_multi_nonresuming_raise_detach",
                )?;
            }

            let rt_len = self.declare_runtime_effect_perform_slot_read_len_words();
            let call = self.builder.build_call(
                rt_len,
                &[],
                "multi_nonresuming_raise_read_slot_len_words",
            )?;
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

            let expected_len = i32_ty.const_int(2, false);
            let len_ok = self.builder.build_int_compare(
                IntPredicate::EQ,
                len_words_i32,
                expected_len,
                "multi_nonresuming_raise_slot_len_ok",
            )?;
            let len_ok_bb = self
                .context
                .append_basic_block(func, "multi_nonresuming_raise_slot_len_ok_bb");
            let len_bad_bb = self
                .context
                .append_basic_block(func, "multi_nonresuming_raise_slot_len_bad_bb");
            self.builder
                .build_conditional_branch(len_ok, len_ok_bb, len_bad_bb)?;

            self.builder.position_at_end(len_bad_bb);
            self.emit_exit_with_code(span, 3)?;

            self.builder.position_at_end(len_ok_bb);
            let rt_read_at = self.declare_runtime_effect_perform_slot_read_u64_at();
            let idx0 = i32_ty.const_int(0, false);
            let idx1 = i32_ty.const_int(1, false);

            let kind_call = self.builder.build_call(
                rt_read_at,
                &[idx0.into()],
                "multi_nonresuming_raise_read_slot_word0",
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
                "multi_nonresuming_raise_read_slot_word1",
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
                .build_call(rt_clear, &[], "multi_nonresuming_raise_clear")?;

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
                        "multi_nonresuming_raise_kind_is_int",
                    )?;
                    let ok_bb = self
                        .context
                        .append_basic_block(func, "multi_nonresuming_raise_kind_int_ok");
                    let bad_bb = self
                        .context
                        .append_basic_block(func, "multi_nonresuming_raise_kind_int_bad");
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
                        "multi_nonresuming_raise_kind_is_runtime_error",
                    )?;
                    let ok_bb = self
                        .context
                        .append_basic_block(func, "multi_nonresuming_raise_kind_runtime_error_ok");
                    let bad_bb = self
                        .context
                        .append_basic_block(func, "multi_nonresuming_raise_kind_runtime_error_bad");
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
                        i32_ty,
                        "multi_nonresuming_raise_runtime_error_tag_i32",
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
                        "multi_nonresuming_raise_runtime_error_tag",
                    )?;
                    agg = self.builder.build_insert_value(
                        agg,
                        payload_word_zero,
                        1,
                        "multi_nonresuming_raise_runtime_error_payload_word",
                    )?;
                    agg = self.builder.build_insert_value(
                        agg,
                        payload_ptr_zero,
                        2,
                        "multi_nonresuming_raise_runtime_error_payload_ptr",
                    )?;
                    CgValue {
                        ty: CgTy::Enum(enum_ty),
                        value: Some(agg.as_basic_value_enum()),
                    }
                }
                _ => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle binder type (Raise payload decode)",
                        at: binder.span.into(),
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

            for custom in &custom_arms {
                self.push_effect_unwind_target(&custom.arm.op.op.fqn, finally_unwind_bb);
            }
            self.push_raise_target(finally_unwind_bb);
            let arm_v = self.codegen_expr_in_expected_context(&raise_arm.body, Some(out_ty))?;
            self.pop_raise_target();
            for _ in custom_arms.iter().rev() {
                self.pop_effect_unwind_target();
            }
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

        for custom in &custom_arms {
            let arm = custom.arm;
            let binder = &arm.op.binders[0];
            self.builder.position_at_end(custom.catch_bb);

            if let Some(custom_outer_top) = custom_outer_top {
                let rt_swap = self.declare_runtime_effect_handler_stack_swap_top();
                let _ = self.builder.build_call(
                    rt_swap,
                    &[custom_outer_top.into()],
                    "handle_multi_nonresuming_custom_detach",
                )?;
            }

            let rt_len = self.declare_runtime_effect_perform_slot_read_len_words();
            let call = self.builder.build_call(
                rt_len,
                &[],
                "multi_nonresuming_custom_read_slot_len_words",
            )?;
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

            let expected_len = i32_ty.const_int(1, false);
            let len_ok = self.builder.build_int_compare(
                IntPredicate::EQ,
                len_words_i32,
                expected_len,
                "multi_nonresuming_custom_slot_len_ok",
            )?;
            let len_ok_bb = self
                .context
                .append_basic_block(func, "multi_nonresuming_custom_slot_len_ok_bb");
            let len_bad_bb = self
                .context
                .append_basic_block(func, "multi_nonresuming_custom_slot_len_bad_bb");
            self.builder
                .build_conditional_branch(len_ok, len_ok_bb, len_bad_bb)?;

            self.builder.position_at_end(len_bad_bb);
            self.emit_exit_with_code(span, 3)?;

            self.builder.position_at_end(len_ok_bb);
            let rt_read = self.declare_runtime_effect_perform_slot_read_u64();
            let value_call = self.builder.build_call(
                rt_read,
                &[],
                "multi_nonresuming_custom_read_slot_word0",
            )?;
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
            let gc_call = self.builder.build_call(
                rt_read_gc,
                &[],
                "multi_nonresuming_custom_read_slot_gc_ref",
            )?;
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
                .build_call(rt_clear, &[], "multi_nonresuming_custom_clear")?;

            for nested_custom in &custom_arms {
                self.push_effect_unwind_target(&nested_custom.arm.op.op.fqn, finally_unwind_bb);
            }
            self.push_raise_target(finally_unwind_bb);
            let arm_v = self.codegen_expr_in_expected_context(&arm.body, Some(out_ty))?;
            self.pop_raise_target();
            for _ in custom_arms.iter().rev() {
                self.pop_effect_unwind_target();
            }
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

        self.builder.position_at_end(finally_unwind_bb);
        if let Some(finally) = handle.finally.as_ref() {
            let _ = self.codegen_block_value(finally)?;
        }
        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_none()
        {
            if let Some(target) = outer_raise_target {
                self.builder.build_unconditional_branch(target)?;
            } else {
                let ret_ty = self.current_fun_return_ty.ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "handle multi nonresuming finally unwind needs function return type",
                        at: span.into(),
                    },
                )?;
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
            self.builder.build_unconditional_branch(merge_bb)?;
        }

        self.builder.position_at_end(merge_bb);

        match out_ty {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
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
                let loaded = self.builder.build_load(
                    llvm_ty,
                    ptr,
                    "handle_multi_nonresuming_result_value",
                )?;
                Ok(CgValue {
                    ty: out_ty,
                    value: Some(loaded),
                })
            }
        }
    }

}
