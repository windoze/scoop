#[derive(Debug, Clone)]
struct MultiResumingEscapeSitePlan<'hir> {
    site: MultiResumingEscapeSiteKind<'hir>,
    arm: &'hir hir::HandleArm,
    continuation_symbol: hir::SymbolId,
    resume_value_ty: CgTy,
    op_tag: u32,
}

impl<'hir> MultiResumingEscapeSitePlan<'hir> {
    fn decl(&self) -> &'hir hir::ValDecl {
        match &self.site {
            MultiResumingEscapeSiteKind::Direct(site) => site.decl,
            MultiResumingEscapeSiteKind::Indirect(site) => site.decl,
        }
    }

    fn top_level_stmt_idx(&self) -> usize {
        match &self.site {
            MultiResumingEscapeSiteKind::Direct(site) => site.top_level_stmt_idx,
            MultiResumingEscapeSiteKind::Indirect(site) => site.top_level_stmt_idx,
        }
    }

    fn id(&self) -> hir::SymbolId {
        match &self.site {
            MultiResumingEscapeSiteKind::Direct(site) => site.id,
            MultiResumingEscapeSiteKind::Indirect(site) => site.id,
        }
    }

    fn resume_path(&self) -> &[MixedEscapeDirectFrame<'hir>] {
        match &self.site {
            MultiResumingEscapeSiteKind::Direct(site) => site.resume_path.as_slice(),
            MultiResumingEscapeSiteKind::Indirect(site) => site.resume_path.as_slice(),
        }
    }

    fn direct_args(&self) -> Option<&'hir [hir::CallArg]> {
        match &self.site {
            MultiResumingEscapeSiteKind::Direct(site) => Some(site.args),
            MultiResumingEscapeSiteKind::Indirect(_) => None,
        }
    }

    fn is_direct(&self) -> bool {
        matches!(self.site, MultiResumingEscapeSiteKind::Direct(_))
    }

    fn is_indirect(&self) -> bool {
        matches!(self.site, MultiResumingEscapeSiteKind::Indirect(_))
    }

    fn same_source_site(&self, other: &Self) -> bool {
        match (&self.site, &other.site) {
            (
                MultiResumingEscapeSiteKind::Direct(a),
                MultiResumingEscapeSiteKind::Direct(b),
            ) => std::ptr::eq(a.decl, b.decl),
            (
                MultiResumingEscapeSiteKind::Indirect(a),
                MultiResumingEscapeSiteKind::Indirect(b),
            ) => std::ptr::eq(a.decl, b.decl),
            _ => false,
        }
    }
}

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    fn mixed_escape_direct_frame_same_structure<'hir>(
        a: &MixedEscapeDirectFrame<'hir>,
        b: &MixedEscapeDirectFrame<'hir>,
    ) -> bool {
        match (a, b) {
            (
                MixedEscapeDirectFrame::IfThen { if_expr: a_e, .. },
                MixedEscapeDirectFrame::IfThen { if_expr: b_e, .. },
            ) => std::ptr::eq(*a_e, *b_e),
            (
                MixedEscapeDirectFrame::IfElse { if_expr: a_e, .. },
                MixedEscapeDirectFrame::IfElse { if_expr: b_e, .. },
            ) => std::ptr::eq(*a_e, *b_e),
            (
                MixedEscapeDirectFrame::WhileBody {
                    while_body: a_b, ..
                },
                MixedEscapeDirectFrame::WhileBody {
                    while_body: b_b, ..
                },
            ) => std::ptr::eq(*a_b, *b_b),
            (
                MixedEscapeDirectFrame::Block { block: a_b, .. },
                MixedEscapeDirectFrame::Block { block: b_b, .. },
            ) => std::ptr::eq(*a_b, *b_b),
            _ => false,
        }
    }

    fn multi_resuming_heap_site_matches_prefix<'hir>(
        site: &MultiResumingEscapeSitePlan<'hir>,
        prefix: &[MixedEscapeDirectFrame<'hir>],
    ) -> bool {
        if site.resume_path().len() < prefix.len() {
            return false;
        }
        prefix.iter().enumerate().all(|(idx, frame)| {
            Self::mixed_escape_direct_frame_same_structure(frame, &site.resume_path()[idx])
        })
    }

    fn multi_resuming_heap_target_stmt_idx<'hir>(
        site: &MultiResumingEscapeSitePlan<'hir>,
        prefix_len: usize,
    ) -> usize {
        if prefix_len == 0 {
            site.top_level_stmt_idx()
        } else {
            site.resume_path()[prefix_len - 1].stmt_idx()
        }
    }

    fn codegen_multi_resuming_heap_stmt_unit(
        &mut self,
        stmt: &hir::Stmt,
    ) -> Result<(), LlvmEmitError> {
        let _ = self.codegen_mixed_escape_tail_stmt(
            stmt,
            "`return` inside multi-resuming heap continuation step",
            "statement inside multi-resuming heap continuation step",
        )?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn codegen_multi_resuming_heap_stmt_unit_with_indirect_dispatch<'hir>(
        &mut self,
        span: crate::span::Span,
        stmt: &'hir hir::Stmt,
        site_indices: &[usize],
        site_plans: &[MultiResumingEscapeSitePlan<'hir>],
        binder_slots_by_site: &[Vec<ImmediateResumeBinderSlot<'ctx>>],
        arm_bbs: &[inkwell::basic_block::BasicBlock<'ctx>],
        step_fn: FunctionValue<'ctx>,
        cont_ptr: PointerValue<'ctx>,
        continuation_created_ptr: Option<PointerValue<'ctx>>,
        state_ty: inkwell::types::StructType<'ctx>,
        state_raw: PointerValue<'ctx>,
        state_ptr: PointerValue<'ctx>,
        outer_visible_supported: &[EscapeCaptureMeta],
        outer_field_base: u32,
        body_visible_supported: &[EscapeCaptureMeta],
        body_field_base: u32,
        pc_field_idx: u32,
    ) -> Result<(), LlvmEmitError> {
        let Some((&first_site_idx, rest_site_indices)) = site_indices.split_first() else {
            return Ok(());
        };
        let first_plan = &site_plans[first_site_idx];
        if !first_plan.is_indirect() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle multi-resuming heap-continuation-only (expected indirect site plan)",
                at: first_plan.decl().span.into(),
            });
        }
        let hir::StmtKind::Val(decl) = &stmt.kind else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle multi-resuming heap-continuation-only (indirect site must be val-bound)",
                at: stmt.span.into(),
            });
        };
        if !std::ptr::eq(decl, first_plan.decl()) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle multi-resuming heap-continuation-only (indirect call binding path mismatch)",
                at: stmt.span.into(),
            });
        }
        for site_idx in rest_site_indices {
            let plan = &site_plans[*site_idx];
            if !plan.is_indirect() || !first_plan.same_source_site(plan) {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle multi-resuming heap-continuation-only (multiple indirect sites in same source statement not yet supported)",
                    at: stmt.span.into(),
                });
            }
        }

        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: stmt.span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: stmt.span.into(),
            })?;
        let dispatch_bb = self
            .context
            .append_basic_block(func, "multi_resume_heap_indirect_dispatch");
        let nomatch_bb = self
            .context
            .append_basic_block(func, "multi_resume_heap_indirect_nomatch");
        let catch_bbs = site_indices
            .iter()
            .map(|site_idx| {
                self.context.append_basic_block(
                    func,
                    &format!("multi_resume_heap_indirect_site_{site_idx}_catch"),
                )
            })
            .collect::<Vec<_>>();
        let outer_raise_target = self.current_raise_target();

        self.push_raise_target(dispatch_bb);
        let stmt_result = self.codegen_multi_resuming_heap_stmt_unit(stmt);
        self.pop_raise_target();
        stmt_result?;
        let normal_cont_bb =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block after indirect site statement",
                    at: stmt.span.into(),
                })?;

        self.builder.position_at_end(dispatch_bb);
        let rt_read_tag = self.declare_runtime_effect_perform_slot_read_op_tag();
        let tag_call = self.builder.build_call(
            rt_read_tag,
            &[],
            "multi_resume_heap_indirect_read_op_tag",
        )?;
        let tag_raw =
            tag_call
                .try_as_basic_value()
                .basic()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi-resuming heap indirect dispatch read_op_tag return value",
                    at: span.into(),
                })?;
        let BasicValueEnum::IntValue(slot_tag) = tag_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "multi-resuming heap indirect dispatch read_op_tag return type",
                at: span.into(),
            });
        };
        let i32_ty = self.context.i32_type();
        let dispatch_cases = site_indices
            .iter()
            .zip(catch_bbs.iter())
            .map(|(site_idx, catch_bb)| {
                (
                    i32_ty.const_int(site_plans[*site_idx].op_tag as u64, false),
                    *catch_bb,
                )
            })
            .collect::<Vec<_>>();
        self.builder.build_switch(slot_tag, nomatch_bb, &dispatch_cases)?;

        for (site_idx, catch_bb) in site_indices.iter().zip(catch_bbs.iter()) {
            let plan = &site_plans[*site_idx];
            self.builder.position_at_end(*catch_bb);
            self.capture_escape_state_with_pc(
                plan.decl().span,
                state_ty,
                state_ptr,
                outer_visible_supported,
                outer_field_base,
                body_visible_supported,
                body_field_base,
                pc_field_idx,
                *site_idx,
            )?;
            if binder_slots_by_site[*site_idx].len() > 1 {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle multi-resuming heap-continuation-only indirect binder count (only 1 supported)",
                    at: plan.arm.op.span.into(),
                });
            }
            if let Some(slot) = binder_slots_by_site[*site_idx].first() {
                let rt_read = self.declare_runtime_effect_perform_slot_read_u64();
                let word_call = self.builder.build_call(
                    rt_read,
                    &[],
                    "multi_resume_heap_indirect_read_binder_word",
                )?;
                let word_raw =
                    word_call
                        .try_as_basic_value()
                        .basic()
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "multi-resuming heap indirect binder word return value",
                            at: plan.arm.op.span.into(),
                        })?;
                let BasicValueEnum::IntValue(word_u64) = word_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "multi-resuming heap indirect binder word return type",
                        at: plan.arm.op.span.into(),
                    });
                };
                let rt_read_gc = self.declare_runtime_effect_perform_slot_read_gc_ref();
                let gc_call = self.builder.build_call(
                    rt_read_gc,
                    &[],
                    "multi_resume_heap_indirect_read_binder_gc",
                )?;
                let gc_raw =
                    gc_call
                        .try_as_basic_value()
                        .basic()
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "multi-resuming heap indirect binder gc return value",
                            at: plan.arm.op.span.into(),
                        })?;
                let BasicValueEnum::PointerValue(gc_ref_raw) = gc_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "multi-resuming heap indirect binder gc return type",
                        at: plan.arm.op.span.into(),
                    });
                };
                let binder_value =
                    self.decode_abi_payload_transport(span, word_u64, gc_ref_raw, slot.ty)?;
                let _ = self.store_local_value(span, slot.ptr, slot.ty, binder_value)?;
            }

            let rt_clear = self.declare_runtime_effect_clear();
            let _ = self.builder.build_call(
                rt_clear,
                &[],
                "multi_resume_heap_indirect_effect_clear",
            )?;

            let step_ptr = step_fn.as_global_value().as_pointer_value();
            let cont_call = self.builder.build_call(
                self.declare_runtime_continuation_alloc(),
                &[state_raw.into(), step_ptr.into()],
                "multi_resume_heap_indirect_cont_alloc",
            )?;
            let cont_raw =
                cont_call
                    .try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "multi-resuming heap indirect continuation alloc return value",
                        at: plan.decl().span.into(),
                    })?;
            let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi-resuming heap indirect continuation alloc return type",
                    at: plan.decl().span.into(),
                });
            };
            let pin = self.declare_runtime_gc_pin();
            let _ = self.builder.build_call(
                pin,
                &[k_raw.into()],
                "multi_resume_heap_indirect_k_pin",
            )?;
            if let Some(continuation_created_ptr) = continuation_created_ptr {
                let _ = self.builder.build_store(
                    continuation_created_ptr,
                    self.context.bool_type().const_all_ones(),
                )?;
            }
            let _ = self.store_local_value(
                span,
                cont_ptr,
                CgTy::Ref,
                CgValue {
                    ty: CgTy::Ref,
                    value: Some(k_raw.into()),
                },
            )?;
            self.builder.build_unconditional_branch(arm_bbs[*site_idx])?;
        }

        self.builder.position_at_end(nomatch_bb);
        if let Some(target) = outer_raise_target {
            self.builder.build_unconditional_branch(target)?;
        } else {
            let unpin = self.declare_runtime_gc_unpin();
            let _ = self.builder.build_call(
                unpin,
                &[state_raw.into()],
                "multi_resume_heap_indirect_nomatch_unpin_state",
            )?;
            let ret_ty = self
                .current_fun_return_ty
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi-resuming heap indirect dispatch unwind needs function return type",
                    at: span.into(),
                })?;
            let v = self.default_value(span, ret_ty)?;
            self.emit_return(span, ret_ty, v)?;
        }
        self.builder.position_at_end(normal_cont_bb);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn codegen_multi_resuming_heap_intercept_site<'hir>(
        &mut self,
        span: crate::span::Span,
        site_idx: usize,
        site_plans: &[MultiResumingEscapeSitePlan<'hir>],
        binder_slots_by_site: &[Vec<ImmediateResumeBinderSlot<'ctx>>],
        arm_bbs: &[inkwell::basic_block::BasicBlock<'ctx>],
        step_fn: FunctionValue<'ctx>,
        cont_ptr: PointerValue<'ctx>,
        continuation_created_ptr: Option<PointerValue<'ctx>>,
        state_ty: inkwell::types::StructType<'ctx>,
        state_raw: PointerValue<'ctx>,
        state_ptr: PointerValue<'ctx>,
        outer_visible_supported: &[EscapeCaptureMeta],
        outer_field_base: u32,
        body_visible_supported: &[EscapeCaptureMeta],
        body_field_base: u32,
        pc_field_idx: u32,
    ) -> Result<(), LlvmEmitError> {
        let plan = &site_plans[site_idx];
        let direct_args = plan.direct_args().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "handle multi-resuming heap-continuation-only (direct perform args missing)",
            at: plan.decl().span.into(),
        })?;
        self.capture_escape_state_with_pc(
            plan.decl().span,
            state_ty,
            state_ptr,
            outer_visible_supported,
            outer_field_base,
            body_visible_supported,
            body_field_base,
            pc_field_idx,
            site_idx,
        )?;
        for (slot, arg) in binder_slots_by_site[site_idx]
            .iter()
            .zip(direct_args.iter())
        {
            let hir::CallArg::Positional(expr) = arg else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle multi-resuming heap named perform arg",
                    at: span.into(),
                });
            };
            let value = self.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
            let _ = self.store_local_value(expr.span, slot.ptr, slot.ty, value)?;
        }
        let step_ptr = step_fn.as_global_value().as_pointer_value();
        let cont_call = self.builder.build_call(
            self.declare_runtime_continuation_alloc(),
            &[state_raw.into(), step_ptr.into()],
            "multi_resume_heap_cont_alloc",
        )?;
        let cont_raw =
            cont_call
                .try_as_basic_value()
                .basic()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi-resuming heap continuation alloc return value",
                    at: plan.decl().span.into(),
                })?;
        let BasicValueEnum::PointerValue(k_raw) = cont_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "multi-resuming heap continuation alloc return type",
                at: plan.decl().span.into(),
            });
        };
        let pin = self.declare_runtime_gc_pin();
        let _ = self
            .builder
            .build_call(pin, &[k_raw.into()], "multi_resume_heap_k_pin")?;
        if let Some(continuation_created_ptr) = continuation_created_ptr {
            let _ = self.builder.build_store(
                continuation_created_ptr,
                self.context.bool_type().const_all_ones(),
            )?;
        }
        let _ = self.store_local_value(
            span,
            cont_ptr,
            CgTy::Ref,
            CgValue {
                ty: CgTy::Ref,
                value: Some(k_raw.into()),
            },
        )?;
        self.builder.build_unconditional_branch(arm_bbs[site_idx])?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn codegen_multi_resuming_heap_block_unit_with_intercepts<'hir>(
        &mut self,
        span: crate::span::Span,
        site_plans: &[MultiResumingEscapeSitePlan<'hir>],
        stmts: &'hir [hir::Stmt],
        prefix: &[MixedEscapeDirectFrame<'hir>],
        stmt_idx_base: usize,
        start_idx: usize,
        binder_slots_by_site: &[Vec<ImmediateResumeBinderSlot<'ctx>>],
        arm_bbs: &[inkwell::basic_block::BasicBlock<'ctx>],
        step_fn: FunctionValue<'ctx>,
        cont_ptr: PointerValue<'ctx>,
        continuation_created_ptr: Option<PointerValue<'ctx>>,
        state_ty: inkwell::types::StructType<'ctx>,
        state_raw: PointerValue<'ctx>,
        state_ptr: PointerValue<'ctx>,
        outer_visible_supported: &[EscapeCaptureMeta],
        outer_field_base: u32,
        body_visible_supported: &[EscapeCaptureMeta],
        body_field_base: u32,
        pc_field_idx: u32,
    ) -> Result<bool, LlvmEmitError> {
        for (local_stmt_idx, stmt) in stmts.iter().enumerate().skip(start_idx) {
            let stmt_idx = stmt_idx_base + local_stmt_idx;
            let mut direct_site_idx: Option<usize> = None;
            let mut indirect_site_indices: Vec<usize> = Vec::new();
            let mut nested_site_indices: Vec<usize> = Vec::new();
            for (site_idx, plan) in site_plans.iter().enumerate() {
                if !Self::multi_resuming_heap_site_matches_prefix(plan, prefix) {
                    continue;
                }
                let target_stmt_idx =
                    Self::multi_resuming_heap_target_stmt_idx(plan, prefix.len());
                if target_stmt_idx != stmt_idx {
                    continue;
                }
                if plan.resume_path().len() == prefix.len() {
                    if plan.is_direct() {
                        if direct_site_idx.replace(site_idx).is_some() {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle multi-resuming heap-continuation-only (multiple direct sites in same source statement not yet supported)",
                                at: stmt.span.into(),
                            });
                        }
                    } else {
                        indirect_site_indices.push(site_idx);
                    }
                } else {
                    nested_site_indices.push(site_idx);
                }
            }

            if let Some(site_idx) = direct_site_idx {
                let plan = &site_plans[site_idx];
                let hir::StmtKind::Val(decl) = &stmt.kind else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle multi-resuming heap-continuation-only (expected direct perform binding)",
                        at: stmt.span.into(),
                    });
                };
                if !std::ptr::eq(decl, plan.decl()) {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle multi-resuming heap-continuation-only (direct perform binding path mismatch)",
                        at: stmt.span.into(),
                    });
                }
                self.codegen_multi_resuming_heap_intercept_site(
                    span,
                    site_idx,
                    site_plans,
                    binder_slots_by_site,
                    arm_bbs,
                    step_fn,
                    cont_ptr,
                    continuation_created_ptr,
                    state_ty,
                    state_raw,
                    state_ptr,
                    outer_visible_supported,
                    outer_field_base,
                    body_visible_supported,
                    body_field_base,
                    pc_field_idx,
                )?;
                return Ok(true);
            }

            if !indirect_site_indices.is_empty() {
                self.codegen_multi_resuming_heap_stmt_unit_with_indirect_dispatch(
                    span,
                    stmt,
                    indirect_site_indices.as_slice(),
                    site_plans,
                    binder_slots_by_site,
                    arm_bbs,
                    step_fn,
                    cont_ptr,
                    continuation_created_ptr,
                    state_ty,
                    state_raw,
                    state_ptr,
                    outer_visible_supported,
                    outer_field_base,
                    body_visible_supported,
                    body_field_base,
                    pc_field_idx,
                )?;
                if let Some(bb) = self.builder.get_insert_block()
                    && bb.get_terminator().is_some()
                {
                    return Ok(true);
                }
                continue;
            }

            if nested_site_indices.is_empty() {
                self.codegen_multi_resuming_heap_stmt_unit(stmt)?;
                if let Some(bb) = self.builder.get_insert_block()
                    && bb.get_terminator().is_some()
                {
                    return Ok(true);
                }
                continue;
            }

            let prefix_len = prefix.len();
            let first_next_frame = site_plans[nested_site_indices[0]].resume_path()[prefix_len];
            match first_next_frame {
                MixedEscapeDirectFrame::Block { block, .. } => {
                    let hir::StmtKind::Expr(expr) = &stmt.kind else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle multi-resuming heap-continuation-only (expected block statement)",
                            at: stmt.span.into(),
                        });
                    };
                    let hir::ExprKind::Block(actual_block) = &expr.kind else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle multi-resuming heap-continuation-only (expected block expression)",
                            at: expr.span.into(),
                        });
                    };
                    if !std::ptr::eq(actual_block, block) {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle multi-resuming heap-continuation-only (block path mismatch)",
                            at: expr.span.into(),
                        });
                    }
                    for site_idx in &nested_site_indices {
                        let frame = site_plans[*site_idx].resume_path()[prefix_len];
                        let MixedEscapeDirectFrame::Block {
                            block: candidate_block,
                            ..
                        } = frame
                        else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle multi-resuming heap-continuation-only (mixed nested source path kinds not yet supported)",
                                at: stmt.span.into(),
                            });
                        };
                        if !std::ptr::eq(candidate_block, block) {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle multi-resuming heap-continuation-only (mixed nested block paths not yet supported)",
                                at: stmt.span.into(),
                            });
                        }
                    }
                    let saved_env = self.env.clone();
                    self.env.push_scope();
                    let mut next_prefix = prefix.to_vec();
                    next_prefix.push(MixedEscapeDirectFrame::Block { block, stmt_idx });
                    let terminated = self.codegen_multi_resuming_heap_block_unit_with_intercepts(
                        span,
                        site_plans,
                        &block.stmts,
                        next_prefix.as_slice(),
                        0,
                        0,
                        binder_slots_by_site,
                        arm_bbs,
                        step_fn,
                        cont_ptr,
                        continuation_created_ptr,
                        state_ty,
                        state_raw,
                        state_ptr,
                        outer_visible_supported,
                        outer_field_base,
                        body_visible_supported,
                        body_field_base,
                        pc_field_idx,
                    )?;
                    self.env = saved_env;
                    if terminated {
                        return Ok(true);
                    }
                }
                MixedEscapeDirectFrame::IfThen { if_expr, .. }
                | MixedEscapeDirectFrame::IfElse { if_expr, .. } => {
                    let hir::StmtKind::Expr(expr) = &stmt.kind else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle multi-resuming heap-continuation-only (expected if statement)",
                            at: stmt.span.into(),
                        });
                    };
                    if !std::ptr::eq(expr, if_expr) {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle multi-resuming heap-continuation-only (if path mismatch)",
                            at: stmt.span.into(),
                        });
                    }
                    let hir::ExprKind::If {
                        cond,
                        then_branch,
                        else_branch,
                    } = &expr.kind
                    else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle multi-resuming heap-continuation-only (expected if expression)",
                            at: expr.span.into(),
                        });
                    };
                    let cond_v = self.codegen_expr_in_expected_context(cond, Some(CgTy::Bool))?;
                    let cond_v = self.coerce_value(cond.span, cond_v, CgTy::Bool)?;
                    let cond_i1 = cond_v.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle multi-resuming heap-continuation-only if condition value",
                        at: cond.span.into(),
                    })?;
                    let insert_block = self
                        .builder
                        .get_insert_block()
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "builder has no insert block",
                            at: stmt.span.into(),
                        })?;
                    let func = insert_block.get_parent().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "builder has no parent function",
                            at: stmt.span.into(),
                        },
                    )?;
                    let then_bb = self.context.append_basic_block(func, "multi_resume_heap_if_then");
                    let after_bb =
                        self.context
                            .append_basic_block(func, "multi_resume_heap_if_after");
                    let else_bb = if else_branch.is_some() {
                        Some(
                            self.context
                                .append_basic_block(func, "multi_resume_heap_if_else"),
                        )
                    } else {
                        None
                    };
                    self.builder.build_conditional_branch(
                        cond_i1,
                        then_bb,
                        else_bb.unwrap_or(after_bb),
                    )?;

                    let saved_env = self.env.clone();
                    let mut then_prefix = prefix.to_vec();
                    let then_block = match &then_branch.kind {
                        hir::ExprKind::Block(block) => block,
                        _ => {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle multi-resuming heap-continuation-only (expected if-then block)",
                                at: then_branch.span.into(),
                            });
                        }
                    };
                    then_prefix.push(MixedEscapeDirectFrame::IfThen {
                        if_expr: expr,
                        then_block,
                        stmt_idx,
                    });
                    self.builder.position_at_end(then_bb);
                    self.env = saved_env.clone();
                    self.env.push_scope();
                    let _ = self.codegen_multi_resuming_heap_block_unit_with_intercepts(
                        span,
                        site_plans,
                        &then_block.stmts,
                        then_prefix.as_slice(),
                        0,
                        0,
                        binder_slots_by_site,
                        arm_bbs,
                        step_fn,
                        cont_ptr,
                        continuation_created_ptr,
                        state_ty,
                        state_raw,
                        state_ptr,
                        outer_visible_supported,
                        outer_field_base,
                        body_visible_supported,
                        body_field_base,
                        pc_field_idx,
                    )?;
                    self.env = saved_env.clone();
                    if let Some(bb) = self.builder.get_insert_block()
                        && bb.get_terminator().is_none()
                    {
                        self.builder.build_unconditional_branch(after_bb)?;
                    }

                    if let Some(else_bb) = else_bb {
                        let else_expr = else_branch.as_deref().ok_or(
                            LlvmEmitError::UnsupportedMainBody {
                                kind: "handle multi-resuming heap-continuation-only (missing if-else branch)",
                                at: expr.span.into(),
                            },
                        )?;
                        let else_block = match &else_expr.kind {
                            hir::ExprKind::Block(block) => block,
                            _ => {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "handle multi-resuming heap-continuation-only (expected if-else block)",
                                    at: else_expr.span.into(),
                                });
                            }
                        };
                        let mut else_prefix = prefix.to_vec();
                        else_prefix.push(MixedEscapeDirectFrame::IfElse {
                            if_expr: expr,
                            else_block,
                            stmt_idx,
                        });
                        self.builder.position_at_end(else_bb);
                        self.env = saved_env.clone();
                        self.env.push_scope();
                        let _ = self.codegen_multi_resuming_heap_block_unit_with_intercepts(
                            span,
                            site_plans,
                            &else_block.stmts,
                            else_prefix.as_slice(),
                            0,
                            0,
                            binder_slots_by_site,
                            arm_bbs,
                            step_fn,
                            cont_ptr,
                            continuation_created_ptr,
                            state_ty,
                            state_raw,
                            state_ptr,
                            outer_visible_supported,
                            outer_field_base,
                            body_visible_supported,
                            body_field_base,
                            pc_field_idx,
                        )?;
                        self.env = saved_env.clone();
                        if let Some(bb) = self.builder.get_insert_block()
                            && bb.get_terminator().is_none()
                        {
                            self.builder.build_unconditional_branch(after_bb)?;
                        }
                    }

                    self.builder.position_at_end(after_bb);
                    self.env = saved_env;
                }
                MixedEscapeDirectFrame::WhileBody {
                    while_cond,
                    while_body,
                    ..
                } => {
                    let hir::StmtKind::While { cond, body } = &stmt.kind else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle multi-resuming heap-continuation-only (expected while statement)",
                            at: stmt.span.into(),
                        });
                    };
                    if !std::ptr::eq(cond, while_cond) || !std::ptr::eq(body, while_body) {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle multi-resuming heap-continuation-only (while path mismatch)",
                            at: stmt.span.into(),
                        });
                    }
                    let insert_block = self
                        .builder
                        .get_insert_block()
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "builder has no insert block",
                            at: stmt.span.into(),
                        })?;
                    let func = insert_block.get_parent().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "builder has no parent function",
                            at: stmt.span.into(),
                        },
                    )?;
                    let cond_bb =
                        self.context
                            .append_basic_block(func, "multi_resume_heap_while_cond");
                    let body_bb =
                        self.context
                            .append_basic_block(func, "multi_resume_heap_while_body");
                    let after_bb =
                        self.context
                            .append_basic_block(func, "multi_resume_heap_while_after");
                    self.builder.build_unconditional_branch(cond_bb)?;

                    self.builder.position_at_end(cond_bb);
                    let cond_v = self.codegen_expr_in_expected_context(cond, Some(CgTy::Bool))?;
                    let cond_v = self.coerce_value(cond.span, cond_v, CgTy::Bool)?;
                    let cond_i1 = cond_v.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle multi-resuming heap-continuation-only while condition value",
                        at: cond.span.into(),
                    })?;
                    self.builder
                        .build_conditional_branch(cond_i1, body_bb, after_bb)?;

                    let saved_env = self.env.clone();
                    let mut next_prefix = prefix.to_vec();
                    next_prefix.push(MixedEscapeDirectFrame::WhileBody {
                        while_cond: cond,
                        while_body: body,
                        stmt_idx,
                    });
                    self.builder.position_at_end(body_bb);
                    self.env = saved_env.clone();
                    self.env.push_scope();
                    let _ = self.codegen_multi_resuming_heap_block_unit_with_intercepts(
                        span,
                        site_plans,
                        &body.stmts,
                        next_prefix.as_slice(),
                        0,
                        0,
                        binder_slots_by_site,
                        arm_bbs,
                        step_fn,
                        cont_ptr,
                        continuation_created_ptr,
                        state_ty,
                        state_raw,
                        state_ptr,
                        outer_visible_supported,
                        outer_field_base,
                        body_visible_supported,
                        body_field_base,
                        pc_field_idx,
                    )?;
                    self.env = saved_env.clone();
                    if let Some(bb) = self.builder.get_insert_block()
                        && bb.get_terminator().is_none()
                    {
                        self.builder.build_unconditional_branch(cond_bb)?;
                    }

                    self.builder.position_at_end(after_bb);
                    self.env = saved_env;
                }
            }

            if let Some(bb) = self.builder.get_insert_block()
                && bb.get_terminator().is_some()
            {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn codegen_multi_resuming_heap_finish_step_without_more_sites(
        &mut self,
        _span: crate::span::Span,
        state_raw: PointerValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_some()
        {
            return Ok(());
        }
        let unpin = self.declare_runtime_gc_unpin();
        let _ = self.builder.build_call(
            unpin,
            &[state_raw.into()],
            "multi_resume_heap_step_state_unpin",
        )?;
        self.builder.build_return(None)?;
        Ok(())
    }

    fn codegen_multi_resuming_heap_step_bind_resumed_site<'hir>(
        &mut self,
        _span: crate::span::Span,
        plan: &MultiResumingEscapeSitePlan<'hir>,
        resume_word: IntValue<'ctx>,
        resume_gc_ref: PointerValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let target_ptr = if let Some(local) = self.env.get(plan.id()) {
            if local.ty != plan.resume_value_ty {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle multi-resuming heap-continuation-only perform value type mismatch",
                    at: plan.decl().span.into(),
                });
            }
            local.ptr
        } else {
            let name = plan.decl().name.as_deref().unwrap_or("resume_value");
            let ptr = self.create_entry_alloca(plan.decl().span, name, plan.resume_value_ty)?;
            self.env.insert(
                plan.id(),
                CgLocal {
                    hir_ty: Some(plan.decl().ty),
                    ty: plan.resume_value_ty,
                    ptr,
                    mutable: plan.decl().mutable,
                },
            );
            ptr
        };

        match &plan.site {
            MultiResumingEscapeSiteKind::Direct(_) => {
                let resume_value = self.decode_abi_payload_transport(
                    plan.decl().span,
                    resume_word,
                    resume_gc_ref,
                    plan.resume_value_ty,
                )?;
                let _ = self.store_local_value(
                    plan.decl().span,
                    target_ptr,
                    plan.resume_value_ty,
                    resume_value,
                )?;
            }
            MultiResumingEscapeSiteKind::Indirect(site) => {
                let rt_get_callee = self.declare_runtime_callee_suspend_state_get();
                let get_call = self
                    .builder
                    .build_call(rt_get_callee, &[], "multi_resume_heap_step_callee_state_get")?;
                let callee_state_raw = get_call
                    .try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "multi-resuming heap step callee_state_get return",
                        at: plan.decl().span.into(),
                    })?
                    .into_pointer_value();
                let callee_prefix_ty = self.llvm_callee_suspend_state_prefix_type();
                let callee_state_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
                let callee_state_ptr = self.builder.build_pointer_cast(
                    callee_state_raw,
                    callee_state_ptr_ty,
                    "multi_resume_heap_step_callee_state_typed",
                )?;
                let callee_rw_ptr = self.builder.build_struct_gep(
                    callee_prefix_ty,
                    callee_state_ptr,
                    1,
                    "multi_resume_heap_step_callee_resume_word_gep",
                )?;
                let _ = self.builder.build_store(callee_rw_ptr, resume_word)?;
                let callee_rg_ptr = self.builder.build_struct_gep(
                    callee_prefix_ty,
                    callee_state_ptr,
                    2,
                    "multi_resume_heap_step_callee_resume_gc_ref_gep",
                )?;
                let i8_ptr_ty = self.llvm_i8_ptr_type();
                let slot_addr = self.builder.build_pointer_cast(
                    callee_rg_ptr,
                    i8_ptr_ty,
                    "multi_resume_heap_step_callee_resume_gc_slot",
                )?;
                let wb = self.declare_runtime_gc_write_barrier();
                let _ = self.builder.build_call(
                    wb,
                    &[slot_addr.into(), resume_gc_ref.into()],
                    "multi_resume_heap_step_callee_resume_gc_store",
                )?;

                let call_result =
                    self.codegen_expr_in_expected_context(site.init, Some(plan.resume_value_ty))?;
                let _ = self.store_local_value(
                    site.init.span,
                    target_ptr,
                    plan.resume_value_ty,
                    call_result,
                )?;
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn codegen_multi_resuming_heap_step_continue_after_completed_frame<'hir>(
        &mut self,
        span: crate::span::Span,
        handle: &'hir hir::HandleExpr,
        current_site_idx: usize,
        completed_depth: usize,
        site_plans: &[MultiResumingEscapeSitePlan<'hir>],
        binder_slots_by_site: &[Vec<ImmediateResumeBinderSlot<'ctx>>],
        arm_bbs: &[inkwell::basic_block::BasicBlock<'ctx>],
        step_fn: FunctionValue<'ctx>,
        cont_ptr: PointerValue<'ctx>,
        state_ty: inkwell::types::StructType<'ctx>,
        state_raw: PointerValue<'ctx>,
        state_ptr: PointerValue<'ctx>,
        outer_visible_supported: &[EscapeCaptureMeta],
        outer_field_base: u32,
        body_visible_supported: &[EscapeCaptureMeta],
        body_field_base: u32,
        pc_field_idx: u32,
    ) -> Result<(), LlvmEmitError> {
        let site = &site_plans[current_site_idx];
        if completed_depth == 0 {
            let saved_env = self.env.clone();
            let _ = self.codegen_multi_resuming_heap_block_unit_with_intercepts(
                span,
                site_plans,
                &handle.body.stmts,
                &[],
                0,
                site.top_level_stmt_idx() + 1,
                binder_slots_by_site,
                arm_bbs,
                step_fn,
                cont_ptr,
                None,
                state_ty,
                state_raw,
                state_ptr,
                outer_visible_supported,
                outer_field_base,
                body_visible_supported,
                body_field_base,
                pc_field_idx,
            )?;
            self.env = saved_env;
            return self.codegen_multi_resuming_heap_finish_step_without_more_sites(span, state_raw);
        }

        let parent_depth = completed_depth - 1;
        let start_idx = site.resume_path()[parent_depth].stmt_idx() + 1;
        match &site.resume_path()[parent_depth] {
            MixedEscapeDirectFrame::WhileBody {
                while_cond,
                while_body,
                ..
            } => self.codegen_multi_resuming_heap_step_continue_while_tail(
                span,
                handle,
                current_site_idx,
                parent_depth,
                start_idx,
                while_cond,
                while_body,
                site_plans,
                binder_slots_by_site,
                arm_bbs,
                step_fn,
                cont_ptr,
                state_ty,
                state_raw,
                state_ptr,
                outer_visible_supported,
                outer_field_base,
                body_visible_supported,
                body_field_base,
                pc_field_idx,
            ),
            MixedEscapeDirectFrame::Block { .. }
            | MixedEscapeDirectFrame::IfThen { .. }
            | MixedEscapeDirectFrame::IfElse { .. } => {
                self.codegen_multi_resuming_heap_step_continue_frame_tail(
                    span,
                    handle,
                    current_site_idx,
                    parent_depth,
                    start_idx,
                    site_plans,
                    binder_slots_by_site,
                    arm_bbs,
                    step_fn,
                    cont_ptr,
                    state_ty,
                    state_raw,
                    state_ptr,
                    outer_visible_supported,
                    outer_field_base,
                    body_visible_supported,
                    body_field_base,
                    pc_field_idx,
                )
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn codegen_multi_resuming_heap_step_continue_frame_tail<'hir>(
        &mut self,
        span: crate::span::Span,
        handle: &'hir hir::HandleExpr,
        current_site_idx: usize,
        depth: usize,
        start_idx: usize,
        site_plans: &[MultiResumingEscapeSitePlan<'hir>],
        binder_slots_by_site: &[Vec<ImmediateResumeBinderSlot<'ctx>>],
        arm_bbs: &[inkwell::basic_block::BasicBlock<'ctx>],
        step_fn: FunctionValue<'ctx>,
        cont_ptr: PointerValue<'ctx>,
        state_ty: inkwell::types::StructType<'ctx>,
        state_raw: PointerValue<'ctx>,
        state_ptr: PointerValue<'ctx>,
        outer_visible_supported: &[EscapeCaptureMeta],
        outer_field_base: u32,
        body_visible_supported: &[EscapeCaptureMeta],
        body_field_base: u32,
        pc_field_idx: u32,
    ) -> Result<(), LlvmEmitError> {
        let site = &site_plans[current_site_idx];
        let stmts = match &site.resume_path()[depth] {
            MixedEscapeDirectFrame::Block { block, .. } => &block.stmts,
            MixedEscapeDirectFrame::IfThen { then_block, .. } => &then_block.stmts,
            MixedEscapeDirectFrame::IfElse { else_block, .. } => &else_block.stmts,
            MixedEscapeDirectFrame::WhileBody { while_body, .. } => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle multi-resuming heap-continuation-only (while frame needs specialized lowering)",
                    at: while_body.span.into(),
                });
            }
        };
        let saved_env = self.env.clone();
        let prefix = site.resume_path()[..=depth].to_vec();
        self.env.push_scope();
        let _ = self.codegen_multi_resuming_heap_block_unit_with_intercepts(
            span,
            site_plans,
            stmts,
            prefix.as_slice(),
            0,
            start_idx,
            binder_slots_by_site,
            arm_bbs,
            step_fn,
            cont_ptr,
            None,
            state_ty,
            state_raw,
            state_ptr,
            outer_visible_supported,
            outer_field_base,
            body_visible_supported,
            body_field_base,
            pc_field_idx,
        )?;
        self.env = saved_env;
        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_none()
        {
            self.codegen_multi_resuming_heap_step_continue_after_completed_frame(
                span,
                handle,
                current_site_idx,
                depth,
                site_plans,
                binder_slots_by_site,
                arm_bbs,
                step_fn,
                cont_ptr,
                state_ty,
                state_raw,
                state_ptr,
                outer_visible_supported,
                outer_field_base,
                body_visible_supported,
                body_field_base,
                pc_field_idx,
            )?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn codegen_multi_resuming_heap_step_continue_while_tail<'hir>(
        &mut self,
        span: crate::span::Span,
        handle: &'hir hir::HandleExpr,
        current_site_idx: usize,
        depth: usize,
        start_idx: usize,
        while_cond: &'hir hir::Expr,
        while_body: &'hir hir::Block,
        site_plans: &[MultiResumingEscapeSitePlan<'hir>],
        binder_slots_by_site: &[Vec<ImmediateResumeBinderSlot<'ctx>>],
        arm_bbs: &[inkwell::basic_block::BasicBlock<'ctx>],
        step_fn: FunctionValue<'ctx>,
        cont_ptr: PointerValue<'ctx>,
        state_ty: inkwell::types::StructType<'ctx>,
        state_raw: PointerValue<'ctx>,
        state_ptr: PointerValue<'ctx>,
        outer_visible_supported: &[EscapeCaptureMeta],
        outer_field_base: u32,
        body_visible_supported: &[EscapeCaptureMeta],
        body_field_base: u32,
        pc_field_idx: u32,
    ) -> Result<(), LlvmEmitError> {
        let prefix = site_plans[current_site_idx].resume_path()[..=depth].to_vec();
        let insert_block = self
            .builder
            .get_insert_block()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no insert block",
                at: while_body.span.into(),
            })?;
        let func = insert_block.get_parent().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "builder has no parent function",
            at: while_body.span.into(),
        })?;
        let tail_bb = self
            .context
            .append_basic_block(func, "multi_resume_heap_step_while_tail");
        let cond_bb = self
            .context
            .append_basic_block(func, "multi_resume_heap_step_while_cond");
        let body_bb = self
            .context
            .append_basic_block(func, "multi_resume_heap_step_while_body");
        let after_bb = self
            .context
            .append_basic_block(func, "multi_resume_heap_step_while_after");
        let saved_env = self.env.clone();

        self.builder.build_unconditional_branch(tail_bb)?;

        self.builder.position_at_end(tail_bb);
        self.env = saved_env.clone();
        self.env.push_scope();
        let _ = self.codegen_multi_resuming_heap_block_unit_with_intercepts(
            span,
            site_plans,
            &while_body.stmts,
            prefix.as_slice(),
            0,
            start_idx,
            binder_slots_by_site,
            arm_bbs,
            step_fn,
            cont_ptr,
            None,
            state_ty,
            state_raw,
            state_ptr,
            outer_visible_supported,
            outer_field_base,
            body_visible_supported,
            body_field_base,
            pc_field_idx,
        )?;
        self.env = saved_env.clone();
        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_none()
        {
            self.builder.build_unconditional_branch(cond_bb)?;
        }

        self.builder.position_at_end(cond_bb);
        let cond_v = self.codegen_expr_in_expected_context(while_cond, Some(CgTy::Bool))?;
        let cond_v = self.coerce_value(while_cond.span, cond_v, CgTy::Bool)?;
        let cond_i1 = cond_v.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "handle multi-resuming heap-continuation-only while condition value",
            at: while_cond.span.into(),
        })?;
        self.builder
            .build_conditional_branch(cond_i1, body_bb, after_bb)?;

        self.builder.position_at_end(body_bb);
        self.env = saved_env.clone();
        self.env.push_scope();
        let _ = self.codegen_multi_resuming_heap_block_unit_with_intercepts(
            span,
            site_plans,
            &while_body.stmts,
            prefix.as_slice(),
            0,
            0,
            binder_slots_by_site,
            arm_bbs,
            step_fn,
            cont_ptr,
            None,
            state_ty,
            state_raw,
            state_ptr,
            outer_visible_supported,
            outer_field_base,
            body_visible_supported,
            body_field_base,
            pc_field_idx,
        )?;
        self.env = saved_env.clone();
        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_none()
        {
            self.builder.build_unconditional_branch(cond_bb)?;
        }

        self.builder.position_at_end(after_bb);
        self.env = saved_env;
        self.codegen_multi_resuming_heap_step_continue_after_completed_frame(
            span,
            handle,
            current_site_idx,
            depth,
            site_plans,
            binder_slots_by_site,
            arm_bbs,
            step_fn,
            cont_ptr,
            state_ty,
            state_raw,
            state_ptr,
            outer_visible_supported,
            outer_field_base,
            body_visible_supported,
            body_field_base,
            pc_field_idx,
        )?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn codegen_multi_resuming_heap_step_continue_after_site<'hir>(
        &mut self,
        span: crate::span::Span,
        handle: &'hir hir::HandleExpr,
        current_site_idx: usize,
        site_plans: &[MultiResumingEscapeSitePlan<'hir>],
        binder_slots_by_site: &[Vec<ImmediateResumeBinderSlot<'ctx>>],
        arm_bbs: &[inkwell::basic_block::BasicBlock<'ctx>],
        step_fn: FunctionValue<'ctx>,
        cont_ptr: PointerValue<'ctx>,
        state_ty: inkwell::types::StructType<'ctx>,
        state_raw: PointerValue<'ctx>,
        state_ptr: PointerValue<'ctx>,
        outer_visible_supported: &[EscapeCaptureMeta],
        outer_field_base: u32,
        body_visible_supported: &[EscapeCaptureMeta],
        body_field_base: u32,
        pc_field_idx: u32,
    ) -> Result<(), LlvmEmitError> {
        let site = &site_plans[current_site_idx];
        if site.resume_path().is_empty() {
            let saved_env = self.env.clone();
            let _ = self.codegen_multi_resuming_heap_block_unit_with_intercepts(
                span,
                site_plans,
                &handle.body.stmts,
                &[],
                0,
                site.top_level_stmt_idx() + 1,
                binder_slots_by_site,
                arm_bbs,
                step_fn,
                cont_ptr,
                None,
                state_ty,
                state_raw,
                state_ptr,
                outer_visible_supported,
                outer_field_base,
                body_visible_supported,
                body_field_base,
                pc_field_idx,
            )?;
            self.env = saved_env;
            return self.codegen_multi_resuming_heap_finish_step_without_more_sites(span, state_raw);
        }

        let last_depth = site.resume_path().len() - 1;
        let start_idx = site.resume_path()[last_depth].stmt_idx() + 1;
        match &site.resume_path()[last_depth] {
            MixedEscapeDirectFrame::WhileBody {
                while_cond,
                while_body,
                ..
            } => self.codegen_multi_resuming_heap_step_continue_while_tail(
                span,
                handle,
                current_site_idx,
                last_depth,
                start_idx,
                while_cond,
                while_body,
                site_plans,
                binder_slots_by_site,
                arm_bbs,
                step_fn,
                cont_ptr,
                state_ty,
                state_raw,
                state_ptr,
                outer_visible_supported,
                outer_field_base,
                body_visible_supported,
                body_field_base,
                pc_field_idx,
            ),
            MixedEscapeDirectFrame::Block { .. }
            | MixedEscapeDirectFrame::IfThen { .. }
            | MixedEscapeDirectFrame::IfElse { .. } => {
                self.codegen_multi_resuming_heap_step_continue_frame_tail(
                    span,
                    handle,
                    current_site_idx,
                    last_depth,
                    start_idx,
                    site_plans,
                    binder_slots_by_site,
                    arm_bbs,
                    step_fn,
                    cont_ptr,
                    state_ty,
                    state_raw,
                    state_ptr,
                    outer_visible_supported,
                    outer_field_base,
                    body_visible_supported,
                    body_field_base,
                    pc_field_idx,
                )
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn codegen_multi_resuming_heap_main_body_with_intercepts<'hir>(
        &mut self,
        span: crate::span::Span,
        handle: &'hir hir::HandleExpr,
        out_ty: CgTy,
        result_ptr: Option<PointerValue<'ctx>>,
        completion_target: inkwell::basic_block::BasicBlock<'ctx>,
        site_plans: &[MultiResumingEscapeSitePlan<'hir>],
        binder_slots_by_site: &[Vec<ImmediateResumeBinderSlot<'ctx>>],
        arm_bbs: &[inkwell::basic_block::BasicBlock<'ctx>],
        step_fn: FunctionValue<'ctx>,
        cont_ptr: PointerValue<'ctx>,
        continuation_created_ptr: PointerValue<'ctx>,
        state_ty: inkwell::types::StructType<'ctx>,
        state_raw: PointerValue<'ctx>,
        state_ptr: PointerValue<'ctx>,
        outer_visible_supported: &[EscapeCaptureMeta],
        outer_field_base: u32,
        body_visible_supported: &[EscapeCaptureMeta],
        body_field_base: u32,
        pc_field_idx: u32,
    ) -> Result<(), LlvmEmitError> {
        if handle.body.stmts.is_empty() {
            if let Some(ptr) = result_ptr
                && out_ty != CgTy::Unit
                && out_ty != CgTy::Never
            {
                let _ = self.store_local_value(span, ptr, out_ty, CgValue::unit())?;
            }
            self.builder.build_unconditional_branch(completion_target)?;
            return Ok(());
        }

        let last_stmt_idx = handle.body.stmts.len() - 1;
        for (stmt_idx, stmt) in handle.body.stmts.iter().take(last_stmt_idx).enumerate() {
            let _ = self.codegen_multi_resuming_heap_block_unit_with_intercepts(
                span,
                site_plans,
                std::slice::from_ref(stmt),
                &[],
                stmt_idx,
                0,
                binder_slots_by_site,
                arm_bbs,
                step_fn,
                cont_ptr,
                Some(continuation_created_ptr),
                state_ty,
                state_raw,
                state_ptr,
                outer_visible_supported,
                outer_field_base,
                body_visible_supported,
                body_field_base,
                pc_field_idx,
            )?;
            if let Some(bb) = self.builder.get_insert_block()
                && bb.get_terminator().is_some()
            {
                return Ok(());
            }
        }

        let final_stmt = &handle.body.stmts[last_stmt_idx];
        let mut final_stmt_has_site = false;
        let mut final_stmt_has_nested_site = false;
        for plan in site_plans {
            if !Self::multi_resuming_heap_site_matches_prefix(plan, &[]) {
                continue;
            }
            if plan.top_level_stmt_idx() != last_stmt_idx {
                continue;
            }
            final_stmt_has_site = true;
            if !plan.resume_path().is_empty() {
                final_stmt_has_nested_site = true;
                break;
            }
        }

        if final_stmt_has_site {
            if final_stmt_has_nested_site {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle multi-resuming heap-continuation-only (nested source-path in tail expression not yet supported)",
                    at: final_stmt.span.into(),
                });
            }
            let _ = self.codegen_multi_resuming_heap_block_unit_with_intercepts(
                span,
                site_plans,
                std::slice::from_ref(final_stmt),
                &[],
                last_stmt_idx,
                0,
                binder_slots_by_site,
                arm_bbs,
                step_fn,
                cont_ptr,
                Some(continuation_created_ptr),
                state_ty,
                state_raw,
                state_ptr,
                outer_visible_supported,
                outer_field_base,
                body_visible_supported,
                body_field_base,
                pc_field_idx,
            )?;
            return Ok(());
        }

        let final_value = match &final_stmt.kind {
            hir::StmtKind::Empty => CgValue::unit(),
            hir::StmtKind::Val(decl) => {
                self.codegen_val_decl(decl)?;
                CgValue::unit()
            }
            hir::StmtKind::Assign { lhs, eq_span, rhs } => {
                self.codegen_assign_stmt(*eq_span, lhs, rhs)?;
                CgValue::unit()
            }
            hir::StmtKind::Expr(expr) => self.codegen_expr_in_expected_context(expr, Some(out_ty))?,
            hir::StmtKind::While { cond, body } => {
                self.codegen_while_stmt(final_stmt.span, cond, body)?;
                CgValue::unit()
            }
            hir::StmtKind::Return { .. } => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "`return` inside multi-resuming heap continuation body",
                    at: final_stmt.span.into(),
                });
            }
            hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Todo(_) => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "statement inside multi-resuming heap continuation body",
                    at: final_stmt.span.into(),
                });
            }
        };

        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_none()
        {
            let final_value = match out_ty {
                CgTy::Unit => CgValue::unit(),
                CgTy::Never => CgValue::never(),
                _ => self.coerce_value(final_stmt.span, final_value, out_ty)?,
            };
            if let Some(ptr) = result_ptr {
                let _ = self.store_local_value(final_stmt.span, ptr, out_ty, final_value)?;
            }
            self.builder.build_unconditional_branch(completion_target)?;
        }
        Ok(())
    }
    fn build_multi_resuming_escape_binder_slots<'hir>(
        &mut self,
        arm: &'hir hir::HandleArm,
        name_prefix: &str,
    ) -> Result<Vec<ImmediateResumeBinderSlot<'ctx>>, LlvmEmitError> {
        let mut slots = Vec::with_capacity(arm.op.binders.len());
        for (idx, binder) in arm.op.binders.iter().enumerate() {
            let binder_ty =
                self.cg_ty_of(binder.ty)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle multi-resuming heap-continuation-only binder type",
                        at: binder.span.into(),
                    })?;
            let slot_name = format!("{name_prefix}_{idx}_{}", binder.name);
            let ptr = self.create_entry_alloca(binder.span, &slot_name, binder_ty)?;
            slots.push(ImmediateResumeBinderSlot {
                id: binder.id,
                hir_ty: binder.ty,
                ty: binder_ty,
                ptr,
            });
        }
        Ok(slots)
    }

    fn codegen_handle_expr_unified_heap_continuation_only_multi_resuming_leaf<'hir>(
        &mut self,
        span: crate::span::Span,
        handle: &'hir hir::HandleExpr,
        state_machine_plan: &HandleStateMachinePlan,
        escape_arms: &[(&'hir hir::HandleArm, hir::SymbolId)],
        sibling_nonresuming_arms: &[&'hir hir::HandleArm],
        out_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let sibling_plan = self.collect_sibling_nonresuming_plan(sibling_nonresuming_arms)?;
        let raise_sibling = sibling_plan.raise_arm;
        let custom_siblings = sibling_plan.custom_arms.clone();
        let has_sibling_nonresuming = sibling_plan.has_any();
        let has_finally = handle.finally.is_some();
        let outer_raise_target = self.current_raise_target();

        let escape_arm_plans =
            Self::build_multi_resuming_escape_arm_plans(handle, escape_arms)?;
        let ResolvedMultiResumingEscapeSites {
            sites: resolved_sites,
            capture_ids,
        } = Self::resolve_multi_resuming_escape_sites_from_plan(
            handle,
            state_machine_plan,
            escape_arm_plans.as_slice(),
        )?;

        let mut scanned_sites: Vec<MultiResumingEscapeSitePlan<'hir>> =
            Vec::with_capacity(resolved_sites.len());
        for resolved in resolved_sites {
            match resolved.site {
                MultiResumingEscapeSiteKind::Direct(site) => {
                    let arm = resolved.arm.arm;
                    if arm.op.binders.len() != site.args.len() {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle multi-resuming heap-continuation-only binder arity mismatch",
                            at: arm.op.span.into(),
                        });
                    }
                    let resume_value_ty =
                        self.cg_ty_of(site.decl.ty)
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle multi-resuming heap-continuation-only perform value type",
                                at: site.decl.span.into(),
                            })?;
                    scanned_sites.push(MultiResumingEscapeSitePlan {
                        site: MultiResumingEscapeSiteKind::Direct(site),
                        arm,
                        continuation_symbol: resolved.arm.continuation_symbol,
                        resume_value_ty,
                        op_tag: self.effect_op_tag(&arm.op.op.fqn),
                    });
                }
                MultiResumingEscapeSiteKind::Indirect(indirect_site) => {
                    let arm = resolved.arm.arm;
                    if arm.op.binders.len() > 1 {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle multi-resuming heap-continuation-only indirect binder count (only 1 supported)",
                            at: arm.op.span.into(),
                        });
                    }
                    let resume_value_ty =
                        self.cg_ty_of(indirect_site.decl.ty)
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle multi-resuming heap-continuation-only perform value type",
                                at: indirect_site.decl.span.into(),
                            })?;
                    scanned_sites.push(MultiResumingEscapeSitePlan {
                        site: MultiResumingEscapeSiteKind::Indirect(indirect_site),
                        arm,
                        continuation_symbol: resolved.arm.continuation_symbol,
                        resume_value_ty,
                        op_tag: self.effect_op_tag(&arm.op.op.fqn),
                    });
                }
            }
        }

        if scanned_sites.is_empty() {
            if has_sibling_nonresuming {
                return self.codegen_handle_expr_multi_nonresuming_leaf(
                    span,
                    handle,
                    sibling_nonresuming_arms,
                    out_ty,
                );
            }
            if has_finally {
                return self.codegen_handle_expr_no_perform(span, handle, out_ty);
            }
            let body_v = self.codegen_block_value(&handle.body)?;
            return match out_ty {
                CgTy::Unit => Ok(CgValue::unit()),
                CgTy::Never => Ok(CgValue::never()),
                _ => Ok(self.coerce_value(handle.body.span, body_v, out_ty)?),
            };
        }

        scanned_sites.sort_by_key(|plan| (plan.top_level_stmt_idx(), plan.decl().span.start));

        let (outer_visible_supported, body_visible_supported) =
            self.collect_escape_capture_metas_from_plan(
                span,
                state_machine_plan,
                &capture_ids,
                "handle multi-resuming heap-continuation-only capture local type",
                "handle multi-resuming heap-continuation-only capture local missing",
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

        let func_name = func.get_name().to_str().unwrap_or("anonymous").to_string();
        let func_name = sanitize_llvm_ident(&func_name);
        let seq = handle.body.span.start as u32;

        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();

        let state_ty_name =
            format!("scoop.runtime.MultiResumingHeapState__{func_name}_{seq}");
        let state_ty = if let Some(existing) = self.context.get_struct_type(&state_ty_name) {
            existing
        } else {
            let ty = self.context.opaque_struct_type(&state_ty_name);
            let header_ty = self.llvm_gc_object_header_type();
            let mut fields: Vec<BasicTypeEnum<'ctx>> = vec![header_ty.into(), i32_ty.into()];
            for cap in &outer_visible_supported {
                fields.push(match self.escape_capture_storage_kind(span, cap.ty)? {
                    Some(EscapeCaptureStorageKind::Word) => i64_ty.into(),
                    Some(EscapeCaptureStorageKind::GcRef) => gc_i8_ptr_ty.into(),
                    None => unreachable!("captures filtered by type"),
                });
            }
            for cap in &body_visible_supported {
                fields.push(match self.escape_capture_storage_kind(span, cap.ty)? {
                    Some(EscapeCaptureStorageKind::Word) => i64_ty.into(),
                    Some(EscapeCaptureStorageKind::GcRef) => gc_i8_ptr_ty.into(),
                    None => unreachable!("captures filtered by type"),
                });
            }
            ty.set_body(&fields, false);
            ty
        };
        let outer_field_base = 2u32;
        let body_field_base = outer_field_base.saturating_add(outer_visible_supported.len() as u32);
        let pc_field_idx = 1u32;

        let step_name = format!("__scoop_multi_resume_heap_step__{func_name}_{seq}");
        let step_fn_ty = self.context.void_type().fn_type(
            &[gc_i8_ptr_ty.into(), i64_ty.into(), gc_i8_ptr_ty.into()],
            false,
        );
        let step_fn = self.module.add_function(&step_name, step_fn_ty, None);
        step_fn.set_linkage(Linkage::Internal);
        step_fn.set_gc(super::super::LLVM_GC_STRATEGY_STATEPOINT_EXAMPLE);

        let saved_block = insert_block;
        {
            let mut cg = MainCodegen::new(MainCodegenInputs {
                context: self.context,
                module: self.module,
                builder: self.builder,
                target_data: self.target_data,
                host: self.host,
                source_map: self.source_map,
                entry_source_id: self.entry_source_id,
                types: self.types,
                struct_layouts: self.struct_layouts,
                enum_layouts: self.enum_layouts,
                top_level_vars: self.top_level_vars,
                top_level_consts: self.top_level_consts,
                object_inits: self.object_inits,
                class_inits: self.class_inits,
                class_vtables: self.class_vtables,
                interfaces: self.interfaces,
                class_itables: self.class_itables,
                ctor_call_sites: self.ctor_call_sites,
                extern_funs: self.extern_funs,
                fun_index: self.fun_index,
                effect_op_tags: Rc::clone(&self.effect_op_tags),
            });

            let entry = self.context.append_basic_block(step_fn, "entry");
            cg.builder.position_at_end(entry);
            cg.current_fun_return_ty = Some(CgTy::Unit);
            cg.env.push_scope();

            let state_raw = step_fn
                .get_nth_param(0)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi-resuming heap step state param",
                    at: span.into(),
                })?
                .into_pointer_value();
            let state_ptr_ty = cg.llvm_ptr_type(cg.gc_address_space());
            let state_ptr = cg.builder.build_pointer_cast(
                state_raw,
                state_ptr_ty,
                "multi_resume_heap_step_state_ptr",
            )?;
            let resume_word = step_fn
                .get_nth_param(1)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi-resuming heap step resume_word param",
                    at: span.into(),
                })?
                .into_int_value();
            let resume_gc_ref = step_fn
                .get_nth_param(2)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi-resuming heap step resume_gc_ref param",
                    at: span.into(),
                })?
                .into_pointer_value();

            for (idx, cap) in outer_visible_supported.iter().enumerate() {
                let field_idx = outer_field_base.saturating_add(idx as u32);
                let field_ptr = cg.builder.build_struct_gep(
                    state_ty,
                    state_ptr,
                    field_idx,
                    "multi_resume_heap_step_outer_gep",
                )?;
                let name = format!("multi_resume_heap_outer_{}", cap.id.as_u32());
                let ptr =
                    cg.restore_escape_capture_local_from_state(span, field_ptr, cap.ty, &name)?;
                cg.env.insert(
                    cap.id,
                    CgLocal {
                        hir_ty: cap.hir_ty,
                        ty: cap.ty,
                        ptr,
                        mutable: cap.mutable,
                    },
                );
            }
            for (idx, cap) in body_visible_supported.iter().enumerate() {
                let field_idx = body_field_base.saturating_add(idx as u32);
                let field_ptr = cg.builder.build_struct_gep(
                    state_ty,
                    state_ptr,
                    field_idx,
                    "multi_resume_heap_step_body_gep",
                )?;
                let name = format!("multi_resume_heap_body_{}", cap.id.as_u32());
                let ptr =
                    cg.restore_escape_capture_local_from_state(span, field_ptr, cap.ty, &name)?;
                cg.env.insert(
                    cap.id,
                    CgLocal {
                        hir_ty: cap.hir_ty,
                        ty: cap.ty,
                        ptr,
                        mutable: cap.mutable,
                    },
                );
            }

            let step_cont_ptr =
                cg.create_entry_alloca(span, "multi_resume_heap_step_k", CgTy::Ref)?;
            let step_sibling_dispatch = cg.build_sibling_nonresuming_dispatch_blocks(
                step_fn,
                "multi_resume_heap_step",
                &sibling_plan,
            );
            let step_effect_dispatch_bb = step_sibling_dispatch.effect_dispatch_bb;
            let step_effect_dispatch_nomatch_bb =
                step_sibling_dispatch.effect_dispatch_nomatch_bb;
            let step_raise_catch_bb = step_sibling_dispatch.raise_catch_bb;
            let step_custom_catch_bbs = step_sibling_dispatch.custom_catch_bbs;
            let dispatch_bb = self
                .context
                .append_basic_block(step_fn, "multi_resume_heap_step_dispatch");
            let bad_state_bb = self
                .context
                .append_basic_block(step_fn, "multi_resume_heap_step_bad_pc");
            let mut state_bbs: Vec<inkwell::basic_block::BasicBlock<'ctx>> = Vec::new();
            let mut step_arm_bbs: Vec<inkwell::basic_block::BasicBlock<'ctx>> = Vec::new();
            let mut step_arm_unwind_bbs: Vec<inkwell::basic_block::BasicBlock<'ctx>> = Vec::new();
            let mut step_binder_slots_by_site: Vec<Vec<ImmediateResumeBinderSlot<'ctx>>> =
                Vec::new();
            for (site_idx, plan) in scanned_sites.iter().enumerate() {
                state_bbs.push(self.context.append_basic_block(
                    step_fn,
                    &format!("multi_resume_heap_step_state_{site_idx}"),
                ));
                step_arm_bbs.push(self.context.append_basic_block(
                    step_fn,
                    &format!("multi_resume_heap_step_arm_{site_idx}"),
                ));
                step_arm_unwind_bbs.push(self.context.append_basic_block(
                    step_fn,
                    &format!("multi_resume_heap_step_arm_unwind_{site_idx}"),
                ));
                let prefix = format!("multi_resume_heap_step_site_{site_idx}");
                step_binder_slots_by_site
                    .push(cg.build_multi_resuming_escape_binder_slots(plan.arm, &prefix)?);
            }

            if let Some(step_effect_dispatch_bb) = step_effect_dispatch_bb {
                for (idx, custom) in custom_siblings.iter().enumerate() {
                    cg.push_effect_unwind_target(
                        &custom.arm.op.op.fqn,
                        step_custom_catch_bbs[idx],
                    );
                }
                cg.push_raise_target(step_effect_dispatch_bb);
            }

            cg.builder.build_unconditional_branch(dispatch_bb)?;

            cg.builder.position_at_end(dispatch_bb);
            let state_pc_ptr = cg.builder.build_struct_gep(
                state_ty,
                state_ptr,
                pc_field_idx,
                "multi_resume_heap_step_pc_gep",
            )?;
            let pc = cg
                .builder
                .build_load(i32_ty, state_pc_ptr, "multi_resume_heap_step_pc")?
                .into_int_value();
            let cases: Vec<(IntValue<'ctx>, inkwell::basic_block::BasicBlock<'ctx>)> = state_bbs
                .iter()
                .enumerate()
                .map(|(site_idx, bb)| (i32_ty.const_int(site_idx as u64, false), *bb))
                .collect();
            cg.builder.build_switch(pc, bad_state_bb, &cases)?;

            cg.builder.position_at_end(bad_state_bb);
            cg.emit_exit_with_code(span, 3)?;

            for (site_idx, state_bb) in state_bbs.iter().enumerate() {
                let plan = &scanned_sites[site_idx];
                cg.builder.position_at_end(*state_bb);
                cg.codegen_multi_resuming_heap_step_bind_resumed_site(
                    span,
                    plan,
                    resume_word,
                    resume_gc_ref,
                )?;
                let saved_env = cg.env.clone();
                cg.codegen_multi_resuming_heap_step_continue_after_site(
                    span,
                    handle,
                    site_idx,
                    scanned_sites.as_slice(),
                    step_binder_slots_by_site.as_slice(),
                    step_arm_bbs.as_slice(),
                    step_fn,
                    step_cont_ptr,
                    state_ty,
                    state_raw,
                    state_ptr,
                    &outer_visible_supported,
                    outer_field_base,
                    &body_visible_supported,
                    body_field_base,
                    pc_field_idx,
                )?;
                cg.env = saved_env;
            }

            if step_effect_dispatch_bb.is_some() {
                cg.pop_raise_target();
                for _ in custom_siblings.iter().rev() {
                    cg.pop_effect_unwind_target();
                }
            }

            if let Some(step_effect_dispatch_bb) = step_effect_dispatch_bb {
                let step_effect_dispatch_nomatch_bb = step_effect_dispatch_nomatch_bb
                    .expect("multi-resuming heap step dispatch_nomatch bb should exist");
                cg.builder.position_at_end(step_effect_dispatch_bb);
                let rt_read_tag = cg.declare_runtime_effect_perform_slot_read_op_tag();
                let tag_call = cg.builder.build_call(
                    rt_read_tag,
                    &[],
                    "multi_resume_heap_step_read_op_tag",
                )?;
                let tag_raw = tag_call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "multi-resuming heap step read_op_tag return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(slot_tag) = tag_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "multi-resuming heap step read_op_tag return type",
                        at: span.into(),
                    });
                };
                let mut dispatch_cases: Vec<(
                    IntValue<'ctx>,
                    inkwell::basic_block::BasicBlock<'ctx>,
                )> = Vec::new();
                if let Some(step_raise_catch_bb) = step_raise_catch_bb {
                    let raise_tag = cg.effect_op_tag("scoop.core.Raise.raise");
                    dispatch_cases.push((
                        i32_ty.const_int(raise_tag as u64, false),
                        step_raise_catch_bb,
                    ));
                }
                for (idx, custom) in custom_siblings.iter().enumerate() {
                    dispatch_cases.push((
                        i32_ty.const_int(custom.op_tag as u64, false),
                        step_custom_catch_bbs[idx],
                    ));
                }
                cg.builder.build_switch(
                    slot_tag,
                    step_effect_dispatch_nomatch_bb,
                    &dispatch_cases,
                )?;

                cg.builder.position_at_end(step_effect_dispatch_nomatch_bb);
                let unpin = cg.declare_runtime_gc_unpin();
                let _ = cg.builder.build_call(
                    unpin,
                    &[state_raw.into()],
                    "multi_resume_heap_step_state_unpin_nomatch",
                )?;
                cg.builder.build_return(None)?;

                if let (Some(raise_arm), Some(step_raise_catch_bb)) =
                    (raise_sibling, step_raise_catch_bb)
                {
                    let binder = &raise_arm.op.binders[0];
                    cg.builder.position_at_end(step_raise_catch_bb);

                    let rt_len = cg.declare_runtime_effect_perform_slot_read_len_words();
                    let call = cg.builder.build_call(
                        rt_len,
                        &[],
                        "multi_resume_heap_step_raise_read_slot_len_words",
                    )?;
                    let raw = call.try_as_basic_value().basic().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "effect slot_read_len_words return value",
                            at: span.into(),
                        },
                    )?;
                    let BasicValueEnum::IntValue(len_words_i32) = raw else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "effect slot_read_len_words return type",
                            at: span.into(),
                        });
                    };

                    let expected_len = cg.context.i32_type().const_int(2, false);
                    let len_ok = cg.builder.build_int_compare(
                        IntPredicate::EQ,
                        len_words_i32,
                        expected_len,
                        "multi_resume_heap_step_raise_slot_len_ok",
                    )?;
                    let len_ok_bb = cg.context.append_basic_block(
                        step_fn,
                        "multi_resume_heap_step_raise_slot_len_ok_bb",
                    );
                    let len_bad_bb = cg.context.append_basic_block(
                        step_fn,
                        "multi_resume_heap_step_raise_slot_len_bad_bb",
                    );
                    cg.builder
                        .build_conditional_branch(len_ok, len_ok_bb, len_bad_bb)?;

                    cg.builder.position_at_end(len_bad_bb);
                    cg.emit_exit_with_code(span, 3)?;

                    cg.builder.position_at_end(len_ok_bb);
                    let rt_read_at = cg.declare_runtime_effect_perform_slot_read_u64_at();
                    let idx0 = cg.context.i32_type().const_int(0, false);
                    let idx1 = cg.context.i32_type().const_int(1, false);
                    let kind_call = cg.builder.build_call(
                        rt_read_at,
                        &[idx0.into()],
                        "multi_resume_heap_step_raise_read_slot_word0",
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
                    let value_call = cg.builder.build_call(
                        rt_read_at,
                        &[idx1.into()],
                        "multi_resume_heap_step_raise_read_slot_word1",
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

                    let rt_clear = cg.declare_runtime_effect_clear();
                    let _ = cg.builder.build_call(
                        rt_clear,
                        &[],
                        "multi_resume_heap_step_raise_clear",
                    )?;

                    cg.env.push_scope();
                    let binder_cg_ty =
                        cg.cg_ty_of(binder.ty)
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle binder type",
                                at: binder.span.into(),
                            })?;
                    let binder_value = match binder_cg_ty {
                        CgTy::Int(int_ty) => {
                            let expected = cg.context.i64_type().const_int(1, false);
                            let ok = cg.builder.build_int_compare(
                                IntPredicate::EQ,
                                kind_u64,
                                expected,
                                "multi_resume_heap_step_raise_kind_is_int",
                            )?;
                            let ok_bb = cg.context.append_basic_block(
                                step_fn,
                                "multi_resume_heap_step_raise_kind_int_ok",
                            );
                            let bad_bb = cg.context.append_basic_block(
                                step_fn,
                                "multi_resume_heap_step_raise_kind_int_bad",
                            );
                            cg.builder.build_conditional_branch(ok, ok_bb, bad_bb)?;

                            cg.builder.position_at_end(bad_bb);
                            cg.emit_exit_with_code(span, 3)?;

                            cg.builder.position_at_end(ok_bb);
                            let from_u64 = IntTy {
                                bits: 64,
                                signed: false,
                            };
                            let decoded = cg.cast_int(value_u64, from_u64, int_ty)?;
                            CgValue::int(decoded, int_ty)
                        }
                        CgTy::Enum(enum_ty) if cg.is_sysroot_runtime_error_enum(enum_ty) => {
                            let expected = cg.context.i64_type().const_int(2, false);
                            let ok = cg.builder.build_int_compare(
                                IntPredicate::EQ,
                                kind_u64,
                                expected,
                                "multi_resume_heap_step_raise_kind_is_runtime_error",
                            )?;
                            let ok_bb = cg.context.append_basic_block(
                                step_fn,
                                "multi_resume_heap_step_raise_kind_runtime_error_ok",
                            );
                            let bad_bb = cg.context.append_basic_block(
                                step_fn,
                                "multi_resume_heap_step_raise_kind_runtime_error_bad",
                            );
                            cg.builder.build_conditional_branch(ok, ok_bb, bad_bb)?;

                            cg.builder.position_at_end(bad_bb);
                            cg.emit_exit_with_code(span, 3)?;

                            cg.builder.position_at_end(ok_bb);
                            let repr = cg.cg_enum_layout(span, enum_ty)?.repr;
                            if !matches!(repr, CgEnumRepr::TaggedUnion) {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "Raise<RuntimeError> niche repr (not supported)",
                                    at: span.into(),
                                });
                            }

                            let tag_i32 = cg.builder.build_int_truncate(
                                value_u64,
                                cg.context.i32_type(),
                                "multi_resume_heap_step_runtime_error_tag_i32",
                            )?;
                            let payload_word_zero =
                                cg.int_type(cg.enum_payload_ty()).const_int(0, false);
                            let payload_ptr_zero = cg.llvm_gc_i8_ptr_type().const_null();
                            let llvm_enum_ty = cg.llvm_enum_value_type(span, enum_ty)?;
                            let llvm_enum_ty = llvm_enum_ty.into_struct_type();
                            let mut agg: AggregateValueEnum<'ctx> = llvm_enum_ty.get_undef().into();
                            agg = cg.builder.build_insert_value(
                                agg,
                                tag_i32,
                                0,
                                "multi_resume_heap_step_runtime_error_tag",
                            )?;
                            agg = cg.builder.build_insert_value(
                                agg,
                                payload_word_zero,
                                1,
                                "multi_resume_heap_step_runtime_error_payload_word",
                            )?;
                            agg = cg.builder.build_insert_value(
                                agg,
                                payload_ptr_zero,
                                2,
                                "multi_resume_heap_step_runtime_error_payload_ptr",
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
                    let binder_ptr =
                        cg.create_entry_alloca(binder.span, &binder.name, binder_cg_ty)?;
                    let _ =
                        cg.store_local_value(binder.span, binder_ptr, binder_cg_ty, binder_value)?;
                    cg.env.insert(
                        binder.id,
                        CgLocal {
                            hir_ty: Some(binder.ty),
                            ty: binder_cg_ty,
                            ptr: binder_ptr,
                            mutable: false,
                        },
                    );

                    for custom in &custom_siblings {
                        cg.push_effect_unwind_target(
                            &custom.arm.op.op.fqn,
                            step_effect_dispatch_nomatch_bb,
                        );
                    }
                    cg.push_raise_target(step_effect_dispatch_nomatch_bb);
                    let arm_v =
                        cg.codegen_expr_in_expected_context(&raise_arm.body, Some(out_ty))?;
                    cg.pop_raise_target();
                    for _ in custom_siblings.iter().rev() {
                        cg.pop_effect_unwind_target();
                    }
                    if out_ty != CgTy::Unit && out_ty != CgTy::Never {
                        let _ = cg.coerce_value(raise_arm.body.span, arm_v, out_ty)?;
                    }
                    cg.env.pop_scope();

                    if let Some(bb) = cg.builder.get_insert_block()
                        && bb.get_terminator().is_none()
                    {
                        let unpin = cg.declare_runtime_gc_unpin();
                        let _ = cg.builder.build_call(
                            unpin,
                            &[state_raw.into()],
                            "multi_resume_heap_step_state_unpin_raise",
                        )?;
                        cg.builder.build_return(None)?;
                    }
                }

                for (idx, custom) in custom_siblings.iter().enumerate() {
                    let arm = custom.arm;
                    let binder = &arm.op.binders[0];
                    cg.builder.position_at_end(step_custom_catch_bbs[idx]);

                    let rt_len = cg.declare_runtime_effect_perform_slot_read_len_words();
                    let call = cg.builder.build_call(
                        rt_len,
                        &[],
                        "multi_resume_heap_step_custom_read_slot_len_words",
                    )?;
                    let raw = call.try_as_basic_value().basic().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "effect slot_read_len_words return value",
                            at: span.into(),
                        },
                    )?;
                    let BasicValueEnum::IntValue(len_words_i32) = raw else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "effect slot_read_len_words return type",
                            at: span.into(),
                        });
                    };

                    let expected_len = cg.context.i32_type().const_int(1, false);
                    let len_ok = cg.builder.build_int_compare(
                        IntPredicate::EQ,
                        len_words_i32,
                        expected_len,
                        "multi_resume_heap_step_custom_slot_len_ok",
                    )?;
                    let len_ok_bb = cg.context.append_basic_block(
                        step_fn,
                        "multi_resume_heap_step_custom_slot_len_ok_bb",
                    );
                    let len_bad_bb = cg.context.append_basic_block(
                        step_fn,
                        "multi_resume_heap_step_custom_slot_len_bad_bb",
                    );
                    cg.builder
                        .build_conditional_branch(len_ok, len_ok_bb, len_bad_bb)?;

                    cg.builder.position_at_end(len_bad_bb);
                    cg.emit_exit_with_code(span, 3)?;

                    cg.builder.position_at_end(len_ok_bb);
                    let rt_read = cg.declare_runtime_effect_perform_slot_read_u64();
                    let value_call = cg.builder.build_call(
                        rt_read,
                        &[],
                        "multi_resume_heap_step_custom_read_slot_word0",
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
                    let rt_read_gc = cg.declare_runtime_effect_perform_slot_read_gc_ref();
                    let gc_call = cg.builder.build_call(
                        rt_read_gc,
                        &[],
                        "multi_resume_heap_step_custom_read_slot_gc_ref",
                    )?;
                    let gc_raw = gc_call.try_as_basic_value().basic().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "effect slot_read_gc_ref return value",
                            at: span.into(),
                        },
                    )?;
                    let BasicValueEnum::PointerValue(gc_ref_raw) = gc_raw else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "effect slot_read_gc_ref return type",
                            at: span.into(),
                        });
                    };

                    cg.env.push_scope();
                    let binder_cg_ty =
                        cg.cg_ty_of(binder.ty)
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle binder type (custom non-resuming)",
                                at: binder.span.into(),
                            })?;
                    let binder_value = cg.decode_abi_payload_transport(
                        binder.span,
                        value_u64,
                        gc_ref_raw,
                        binder_cg_ty,
                    )?;
                    let binder_ptr =
                        cg.create_entry_alloca(binder.span, &binder.name, binder_cg_ty)?;
                    let _ =
                        cg.store_local_value(binder.span, binder_ptr, binder_cg_ty, binder_value)?;
                    cg.env.insert(
                        binder.id,
                        CgLocal {
                            hir_ty: Some(binder.ty),
                            ty: binder_cg_ty,
                            ptr: binder_ptr,
                            mutable: false,
                        },
                    );

                    let rt_clear = cg.declare_runtime_effect_clear();
                    let _ = cg.builder.build_call(
                        rt_clear,
                        &[],
                        "multi_resume_heap_step_custom_clear",
                    )?;

                    for custom in &custom_siblings {
                        cg.push_effect_unwind_target(
                            &custom.arm.op.op.fqn,
                            step_effect_dispatch_nomatch_bb,
                        );
                    }
                    cg.push_raise_target(step_effect_dispatch_nomatch_bb);
                    let arm_v = cg.codegen_expr_in_expected_context(&arm.body, Some(out_ty))?;
                    cg.pop_raise_target();
                    for _ in custom_siblings.iter().rev() {
                        cg.pop_effect_unwind_target();
                    }
                    if out_ty != CgTy::Unit && out_ty != CgTy::Never {
                        let _ = cg.coerce_value(arm.body.span, arm_v, out_ty)?;
                    }
                    cg.env.pop_scope();

                    if let Some(bb) = cg.builder.get_insert_block()
                        && bb.get_terminator().is_none()
                    {
                        let unpin = cg.declare_runtime_gc_unpin();
                        let _ = cg.builder.build_call(
                            unpin,
                            &[state_raw.into()],
                            "multi_resume_heap_step_state_unpin_custom",
                        )?;
                        cg.builder.build_return(None)?;
                    }
                }
            }

            for (site_idx, arm_bb) in step_arm_bbs.iter().enumerate() {
                let plan = &scanned_sites[site_idx];
                cg.builder.position_at_end(*arm_bb);
                cg.env.push_scope();
                for slot in &step_binder_slots_by_site[site_idx] {
                    cg.env.insert(
                        slot.id,
                        CgLocal {
                            hir_ty: Some(slot.hir_ty),
                            ty: slot.ty,
                            ptr: slot.ptr,
                            mutable: false,
                        },
                    );
                }
                cg.env.insert(
                    plan.continuation_symbol,
                    CgLocal {
                        hir_ty: None,
                        ty: CgTy::Ref,
                        ptr: step_cont_ptr,
                        mutable: false,
                    },
                );
                if has_sibling_nonresuming {
                    for custom in &custom_siblings {
                        cg.push_effect_unwind_target(
                            &custom.arm.op.op.fqn,
                            step_arm_unwind_bbs[site_idx],
                        );
                    }
                    cg.push_raise_target(step_arm_unwind_bbs[site_idx]);
                }
                let arm_v =
                    cg.codegen_expr_in_expected_context(&plan.arm.body, Some(out_ty))?;
                if has_sibling_nonresuming {
                    cg.pop_raise_target();
                    for _ in custom_siblings.iter().rev() {
                        cg.pop_effect_unwind_target();
                    }
                }
                if out_ty != CgTy::Unit && out_ty != CgTy::Never {
                    let _ = cg.coerce_value(plan.arm.body.span, arm_v, out_ty)?;
                }
                cg.env.pop_scope();

                if let Some(bb) = cg.builder.get_insert_block()
                    && bb.get_terminator().is_none()
                {
                    let llvm_ref_ty = cg.llvm_basic_type_of(span, CgTy::Ref)?;
                    let k_loaded = cg
                        .builder
                        .build_load(
                            llvm_ref_ty,
                            step_cont_ptr,
                            "multi_resume_heap_step_k_unpin_load",
                        )?
                        .into_pointer_value();
                    let unpin = cg.declare_runtime_gc_unpin();
                    let _ = cg.builder.build_call(
                        unpin,
                        &[k_loaded.into()],
                        "multi_resume_heap_step_k_unpin",
                    )?;
                    cg.builder.build_return(None)?;
                }

                if has_sibling_nonresuming {
                    cg.builder.position_at_end(step_arm_unwind_bbs[site_idx]);
                    let llvm_ref_ty = cg.llvm_basic_type_of(span, CgTy::Ref)?;
                    let k_loaded = cg
                        .builder
                        .build_load(
                            llvm_ref_ty,
                            step_cont_ptr,
                            "multi_resume_heap_step_k_unpin_load_unwind",
                        )?
                        .into_pointer_value();
                    let unpin = cg.declare_runtime_gc_unpin();
                    let _ = cg.builder.build_call(
                        unpin,
                        &[k_loaded.into()],
                        "multi_resume_heap_step_k_unpin_unwind",
                    )?;
                    cg.builder.build_return(None)?;
                }
            }

            if !has_sibling_nonresuming {
                for unwind_bb in &step_arm_unwind_bbs {
                    cg.builder.position_at_end(*unwind_bb);
                    cg.builder.build_unreachable()?;
                }
            }

            cg.env.pop_scope();
        }
        self.builder.position_at_end(saved_block);

        let body_bb = self
            .context
            .append_basic_block(func, "handle_multi_resume_heap_body");
        let done_bb = self
            .context
            .append_basic_block(func, "handle_multi_resume_heap_done");
        let finally_bb = if has_finally {
            Some(
                self.context
                    .append_basic_block(func, "handle_multi_resume_heap_finally"),
            )
        } else {
            None
        };
        let finally_unwind_bb = if has_finally {
            Some(
                self.context.append_basic_block(
                    func,
                    "handle_multi_resume_heap_finally_unwind",
                ),
            )
        } else {
            None
        };
        let sibling_dispatch = self.build_sibling_nonresuming_dispatch_blocks(
            func,
            "handle_multi_resume_heap",
            &sibling_plan,
        );
        let effect_dispatch_bb = sibling_dispatch.effect_dispatch_bb;
        let effect_dispatch_nomatch_bb = sibling_dispatch.effect_dispatch_nomatch_bb;
        let raise_catch_bb = sibling_dispatch.raise_catch_bb;
        let custom_catch_bbs = sibling_dispatch.custom_catch_bbs;
        let main_raise_target = effect_dispatch_bb.or(finally_unwind_bb);
        let result_ptr = if out_ty == CgTy::Unit || out_ty == CgTy::Never {
            None
        } else {
            Some(self.create_entry_alloca(
                span,
                "handle_multi_resume_heap_result",
                out_ty,
            )?)
        };
        let cont_ptr =
            self.create_entry_alloca(span, "handle_multi_resume_heap_k", CgTy::Ref)?;
        let continuation_created_ptr = self.create_entry_alloca_raw(
            span,
            "handle_multi_resume_heap_cont_created",
            self.context.bool_type().into(),
        )?;
        let _ = self.builder.build_store(
            continuation_created_ptr,
            self.context.bool_type().const_zero(),
        )?;
        let mut initial_binder_slots_by_site: Vec<Vec<ImmediateResumeBinderSlot<'ctx>>> =
            Vec::new();
        let mut arm_bbs: Vec<inkwell::basic_block::BasicBlock<'ctx>> = Vec::new();
        let mut arm_unwind_bbs: Vec<inkwell::basic_block::BasicBlock<'ctx>> = Vec::new();
        for (site_idx, plan) in scanned_sites.iter().enumerate() {
            let prefix = format!("multi_resume_heap_site_{site_idx}");
            initial_binder_slots_by_site
                .push(self.build_multi_resuming_escape_binder_slots(plan.arm, &prefix)?);
            arm_bbs.push(self.context.append_basic_block(
                func,
                &format!("handle_multi_resume_heap_arm_{site_idx}"),
            ));
            arm_unwind_bbs.push(self.context.append_basic_block(
                func,
                &format!("handle_multi_resume_heap_arm_unwind_{site_idx}"),
            ));
        }

        let total_size = self.target_data.get_store_size(&state_ty);
        let size_bytes = self.target_data.get_store_size(&state_ty);
        let trace_start_offset_bytes =
            if outer_visible_supported.is_empty() && body_visible_supported.is_empty() {
                size_bytes
            } else {
                self.target_data
                    .offset_of_element(&state_ty, outer_field_base)
                    .unwrap_or(size_bytes)
            };
        let state_desc_global_name =
            format!("__scoop_type_desc_multi_resume_heap_state__{func_name}_{seq}");
        let state_desc = self.get_or_create_type_descriptor_global(TypeDescriptorSpec {
            at: span,
            global_name: &state_desc_global_name,
            canonical_name: &state_ty_name,
            obj_ty: state_ty,
            trace_start_offset_bytes,
            parent: None,
            itable: None,
            vtable: None,
        })?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let size_v = i64_ty.const_int(total_size, false);
        let state_desc_i8 = self.builder.build_pointer_cast(
            state_desc.as_pointer_value(),
            i8_ptr_ty,
            "multi_resume_heap_state_desc_i8",
        )?;
        let alloc_call = self.builder.build_call(
            rt_alloc,
            &[state_desc_i8.into(), size_v.into()],
            "rt_alloc_multi_resume_heap_state",
        )?;
        let alloc_raw = alloc_call.try_as_basic_value().basic().ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "multi-resuming heap alloc return value",
                at: span.into(),
            },
        )?;
        let BasicValueEnum::PointerValue(state_raw) = alloc_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "multi-resuming heap alloc return type",
                at: span.into(),
            });
        };
        let pin = self.declare_runtime_gc_pin();
        let _ = self.builder.build_call(
            pin,
            &[state_raw.into()],
            "multi_resume_heap_state_pin",
        )?;

        let state_gc_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let state_gc_ptr = self.builder.build_pointer_cast(
            state_raw,
            state_gc_ptr_ty,
            "multi_resume_heap_state_ptr",
        )?;
        let pc_ptr = self.builder.build_struct_gep(
            state_ty,
            state_gc_ptr,
            pc_field_idx,
            "multi_resume_heap_state_pc_gep",
        )?;
        let _ = self
            .builder
            .build_store(pc_ptr, i32_ty.const_zero())?;
        for (idx, cap) in outer_visible_supported.iter().enumerate() {
            let field_idx = outer_field_base.saturating_add(idx as u32);
            let field_ptr = self.builder.build_struct_gep(
                state_ty,
                state_gc_ptr,
                field_idx,
                "multi_resume_heap_state_outer_init_gep",
            )?;
            self.zero_init_escape_capture_state_field(span, field_ptr, cap.ty)?;
        }
        for (idx, cap) in body_visible_supported.iter().enumerate() {
            let field_idx = body_field_base.saturating_add(idx as u32);
            let field_ptr = self.builder.build_struct_gep(
                state_ty,
                state_gc_ptr,
                field_idx,
                "multi_resume_heap_state_body_init_gep",
            )?;
            self.zero_init_escape_capture_state_field(span, field_ptr, cap.ty)?;
        }

        self.builder.build_unconditional_branch(body_bb)?;

        self.builder.position_at_end(body_bb);
        self.env.push_scope();
        if let Some(main_raise_target) = main_raise_target {
            for (idx, custom) in custom_siblings.iter().enumerate() {
                self.push_effect_unwind_target(&custom.arm.op.op.fqn, custom_catch_bbs[idx]);
            }
            self.push_raise_target(main_raise_target);
        }
        self.codegen_multi_resuming_heap_main_body_with_intercepts(
            span,
            handle,
            out_ty,
            result_ptr,
            finally_bb.unwrap_or(done_bb),
            scanned_sites.as_slice(),
            initial_binder_slots_by_site.as_slice(),
            arm_bbs.as_slice(),
            step_fn,
            cont_ptr,
            continuation_created_ptr,
            state_ty,
            state_raw,
            state_gc_ptr,
            &outer_visible_supported,
            outer_field_base,
            &body_visible_supported,
            body_field_base,
            pc_field_idx,
        )?;
        if main_raise_target.is_some() {
            self.pop_raise_target();
            for _ in custom_siblings.iter().rev() {
                self.pop_effect_unwind_target();
            }
        }
        self.env.pop_scope();

        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_none()
        {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle multi-resuming heap-continuation-only (main body completion missing)",
                at: span.into(),
            });
        }

        if let Some(effect_dispatch_bb) = effect_dispatch_bb {
            let effect_dispatch_nomatch_bb = effect_dispatch_nomatch_bb
                .expect("multi-resuming heap dispatch_nomatch bb should exist");
            self.builder.position_at_end(effect_dispatch_bb);
            let rt_read_tag = self.declare_runtime_effect_perform_slot_read_op_tag();
            let tag_call = self.builder.build_call(
                rt_read_tag,
                &[],
                "multi_resume_heap_dispatch_read_op_tag",
            )?;
            let tag_raw = tag_call.try_as_basic_value().basic().ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "multi-resuming heap dispatch read_op_tag return value",
                    at: span.into(),
                },
            )?;
            let BasicValueEnum::IntValue(slot_tag) = tag_raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi-resuming heap dispatch read_op_tag return type",
                    at: span.into(),
                });
            };
            let mut dispatch_cases: Vec<(IntValue<'ctx>, inkwell::basic_block::BasicBlock<'ctx>)> =
                Vec::new();
            if let Some(raise_catch_bb) = raise_catch_bb {
                let raise_tag = self.effect_op_tag("scoop.core.Raise.raise");
                dispatch_cases.push((i32_ty.const_int(raise_tag as u64, false), raise_catch_bb));
            }
            for (idx, custom) in custom_siblings.iter().enumerate() {
                dispatch_cases.push((
                    i32_ty.const_int(custom.op_tag as u64, false),
                    custom_catch_bbs[idx],
                ));
            }
            self.builder
                .build_switch(slot_tag, effect_dispatch_nomatch_bb, &dispatch_cases)?;

            self.builder.position_at_end(effect_dispatch_nomatch_bb);
            if let Some(finally_unwind_bb) = finally_unwind_bb {
                self.builder.build_unconditional_branch(finally_unwind_bb)?;
            } else {
                let unpin = self.declare_runtime_gc_unpin();
                let _ = self.builder.build_call(
                    unpin,
                    &[state_raw.into()],
                    "multi_resume_heap_state_unpin_nomatch",
                )?;
                if let Some(target) = outer_raise_target {
                    self.builder.build_unconditional_branch(target)?;
                } else {
                    let ret_ty =
                        self.current_fun_return_ty
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "multi-resuming heap dispatch unwind needs function return type",
                                at: span.into(),
                            })?;
                    let v = self.default_value(span, ret_ty)?;
                    self.emit_return(span, ret_ty, v)?;
                }
            }

            if let (Some(raise_arm), Some(raise_catch_bb)) = (raise_sibling, raise_catch_bb) {
                let binder = &raise_arm.op.binders[0];
                self.builder.position_at_end(raise_catch_bb);

                let rt_len = self.declare_runtime_effect_perform_slot_read_len_words();
                let call = self.builder.build_call(
                    rt_len,
                    &[],
                    "multi_resume_heap_raise_read_slot_len_words",
                )?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_len_words return value",
                        at: span.into(),
                    },
                )?;
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
                    "multi_resume_heap_raise_slot_len_ok",
                )?;
                let len_ok_bb = self
                    .context
                    .append_basic_block(func, "multi_resume_heap_raise_slot_len_ok_bb");
                let len_bad_bb = self
                    .context
                    .append_basic_block(func, "multi_resume_heap_raise_slot_len_bad_bb");
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
                    "multi_resume_heap_raise_read_slot_word0",
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
                    "multi_resume_heap_raise_read_slot_word1",
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
                let _ = self.builder.build_call(
                    rt_clear,
                    &[],
                    "multi_resume_heap_raise_clear",
                )?;

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
                            "multi_resume_heap_raise_kind_is_int",
                        )?;
                        let ok_bb = self.context.append_basic_block(
                            func,
                            "multi_resume_heap_raise_kind_int_ok",
                        );
                        let bad_bb = self.context.append_basic_block(
                            func,
                            "multi_resume_heap_raise_kind_int_bad",
                        );
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
                            "multi_resume_heap_raise_kind_is_runtime_error",
                        )?;
                        let ok_bb = self.context.append_basic_block(
                            func,
                            "multi_resume_heap_raise_kind_runtime_error_ok",
                        );
                        let bad_bb = self.context.append_basic_block(
                            func,
                            "multi_resume_heap_raise_kind_runtime_error_bad",
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
                            "multi_resume_heap_runtime_error_tag_i32",
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
                            "multi_resume_heap_runtime_error_tag",
                        )?;
                        agg = self.builder.build_insert_value(
                            agg,
                            payload_word_zero,
                            1,
                            "multi_resume_heap_runtime_error_payload_word",
                        )?;
                        agg = self.builder.build_insert_value(
                            agg,
                            payload_ptr_zero,
                            2,
                            "multi_resume_heap_runtime_error_payload_ptr",
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
                let binder_ptr =
                    self.create_entry_alloca(binder.span, &binder.name, binder_cg_ty)?;
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

                let nested_unwind_target = finally_unwind_bb.unwrap_or(effect_dispatch_nomatch_bb);
                for custom in &custom_siblings {
                    self.push_effect_unwind_target(&custom.arm.op.op.fqn, nested_unwind_target);
                }
                self.push_raise_target(nested_unwind_target);
                let arm_v = self.codegen_expr_in_expected_context(&raise_arm.body, Some(out_ty))?;
                self.pop_raise_target();
                for _ in custom_siblings.iter().rev() {
                    self.pop_effect_unwind_target();
                }
                let arm_v = if out_ty == CgTy::Unit {
                    CgValue::unit()
                } else if out_ty == CgTy::Never {
                    CgValue::never()
                } else {
                    self.coerce_value(raise_arm.body.span, arm_v, out_ty)?
                };
                self.env.pop_scope();

                if let Some(bb) = self.builder.get_insert_block()
                    && bb.get_terminator().is_none()
                {
                    if let Some(ptr) = result_ptr {
                        let _ = self.store_local_value(raise_arm.body.span, ptr, out_ty, arm_v)?;
                    }
                    self.builder
                        .build_unconditional_branch(finally_bb.unwrap_or(done_bb))?;
                }
            }

            for (idx, custom) in custom_siblings.iter().enumerate() {
                let arm = custom.arm;
                let binder = &arm.op.binders[0];
                self.builder.position_at_end(custom_catch_bbs[idx]);

                let rt_len = self.declare_runtime_effect_perform_slot_read_len_words();
                let call = self.builder.build_call(
                    rt_len,
                    &[],
                    "multi_resume_heap_custom_read_slot_len_words",
                )?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_len_words return value",
                        at: span.into(),
                    },
                )?;
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
                    "multi_resume_heap_custom_slot_len_ok",
                )?;
                let len_ok_bb = self
                    .context
                    .append_basic_block(func, "multi_resume_heap_custom_slot_len_ok_bb");
                let len_bad_bb = self
                    .context
                    .append_basic_block(func, "multi_resume_heap_custom_slot_len_bad_bb");
                self.builder
                    .build_conditional_branch(len_ok, len_ok_bb, len_bad_bb)?;

                self.builder.position_at_end(len_bad_bb);
                self.emit_exit_with_code(span, 3)?;

                self.builder.position_at_end(len_ok_bb);
                let rt_read = self.declare_runtime_effect_perform_slot_read_u64();
                let value_call = self.builder.build_call(
                    rt_read,
                    &[],
                    "multi_resume_heap_custom_read_slot_word0",
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
                    "multi_resume_heap_custom_read_slot_gc_ref",
                )?;
                let gc_raw = gc_call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_gc_ref return value",
                        at: span.into(),
                    },
                )?;
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
                let binder_ptr =
                    self.create_entry_alloca(binder.span, &binder.name, binder_cg_ty)?;
                let _ =
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
                let _ = self.builder.build_call(
                    rt_clear,
                    &[],
                    "multi_resume_heap_custom_clear",
                )?;

                let nested_unwind_target = finally_unwind_bb.unwrap_or(effect_dispatch_nomatch_bb);
                for custom in &custom_siblings {
                    self.push_effect_unwind_target(&custom.arm.op.op.fqn, nested_unwind_target);
                }
                self.push_raise_target(nested_unwind_target);
                let arm_v = self.codegen_expr_in_expected_context(&arm.body, Some(out_ty))?;
                self.pop_raise_target();
                for _ in custom_siblings.iter().rev() {
                    self.pop_effect_unwind_target();
                }
                let arm_v = if out_ty == CgTy::Unit {
                    CgValue::unit()
                } else if out_ty == CgTy::Never {
                    CgValue::never()
                } else {
                    self.coerce_value(arm.body.span, arm_v, out_ty)?
                };
                self.env.pop_scope();

                if let Some(bb) = self.builder.get_insert_block()
                    && bb.get_terminator().is_none()
                {
                    if let Some(ptr) = result_ptr {
                        let _ = self.store_local_value(arm.body.span, ptr, out_ty, arm_v)?;
                    }
                    self.builder
                        .build_unconditional_branch(finally_bb.unwrap_or(done_bb))?;
                }
            }
        }

        for (site_idx, arm_bb) in arm_bbs.iter().enumerate() {
            let plan = &scanned_sites[site_idx];
            self.builder.position_at_end(*arm_bb);
            self.env.push_scope();
            for slot in &initial_binder_slots_by_site[site_idx] {
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
            self.env.insert(
                plan.continuation_symbol,
                CgLocal {
                    hir_ty: None,
                    ty: CgTy::Ref,
                    ptr: cont_ptr,
                    mutable: false,
                },
            );
            if has_sibling_nonresuming {
                for custom in &custom_siblings {
                    self.push_effect_unwind_target(
                        &custom.arm.op.op.fqn,
                        arm_unwind_bbs[site_idx],
                    );
                }
                self.push_raise_target(arm_unwind_bbs[site_idx]);
            } else if let Some(finally_unwind_bb) = finally_unwind_bb {
                self.push_raise_target(finally_unwind_bb);
            }
            let arm_v = self.codegen_expr_in_expected_context(&plan.arm.body, Some(out_ty))?;
            if has_sibling_nonresuming {
                self.pop_raise_target();
                for _ in custom_siblings.iter().rev() {
                    self.pop_effect_unwind_target();
                }
            } else if finally_unwind_bb.is_some() {
                self.pop_raise_target();
            }
            let arm_v = if out_ty == CgTy::Unit {
                CgValue::unit()
            } else if out_ty == CgTy::Never {
                CgValue::never()
            } else {
                self.coerce_value(plan.arm.body.span, arm_v, out_ty)?
            };
            self.env.pop_scope();

            if let Some(bb) = self.builder.get_insert_block()
                && bb.get_terminator().is_none()
            {
                if let Some(ptr) = result_ptr {
                    let _ = self.store_local_value(plan.arm.body.span, ptr, out_ty, arm_v)?;
                }
                let target = finally_bb.unwrap_or(done_bb);
                self.builder.build_unconditional_branch(target)?;
            }

            if has_sibling_nonresuming {
                self.builder.position_at_end(arm_unwind_bbs[site_idx]);
                let llvm_ref_ty = self.llvm_basic_type_of(span, CgTy::Ref)?;
                let k_loaded = self
                    .builder
                    .build_load(
                        llvm_ref_ty,
                        cont_ptr,
                        "multi_resume_heap_k_unpin_load_unwind",
                    )?
                    .into_pointer_value();
                let unpin = self.declare_runtime_gc_unpin();
                let _ = self.builder.build_call(
                    unpin,
                    &[k_loaded.into()],
                    "multi_resume_heap_k_unpin_unwind",
                )?;
                if let Some(finally_unwind_bb) = finally_unwind_bb {
                    self.builder.build_unconditional_branch(finally_unwind_bb)?;
                } else if let Some(target) = outer_raise_target {
                    self.builder.build_unconditional_branch(target)?;
                } else {
                    let ret_ty =
                        self.current_fun_return_ty
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "multi-resuming heap arm unwind needs function return type",
                                at: span.into(),
                            })?;
                    let v = self.default_value(span, ret_ty)?;
                    self.emit_return(span, ret_ty, v)?;
                }
            }
        }

        if !has_sibling_nonresuming {
            for unwind_bb in &arm_unwind_bbs {
                self.builder.position_at_end(*unwind_bb);
                self.builder.build_unreachable()?;
            }
        }

        if let Some(finally_unwind_bb) = finally_unwind_bb {
            self.builder.position_at_end(finally_unwind_bb);
            if let Some(finally) = handle.finally.as_ref() {
                let _ = self.codegen_block_value(finally)?;
            }
            if let Some(bb) = self.builder.get_insert_block()
                && bb.get_terminator().is_none()
            {
                let created = self
                    .builder
                    .build_load(
                        self.context.bool_type(),
                        continuation_created_ptr,
                        "multi_resume_heap_unwind_cont_created",
                    )?
                    .into_int_value();
                let unwind_propagate_bb = self.context.append_basic_block(
                    func,
                    "handle_multi_resume_heap_finally_unwind_propagate",
                );
                let unwind_unpin_bb = self.context.append_basic_block(
                    func,
                    "handle_multi_resume_heap_finally_unwind_unpin",
                );
                self.builder
                    .build_conditional_branch(created, unwind_propagate_bb, unwind_unpin_bb)?;

                self.builder.position_at_end(unwind_unpin_bb);
                let unpin = self.declare_runtime_gc_unpin();
                let _ = self.builder.build_call(
                    unpin,
                    &[state_raw.into()],
                    "multi_resume_heap_state_unpin_unwind",
                )?;
                self.builder
                    .build_unconditional_branch(unwind_propagate_bb)?;

                self.builder.position_at_end(unwind_propagate_bb);
                if let Some(target) = outer_raise_target {
                    self.builder.build_unconditional_branch(target)?;
                } else {
                    let ret_ty =
                        self.current_fun_return_ty
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle multi-resuming heap-continuation-only finally unwind needs function return type",
                                at: span.into(),
                            })?;
                    let v = self.default_value(span, ret_ty)?;
                    self.emit_return(span, ret_ty, v)?;
                }
            }
        }

        if let Some(finally_bb) = finally_bb {
            self.builder.position_at_end(finally_bb);
            if let Some(finally) = handle.finally.as_ref() {
                let _ = self.codegen_block_value(finally)?;
            }
            if let Some(bb) = self.builder.get_insert_block()
                && bb.get_terminator().is_none()
            {
                self.builder.build_unconditional_branch(done_bb)?;
            }
        }

        self.builder.position_at_end(done_bb);
        let done_unpin_k_bb = self
            .context
            .append_basic_block(func, "handle_multi_resume_heap_done_unpin_k");
        let done_unpin_state_bb = self
            .context
            .append_basic_block(func, "handle_multi_resume_heap_done_unpin_state");
        let done_merge_bb = self
            .context
            .append_basic_block(func, "handle_multi_resume_heap_done_merge");
        let created = self
            .builder
            .build_load(
                self.context.bool_type(),
                continuation_created_ptr,
                "multi_resume_heap_done_cont_created",
            )?
            .into_int_value();
        self.builder
            .build_conditional_branch(created, done_unpin_k_bb, done_unpin_state_bb)?;

        self.builder.position_at_end(done_unpin_k_bb);
        let llvm_ref_ty = self.llvm_basic_type_of(span, CgTy::Ref)?;
        let k_loaded = self
            .builder
            .build_load(
                llvm_ref_ty,
                cont_ptr,
                "multi_resume_heap_k_unpin_load",
            )?
            .into_pointer_value();
        let unpin = self.declare_runtime_gc_unpin();
        let _ = self.builder.build_call(
            unpin,
            &[k_loaded.into()],
            "multi_resume_heap_k_unpin",
        )?;
        self.builder.build_unconditional_branch(done_merge_bb)?;

        self.builder.position_at_end(done_unpin_state_bb);
        let _ = self.builder.build_call(
            unpin,
            &[state_raw.into()],
            "multi_resume_heap_state_unpin_done",
        )?;
        self.builder.build_unconditional_branch(done_merge_bb)?;

        self.builder.position_at_end(done_merge_bb);

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
                        kind: "handle multi-resuming heap-continuation-only result slot",
                        at: span.into(),
                    });
                };
                let llvm_ty = self.llvm_basic_type_of(span, out_ty)?;
                let loaded = self
                    .builder
                    .build_load(
                        llvm_ty,
                        ptr,
                        "handle_multi_resume_heap_result",
                    )?;
                Ok(CgValue {
                    ty: out_ty,
                    value: Some(loaded),
                })
            }
        }
    }
}
