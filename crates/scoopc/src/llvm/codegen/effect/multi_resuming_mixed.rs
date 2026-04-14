impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    fn codegen_mixed_escape_tail_stmt(
        &mut self,
        stmt: &hir::Stmt,
        return_kind: &'static str,
        stmt_kind: &'static str,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match &stmt.kind {
            hir::StmtKind::Empty => Ok(CgValue::unit()),
            hir::StmtKind::Val(decl) => {
                self.codegen_val_decl(decl)?;
                Ok(CgValue::unit())
            }
            hir::StmtKind::Assign { lhs, eq_span, rhs } => {
                self.codegen_assign_stmt(*eq_span, lhs, rhs)?;
                Ok(CgValue::unit())
            }
            hir::StmtKind::Expr(expr) => self.codegen_expr(expr),
            hir::StmtKind::While { cond, body } => {
                self.codegen_while_stmt(stmt.span, cond, body)?;
                Ok(CgValue::unit())
            }
            hir::StmtKind::Return { .. } => Err(LlvmEmitError::UnsupportedMainBody {
                kind: return_kind,
                at: stmt.span.into(),
            }),
            hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Todo(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: stmt_kind,
                at: stmt.span.into(),
            }),
        }
    }

    fn mixed_escape_resume_path_from_immediate_frames<'hir>(
        frames: &[ImmediateResumeFrame<'hir>],
    ) -> Vec<MixedEscapeDirectFrame<'hir>> {
        frames
            .iter()
            .map(|frame| match frame {
                ImmediateResumeFrame::Block { block, stmt_idx } => {
                    MixedEscapeDirectFrame::Block {
                        block,
                        stmt_idx: *stmt_idx,
                    }
                }
                ImmediateResumeFrame::IfThen {
                    if_expr,
                    then_block,
                    stmt_idx,
                } => MixedEscapeDirectFrame::IfThen {
                    if_expr,
                    then_block,
                    stmt_idx: *stmt_idx,
                },
                ImmediateResumeFrame::IfElse {
                    if_expr,
                    else_block,
                    stmt_idx,
                } => MixedEscapeDirectFrame::IfElse {
                    if_expr,
                    else_block,
                    stmt_idx: *stmt_idx,
                },
                ImmediateResumeFrame::WhileBody {
                    while_cond,
                    while_body,
                    stmt_idx,
                } => MixedEscapeDirectFrame::WhileBody {
                    while_cond,
                    while_body,
                    stmt_idx: *stmt_idx,
                },
            })
            .collect()
    }

    #[allow(clippy::too_many_arguments)]
    fn codegen_multi_resuming_mixed_main_body_with_intercepts_from_start_idx<'hir>(
        &mut self,
        span: crate::span::Span,
        handle: &'hir hir::HandleExpr,
        start_idx: usize,
        out_ty: CgTy,
        result_ptr: Option<PointerValue<'ctx>>,
        completion_target: inkwell::basic_block::BasicBlock<'ctx>,
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
        if start_idx >= handle.body.stmts.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle unified mixed multi-resuming leaf (missing top-level tail after nested immediate site)",
                at: handle.body.span.into(),
            });
        }

        let last_stmt_idx = handle.body.stmts.len() - 1;
        for (stmt_idx, stmt) in handle
            .body
            .stmts
            .iter()
            .enumerate()
            .take(last_stmt_idx)
            .skip(start_idx)
        {
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
                    kind: "handle unified mixed multi-resuming leaf (nested source-path in final top-level statement not yet supported)",
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
                    kind: "`return` inside unified mixed multi-resuming body",
                    at: final_stmt.span.into(),
                });
            }
            hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Todo(_) => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "statement inside unified mixed multi-resuming body",
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

    #[allow(clippy::too_many_arguments)]
    fn codegen_multi_resuming_mixed_continue_after_completed_frame<'hir>(
        &mut self,
        span: crate::span::Span,
        handle: &'hir hir::HandleExpr,
        top_level_stmt_idx: usize,
        resume_path: &[MixedEscapeDirectFrame<'hir>],
        completed_depth: usize,
        out_ty: CgTy,
        result_ptr: Option<PointerValue<'ctx>>,
        completion_target: inkwell::basic_block::BasicBlock<'ctx>,
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
        if completed_depth == 0 {
            return self.codegen_multi_resuming_mixed_main_body_with_intercepts_from_start_idx(
                span,
                handle,
                top_level_stmt_idx + 1,
                out_ty,
                result_ptr,
                completion_target,
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
            );
        }

        let parent_depth = completed_depth - 1;
        let start_idx = resume_path[parent_depth].stmt_idx() + 1;
        match &resume_path[parent_depth] {
            MixedEscapeDirectFrame::WhileBody {
                while_cond,
                while_body,
                ..
            } => self.codegen_multi_resuming_mixed_continue_while_tail(
                span,
                handle,
                top_level_stmt_idx,
                resume_path,
                parent_depth,
                start_idx,
                while_cond,
                while_body,
                out_ty,
                result_ptr,
                completion_target,
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
            ),
            MixedEscapeDirectFrame::Block { .. }
            | MixedEscapeDirectFrame::IfThen { .. }
            | MixedEscapeDirectFrame::IfElse { .. } => {
                self.codegen_multi_resuming_mixed_continue_frame_tail(
                    span,
                    handle,
                    top_level_stmt_idx,
                    resume_path,
                    parent_depth,
                    start_idx,
                    out_ty,
                    result_ptr,
                    completion_target,
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
                )
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn codegen_multi_resuming_mixed_continue_frame_tail<'hir>(
        &mut self,
        span: crate::span::Span,
        handle: &'hir hir::HandleExpr,
        top_level_stmt_idx: usize,
        resume_path: &[MixedEscapeDirectFrame<'hir>],
        depth: usize,
        start_idx: usize,
        out_ty: CgTy,
        result_ptr: Option<PointerValue<'ctx>>,
        completion_target: inkwell::basic_block::BasicBlock<'ctx>,
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
        let stmts = match &resume_path[depth] {
            MixedEscapeDirectFrame::Block { block, .. } => &block.stmts,
            MixedEscapeDirectFrame::IfThen { then_block, .. } => &then_block.stmts,
            MixedEscapeDirectFrame::IfElse { else_block, .. } => &else_block.stmts,
            MixedEscapeDirectFrame::WhileBody { while_body, .. } => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle unified mixed multi-resuming leaf (while frame needs specialized tail lowering)",
                    at: while_body.span.into(),
                });
            }
        };
        let saved_env = self.env.clone();
        let prefix = resume_path[..=depth].to_vec();
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
        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_none()
        {
            self.codegen_multi_resuming_mixed_continue_after_completed_frame(
                span,
                handle,
                top_level_stmt_idx,
                resume_path,
                depth,
                out_ty,
                result_ptr,
                completion_target,
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
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn codegen_multi_resuming_mixed_continue_while_tail<'hir>(
        &mut self,
        span: crate::span::Span,
        handle: &'hir hir::HandleExpr,
        top_level_stmt_idx: usize,
        resume_path: &[MixedEscapeDirectFrame<'hir>],
        depth: usize,
        start_idx: usize,
        while_cond: &'hir hir::Expr,
        while_body: &'hir hir::Block,
        out_ty: CgTy,
        result_ptr: Option<PointerValue<'ctx>>,
        completion_target: inkwell::basic_block::BasicBlock<'ctx>,
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
        let prefix = resume_path[..=depth].to_vec();
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
            .append_basic_block(func, "multi_resume_mixed_tail_while_tail");
        let cond_bb = self
            .context
            .append_basic_block(func, "multi_resume_mixed_tail_while_cond");
        let body_bb = self
            .context
            .append_basic_block(func, "multi_resume_mixed_tail_while_body");
        let after_bb = self
            .context
            .append_basic_block(func, "multi_resume_mixed_tail_while_after");
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

        self.builder.position_at_end(cond_bb);
        let cond_v = self.codegen_expr_in_expected_context(while_cond, Some(CgTy::Bool))?;
        let cond_v = self.coerce_value(while_cond.span, cond_v, CgTy::Bool)?;
        let cond_i1 = cond_v.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "handle unified mixed multi-resuming leaf while condition value",
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
        self.codegen_multi_resuming_mixed_continue_after_completed_frame(
            span,
            handle,
            top_level_stmt_idx,
            resume_path,
            depth,
            out_ty,
            result_ptr,
            completion_target,
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
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn codegen_multi_resuming_mixed_continue_after_immediate_site<'hir>(
        &mut self,
        span: crate::span::Span,
        handle: &'hir hir::HandleExpr,
        perform_site: &'hir ImmediateResumeSite<'hir>,
        out_ty: CgTy,
        result_ptr: Option<PointerValue<'ctx>>,
        completion_target: inkwell::basic_block::BasicBlock<'ctx>,
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
        let resume_path =
            Self::mixed_escape_resume_path_from_immediate_frames(perform_site.resume_path.as_slice());
        if resume_path.is_empty() {
            return self.codegen_multi_resuming_mixed_main_body_with_intercepts_from_start_idx(
                span,
                handle,
                perform_site.top_level_stmt_idx + 1,
                out_ty,
                result_ptr,
                completion_target,
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
            );
        }

        let last_depth = resume_path.len() - 1;
        let start_idx = resume_path[last_depth].stmt_idx() + 1;
        match &resume_path[last_depth] {
            MixedEscapeDirectFrame::WhileBody {
                while_cond,
                while_body,
                ..
            } => self.codegen_multi_resuming_mixed_continue_while_tail(
                span,
                handle,
                perform_site.top_level_stmt_idx,
                resume_path.as_slice(),
                last_depth,
                start_idx,
                while_cond,
                while_body,
                out_ty,
                result_ptr,
                completion_target,
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
            ),
            MixedEscapeDirectFrame::Block { .. }
            | MixedEscapeDirectFrame::IfThen { .. }
            | MixedEscapeDirectFrame::IfElse { .. } => {
                self.codegen_multi_resuming_mixed_continue_frame_tail(
                    span,
                    handle,
                    perform_site.top_level_stmt_idx,
                    resume_path.as_slice(),
                    last_depth,
                    start_idx,
                    out_ty,
                    result_ptr,
                    completion_target,
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
                )
            }
        }
    }

    fn codegen_handle_expr_unified_single_immediate_single_escape_multi_resuming_leaf<'hir>(
        &mut self,
        span: crate::span::Span,
        handle: &'hir hir::HandleExpr,
        state_machine_plan: &HandleStateMachinePlan,
        arms: UnifiedMixedResumingArmPair<'hir>,
        sibling_nonresuming_arms: &[&'hir hir::HandleArm],
        out_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        #[derive(Clone, Copy)]
        struct MixedCustomSibling<'hir, 'ctx> {
            arm: &'hir hir::HandleArm,
            frame_ptr: PointerValue<'ctx>,
            catch_bb: inkwell::basic_block::BasicBlock<'ctx>,
            op_tag: u32,
        }

        let (immediate_arm, resume_symbol) = arms.immediate;
        let (escape_arm, continuation_symbol) = arms.escape;

        let immediate_arm_plans = Self::build_multi_resuming_immediate_arm_plans(
            handle,
            &[(immediate_arm, resume_symbol)],
        )?;
        let resolved_immediate_sites = Self::resolve_multi_resuming_immediate_sites_from_plan(
            handle,
            state_machine_plan,
            immediate_arm_plans.as_slice(),
        )?;
        let [resolved_immediate_site] = resolved_immediate_sites.as_slice() else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle unified mixed multi-resuming leaf (missing direct immediate perform)",
                at: span.into(),
            });
        };
        let perform_site = resolved_immediate_site.site.clone();
        let escape_arm_plans = Self::build_multi_resuming_escape_arm_plans(
            handle,
            &[(escape_arm, continuation_symbol)],
        )?;
        let ResolvedMultiResumingEscapeSites {
            sites: resolved_escape_sites,
            capture_ids,
        } = Self::resolve_multi_resuming_escape_sites_from_plan(
            handle,
            state_machine_plan,
            escape_arm_plans.as_slice(),
        )?;

        let perform_idx = perform_site.top_level_stmt_idx;
        let resume_value_ty =
            self.cg_ty_of(perform_site.decl.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle unified mixed multi-resuming leaf immediate value type",
                    at: perform_site.decl.span.into(),
                })?;

        if immediate_arm.op.binders.len() != perform_site.args.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle resume binder arity mismatch",
                at: immediate_arm.op.span.into(),
            });
        }

        let escape_op_tag = self.effect_op_tag(&escape_arm.op.op.fqn);
        let mut scanned_sites: Vec<MultiResumingEscapeSitePlan<'hir>> =
            Vec::with_capacity(resolved_escape_sites.len());
        let mut escape_resume_value_ty: Option<CgTy> = None;

        for resolved in resolved_escape_sites {
            match resolved.site {
                MultiResumingEscapeSiteKind::Direct(site) => {
                    if site.top_level_stmt_idx <= perform_idx {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle unified mixed multi-resuming leaf (escape perform before immediate site not yet supported)",
                            at: site.decl.span.into(),
                        });
                    }
                    if escape_arm.op.binders.len() != site.args.len() {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle unified mixed multi-resuming leaf escape binder arity mismatch",
                            at: escape_arm.op.span.into(),
                        });
                    }
                    let site_resume_value_ty =
                        self.cg_ty_of(site.decl.ty)
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle unified mixed multi-resuming leaf escape value type",
                                at: site.decl.span.into(),
                            })?;
                    if let Some(expected) = escape_resume_value_ty {
                        if expected != site_resume_value_ty {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle unified mixed multi-resuming leaf escape value type mismatch",
                                at: site.decl.span.into(),
                            });
                        }
                    } else {
                        escape_resume_value_ty = Some(site_resume_value_ty);
                    }
                    scanned_sites.push(MultiResumingEscapeSitePlan {
                        site: MultiResumingEscapeSiteKind::Direct(site),
                        arm: resolved.arm.arm,
                        continuation_symbol: resolved.arm.continuation_symbol,
                        resume_value_ty: site_resume_value_ty,
                        op_tag: escape_op_tag,
                    });
                }
                MultiResumingEscapeSiteKind::Indirect(indirect_site) => {
                    if indirect_site.top_level_stmt_idx <= perform_idx {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle unified mixed multi-resuming leaf (indirect perform before immediate site not yet supported)",
                            at: indirect_site.decl.span.into(),
                        });
                    }
                    if escape_arm.op.binders.len() > 1 {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle unified mixed multi-resuming leaf indirect binder count (only 1 supported)",
                            at: escape_arm.op.span.into(),
                        });
                    }
                    let site_resume_value_ty =
                        self.cg_ty_of(indirect_site.decl.ty)
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle unified mixed multi-resuming leaf escape value type",
                                at: indirect_site.decl.span.into(),
                            })?;
                    if let Some(expected) = escape_resume_value_ty {
                        if expected != site_resume_value_ty {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle unified mixed multi-resuming leaf escape value type mismatch",
                                at: indirect_site.decl.span.into(),
                            });
                        }
                    } else {
                        escape_resume_value_ty = Some(site_resume_value_ty);
                    }
                    scanned_sites.push(MultiResumingEscapeSitePlan {
                        site: MultiResumingEscapeSiteKind::Indirect(indirect_site),
                        arm: resolved.arm.arm,
                        continuation_symbol: resolved.arm.continuation_symbol,
                        resume_value_ty: site_resume_value_ty,
                        op_tag: escape_op_tag,
                    });
                }
            }
        }

        if scanned_sites.is_empty() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle unified mixed multi-resuming leaf (escape site missing)",
                at: escape_arm.span.into(),
            });
        }
        scanned_sites.sort_by_key(|plan| (plan.top_level_stmt_idx(), plan.decl().span.start));
        let _ = escape_resume_value_ty.ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "handle unified mixed multi-resuming leaf escape value type",
            at: escape_arm.span.into(),
        })?;

        let sibling_plan = self.collect_sibling_nonresuming_plan(sibling_nonresuming_arms)?;
        let raise_sibling = sibling_plan.raise_arm;
        let custom_sibling_arms = sibling_plan.custom_arms.clone();
        let has_sibling_nonresuming = sibling_plan.has_any();

        let (outer_visible_supported, body_visible_supported) =
            self.collect_escape_capture_metas_from_plan(
                span,
                state_machine_plan,
                &capture_ids,
                "handle unified mixed multi-resuming leaf capture local type",
                "handle unified mixed multi-resuming leaf capture local missing",
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
        let outer_raise_target = self.current_raise_target();

        let func_name = func.get_name().to_str().unwrap_or("anonymous").to_string();
        let func_name = sanitize_llvm_ident(&func_name);
        let seq = handle.body.span.start as u32;

        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let handler_frame_ty = self.llvm_effect_handler_frame_type();

        let state_ty_name =
            format!("scoop.runtime.MultiResumingMixedEscapeState__{func_name}_{seq}");
        let state_ty = if let Some(existing) = self.context.get_struct_type(&state_ty_name) {
            existing
        } else {
            let ty = self.context.opaque_struct_type(&state_ty_name);
            let header_ty = self.llvm_gc_object_header_type();
            let mut fields: Vec<BasicTypeEnum<'ctx>> =
                vec![header_ty.into(), handler_frame_ty.into(), i32_ty.into()];
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
        let frame_field_idx = 1u32;
        let pc_field_idx = 2u32;
        let outer_field_base = 3u32;
        let body_field_base = outer_field_base.saturating_add(outer_visible_supported.len() as u32);

        let step_name = format!("__scoop_multi_resume_mixed_step__{func_name}_{seq}");
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
                    kind: "multi-resuming mixed step state param",
                    at: span.into(),
                })?
                .into_pointer_value();
            let state_ptr_ty = cg.llvm_ptr_type(cg.gc_address_space());
            let state_ptr = cg.builder.build_pointer_cast(
                state_raw,
                state_ptr_ty,
                "multi_resume_mixed_step_state_ptr",
            )?;
            let resume_word = step_fn
                .get_nth_param(1)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi-resuming mixed step resume_word param",
                    at: span.into(),
                })?
                .into_int_value();
            let resume_gc_ref = step_fn
                .get_nth_param(2)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "multi-resuming mixed step resume_gc_ref param",
                    at: span.into(),
                })?
                .into_pointer_value();

            for (idx, cap) in outer_visible_supported.iter().enumerate() {
                let field_idx = outer_field_base.saturating_add(idx as u32);
                let field_ptr = cg.builder.build_struct_gep(
                    state_ty,
                    state_ptr,
                    field_idx,
                    "multi_resume_mixed_step_outer_gep",
                )?;
                let name = format!("multi_resume_mixed_outer_{}", cap.id.as_u32());
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
                    "multi_resume_mixed_step_body_gep",
                )?;
                let name = format!("multi_resume_mixed_body_{}", cap.id.as_u32());
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

            let step_cont_ptr = cg.create_entry_alloca(
                span,
                &format!("handle_multi_resume_mixed_step_k_{seq}"),
                CgTy::Ref,
            )?;
            let step_sibling_dispatch = cg.build_sibling_nonresuming_dispatch_blocks(
                step_fn,
                "multi_resume_mixed_step",
                &sibling_plan,
            );
            let step_effect_dispatch_bb = step_sibling_dispatch.effect_dispatch_bb;
            let step_effect_dispatch_nomatch_bb =
                step_sibling_dispatch.effect_dispatch_nomatch_bb;
            let step_raise_catch_bb = step_sibling_dispatch.raise_catch_bb;
            let step_custom_catch_bbs = step_sibling_dispatch.custom_catch_bbs;
            let dispatch_bb = self
                .context
                .append_basic_block(step_fn, "multi_resume_mixed_step_dispatch");
            let bad_state_bb = self
                .context
                .append_basic_block(step_fn, "multi_resume_mixed_step_bad_state");
            let mut step_state_bbs: Vec<inkwell::basic_block::BasicBlock<'ctx>> = Vec::new();
            let mut step_arm_bbs: Vec<inkwell::basic_block::BasicBlock<'ctx>> = Vec::new();
            let mut step_arm_unwind_bbs: Vec<inkwell::basic_block::BasicBlock<'ctx>> = Vec::new();
            let mut step_binder_slots_by_site: Vec<Vec<ImmediateResumeBinderSlot<'ctx>>> =
                Vec::new();
            for (site_idx, plan) in scanned_sites.iter().enumerate() {
                step_state_bbs.push(self.context.append_basic_block(
                    step_fn,
                    &format!("multi_resume_mixed_step_state_{site_idx}"),
                ));
                step_arm_bbs.push(self.context.append_basic_block(
                    step_fn,
                    &format!("multi_resume_mixed_step_arm_{site_idx}"),
                ));
                step_arm_unwind_bbs.push(self.context.append_basic_block(
                    step_fn,
                    &format!("multi_resume_mixed_step_arm_unwind_{site_idx}"),
                ));
                let prefix = format!("multi_resume_mixed_step_site_{site_idx}");
                step_binder_slots_by_site
                    .push(cg.build_multi_resuming_escape_binder_slots(plan.arm, &prefix)?);
            }

            if let Some(step_effect_dispatch_bb) = step_effect_dispatch_bb {
                for (idx, custom) in custom_sibling_arms.iter().enumerate() {
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
                "multi_resume_mixed_step_pc_gep",
            )?;
            let pc = cg
                .builder
                .build_load(i32_ty, state_pc_ptr, "multi_resume_mixed_step_pc")?
                .into_int_value();
            let cases: Vec<(IntValue<'ctx>, inkwell::basic_block::BasicBlock<'ctx>)> =
                step_state_bbs
                    .iter()
                    .enumerate()
                    .map(|(site_idx, bb)| (i32_ty.const_int(site_idx as u64, false), *bb))
                    .collect();
            cg.builder.build_switch(pc, bad_state_bb, &cases)?;

            cg.builder.position_at_end(bad_state_bb);
            cg.emit_exit_with_code(span, 3)?;

            for (site_idx, state_bb) in step_state_bbs.iter().enumerate() {
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
                for _ in custom_sibling_arms.iter().rev() {
                    cg.pop_effect_unwind_target();
                }
            }

            if let Some(step_effect_dispatch_bb) = step_effect_dispatch_bb {
                let step_effect_dispatch_nomatch_bb = step_effect_dispatch_nomatch_bb
                    .expect("mixed step dispatch_nomatch bb should exist");
                cg.builder.position_at_end(step_effect_dispatch_bb);
                let rt_read_tag = cg.declare_runtime_effect_perform_slot_read_op_tag();
                let tag_call = cg.builder.build_call(
                    rt_read_tag,
                    &[],
                    "multi_resume_mixed_step_read_op_tag",
                )?;
                let tag_raw = tag_call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "mixed step read_op_tag return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(slot_tag) = tag_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "mixed step read_op_tag return type",
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
                for (idx, custom) in custom_sibling_arms.iter().enumerate() {
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
                    "multi_resume_mixed_step_state_unpin_nomatch",
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
                        "multi_resume_mixed_step_raise_read_slot_len_words",
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
                        "multi_resume_mixed_step_raise_slot_len_ok",
                    )?;
                    let len_ok_bb = cg.context.append_basic_block(
                        step_fn,
                        "multi_resume_mixed_step_raise_slot_len_ok_bb",
                    );
                    let len_bad_bb = cg.context.append_basic_block(
                        step_fn,
                        "multi_resume_mixed_step_raise_slot_len_bad_bb",
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
                        "multi_resume_mixed_step_raise_read_slot_word0",
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
                        "multi_resume_mixed_step_raise_read_slot_word1",
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
                        "multi_resume_mixed_step_raise_clear",
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
                                "multi_resume_mixed_step_raise_kind_is_int",
                            )?;
                            let ok_bb = cg.context.append_basic_block(
                                step_fn,
                                "multi_resume_mixed_step_raise_kind_int_ok",
                            );
                            let bad_bb = cg.context.append_basic_block(
                                step_fn,
                                "multi_resume_mixed_step_raise_kind_int_bad",
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
                                "multi_resume_mixed_step_raise_kind_is_runtime_error",
                            )?;
                            let ok_bb = cg.context.append_basic_block(
                                step_fn,
                                "multi_resume_mixed_step_raise_kind_runtime_error_ok",
                            );
                            let bad_bb = cg.context.append_basic_block(
                                step_fn,
                                "multi_resume_mixed_step_raise_kind_runtime_error_bad",
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
                                "multi_resume_mixed_step_runtime_error_tag_i32",
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
                                "multi_resume_mixed_step_runtime_error_tag",
                            )?;
                            agg = cg.builder.build_insert_value(
                                agg,
                                payload_word_zero,
                                1,
                                "multi_resume_mixed_step_runtime_error_payload_word",
                            )?;
                            agg = cg.builder.build_insert_value(
                                agg,
                                payload_ptr_zero,
                                2,
                                "multi_resume_mixed_step_runtime_error_payload_ptr",
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
                    let _ = cg.store_local_value(
                        binder.span,
                        binder_ptr,
                        binder_cg_ty,
                        binder_value,
                    )?;
                    cg.env.insert(
                        binder.id,
                        CgLocal {
                            hir_ty: Some(binder.ty),
                            ty: binder_cg_ty,
                            ptr: binder_ptr,
                            mutable: false,
                        },
                    );

                    for custom in &custom_sibling_arms {
                        cg.push_effect_unwind_target(
                            &custom.arm.op.op.fqn,
                            step_effect_dispatch_nomatch_bb,
                        );
                    }
                    cg.push_raise_target(step_effect_dispatch_nomatch_bb);
                    let arm_v =
                        cg.codegen_expr_in_expected_context(&raise_arm.body, Some(out_ty))?;
                    cg.pop_raise_target();
                    for _ in custom_sibling_arms.iter().rev() {
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
                            "multi_resume_mixed_step_state_unpin_raise",
                        )?;
                        cg.builder.build_return(None)?;
                    }
                }

                for (idx, custom) in custom_sibling_arms.iter().enumerate() {
                    let arm = custom.arm;
                    let binder = &arm.op.binders[0];
                    cg.builder.position_at_end(step_custom_catch_bbs[idx]);

                    let rt_len = cg.declare_runtime_effect_perform_slot_read_len_words();
                    let call = cg.builder.build_call(
                        rt_len,
                        &[],
                        "multi_resume_mixed_step_custom_read_slot_len_words",
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
                        "multi_resume_mixed_step_custom_slot_len_ok",
                    )?;
                    let len_ok_bb = cg.context.append_basic_block(
                        step_fn,
                        "multi_resume_mixed_step_custom_slot_len_ok_bb",
                    );
                    let len_bad_bb = cg.context.append_basic_block(
                        step_fn,
                        "multi_resume_mixed_step_custom_slot_len_bad_bb",
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
                        "multi_resume_mixed_step_custom_read_slot_word0",
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
                        "multi_resume_mixed_step_custom_read_slot_gc_ref",
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
                    let _ = cg.store_local_value(
                        binder.span,
                        binder_ptr,
                        binder_cg_ty,
                        binder_value,
                    )?;
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
                        "multi_resume_mixed_step_custom_clear",
                    )?;

                    for custom in &custom_sibling_arms {
                        cg.push_effect_unwind_target(
                            &custom.arm.op.op.fqn,
                            step_effect_dispatch_nomatch_bb,
                        );
                    }
                    cg.push_raise_target(step_effect_dispatch_nomatch_bb);
                    let arm_v = cg.codegen_expr_in_expected_context(&arm.body, Some(out_ty))?;
                    cg.pop_raise_target();
                    for _ in custom_sibling_arms.iter().rev() {
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
                            "multi_resume_mixed_step_state_unpin_custom",
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
                    for custom in &custom_sibling_arms {
                        cg.push_effect_unwind_target(
                            &custom.arm.op.op.fqn,
                            step_arm_unwind_bbs[site_idx],
                        );
                    }
                    cg.push_raise_target(step_arm_unwind_bbs[site_idx]);
                }
                let arm_v = cg.codegen_expr_in_expected_context(&plan.arm.body, Some(out_ty))?;
                if has_sibling_nonresuming {
                    cg.pop_raise_target();
                    for _ in custom_sibling_arms.iter().rev() {
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
                            "multi_resume_mixed_step_k_unpin_load",
                        )?
                        .into_pointer_value();
                    let unpin = cg.declare_runtime_gc_unpin();
                    let _ = cg.builder.build_call(
                        unpin,
                        &[k_loaded.into()],
                        "multi_resume_mixed_step_k_unpin",
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
                            "multi_resume_mixed_step_k_unpin_load_unwind",
                        )?
                        .into_pointer_value();
                    let unpin = cg.declare_runtime_gc_unpin();
                    let _ = cg.builder.build_call(
                        unpin,
                        &[k_loaded.into()],
                        "multi_resume_mixed_step_k_unpin_unwind",
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

        let resume_blocks = self.build_mixed_escape_resume_blocks(func, "handle_multi_resume_mixed");
        let dispatch_bb = resume_blocks.dispatch_bb;
        let state0_bb = resume_blocks.state0_bb;
        let state1_bb = resume_blocks.state1_bb;
        let arm_bb = resume_blocks.arm_bb;
        let done_bb = resume_blocks.done_bb;
        let bad_state_bb = resume_blocks.bad_state_bb;
        let finally_bb = resume_blocks.finally_bb;
        let finally_unwind_bb = resume_blocks.finally_unwind_bb;
        let sibling_dispatch = self.build_sibling_nonresuming_dispatch_blocks(
            func,
            "handle_multi_resume_mixed",
            &sibling_plan,
        );
        let effect_dispatch_bb = sibling_dispatch.effect_dispatch_bb;
        let effect_dispatch_nomatch_bb = sibling_dispatch.effect_dispatch_nomatch_bb;
        let raise_catch_bb = sibling_dispatch.raise_catch_bb;
        let custom_catch_bbs = sibling_dispatch.custom_catch_bbs;
        let main_raise_target = effect_dispatch_bb.unwrap_or(finally_unwind_bb);

        let state_ptr =
            self.create_entry_alloca_raw(span, "handle_multi_resume_mixed_state", i32_ty.into())?;
        let resume_used_ptr = self.create_entry_alloca_raw(
            span,
            "handle_multi_resume_mixed_resume_used",
            self.context.bool_type().into(),
        )?;
        let resume_value_ptr = if resume_value_ty == CgTy::Unit {
            None
        } else {
            Some(self.create_entry_alloca(
                span,
                "handle_multi_resume_mixed_resume_value",
                resume_value_ty,
            )?)
        };
        let result_ptr = if matches!(out_ty, CgTy::Unit | CgTy::Never) {
            None
        } else {
            Some(self.create_entry_alloca(
                span,
                "handle_multi_resume_mixed_result",
                out_ty,
            )?)
        };
        let cont_ptr =
            self.create_entry_alloca(span, "handle_multi_resume_mixed_k", CgTy::Ref)?;
        let continuation_created_ptr = self.create_entry_alloca_raw(
            span,
            "handle_multi_resume_mixed_cont_created",
            self.context.bool_type().into(),
        )?;
        let _ = self.builder.build_store(
            continuation_created_ptr,
            self.context.bool_type().const_zero(),
        )?;

        let mut immediate_binder_slots: Vec<ImmediateResumeBinderSlot<'ctx>> = Vec::new();
        for binder in &immediate_arm.op.binders {
            let binder_ty = self
                .cg_ty_of(binder.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle resume binder type",
                    at: binder.span.into(),
                })?;
            let ptr = self.create_entry_alloca(binder.span, &binder.name, binder_ty)?;
            immediate_binder_slots.push(ImmediateResumeBinderSlot {
                id: binder.id,
                hir_ty: binder.ty,
                ty: binder_ty,
                ptr,
            });
        }
        let mut initial_escape_binder_slots_by_site: Vec<Vec<ImmediateResumeBinderSlot<'ctx>>> =
            Vec::new();
        let mut escape_arm_entry_bbs: Vec<inkwell::basic_block::BasicBlock<'ctx>> = Vec::new();
        let mut escape_arm_body_bbs: Vec<inkwell::basic_block::BasicBlock<'ctx>> = Vec::new();
        let mut escape_arm_unwind_bbs: Vec<inkwell::basic_block::BasicBlock<'ctx>> = Vec::new();
        for (site_idx, plan) in scanned_sites.iter().enumerate() {
            let prefix = format!("multi_resume_mixed_site_{site_idx}");
            initial_escape_binder_slots_by_site
                .push(self.build_multi_resuming_escape_binder_slots(plan.arm, &prefix)?);
            escape_arm_entry_bbs.push(self.context.append_basic_block(
                func,
                &format!("handle_multi_resume_mixed_escape_arm_entry_{site_idx}"),
            ));
            escape_arm_body_bbs.push(self.context.append_basic_block(
                func,
                &format!("handle_multi_resume_mixed_escape_arm_body_{site_idx}"),
            ));
            escape_arm_unwind_bbs.push(self.context.append_basic_block(
                func,
                &format!("handle_multi_resume_mixed_escape_arm_unwind_{site_idx}"),
            ));
        }

        let mut custom_siblings: Vec<MixedCustomSibling<'hir, 'ctx>> = Vec::new();
        for (idx, custom) in custom_sibling_arms.iter().enumerate() {
            let frame_ptr = self.create_entry_alloca_raw(
                span,
                &format!("handle_multi_resume_mixed_custom_frame_{idx}"),
                handler_frame_ty.into(),
            )?;
            custom_siblings.push(MixedCustomSibling {
                arm: custom.arm,
                frame_ptr,
                catch_bb: custom_catch_bbs[idx],
                op_tag: custom.op_tag,
            });
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
            format!("__scoop_type_desc_multi_resume_mixed_state__{func_name}_{seq}");
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
            "multi_resume_mixed_state_desc_i8",
        )?;
        let alloc_call = self.builder.build_call(
            rt_alloc,
            &[state_desc_i8.into(), size_v.into()],
            "rt_alloc_multi_resume_mixed_state",
        )?;
        let alloc_raw = alloc_call.try_as_basic_value().basic().ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "multi-resuming mixed alloc return value",
                at: span.into(),
            },
        )?;
        let BasicValueEnum::PointerValue(state_raw) = alloc_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "multi-resuming mixed alloc return type",
                at: span.into(),
            });
        };

        let pin = self.declare_runtime_gc_pin();
        let _ = self.builder.build_call(
            pin,
            &[state_raw.into()],
            "multi_resume_mixed_state_pin",
        )?;

        let state_gc_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let state_gc_ptr = self.builder.build_pointer_cast(
            state_raw,
            state_gc_ptr_ty,
            "multi_resume_mixed_state_ptr",
        )?;
        let state_pc_ptr = self.builder.build_struct_gep(
            state_ty,
            state_gc_ptr,
            pc_field_idx,
            "multi_resume_mixed_state_pc_gep",
        )?;
        let _ = self.builder.build_store(state_pc_ptr, i32_ty.const_zero())?;
        for (idx, cap) in outer_visible_supported.iter().enumerate() {
            let field_idx = outer_field_base.saturating_add(idx as u32);
            let field_ptr = self.builder.build_struct_gep(
                state_ty,
                state_gc_ptr,
                field_idx,
                "multi_resume_mixed_state_outer_init_gep",
            )?;
            self.zero_init_escape_capture_state_field(span, field_ptr, cap.ty)?;
        }
        for (idx, cap) in body_visible_supported.iter().enumerate() {
            let field_idx = body_field_base.saturating_add(idx as u32);
            let field_ptr = self.builder.build_struct_gep(
                state_ty,
                state_gc_ptr,
                field_idx,
                "multi_resume_mixed_state_body_init_gep",
            )?;
            self.zero_init_escape_capture_state_field(span, field_ptr, cap.ty)?;
        }

        let frame_ptr = self.builder.build_struct_gep(
            state_ty,
            state_gc_ptr,
            frame_field_idx,
            "multi_resume_mixed_frame_gep",
        )?;
        let frame_i8 = self.builder.build_address_space_cast(
            frame_ptr,
            i8_ptr_ty,
            "multi_resume_mixed_frame_i8",
        )?;
        let escape_tag = self.effect_op_tag(&escape_arm.op.op.fqn);
        let escape_tag_i32 = i32_ty.const_int(escape_tag as u64, false);
        let rt_push = self.declare_runtime_effect_handler_stack_push();
        let _ = self.builder.build_call(
            rt_push,
            &[frame_i8.into(), escape_tag_i32.into()],
            "multi_resume_mixed_push",
        )?;
        let prev_ptr = self.builder.build_struct_gep(
            handler_frame_ty,
            frame_ptr,
            0,
            "multi_resume_mixed_prev_gep",
        )?;
        let escape_outer_top = self
            .builder
            .build_load(i8_ptr_ty, prev_ptr, "multi_resume_mixed_outer_top")?
            .into_pointer_value();
        let rt_swap = self.declare_runtime_effect_handler_stack_swap_top();
        for custom in &custom_siblings {
            let custom_frame_i8 = self
                .builder
                .build_bit_cast(
                    custom.frame_ptr,
                    i8_ptr_ty,
                    "handle_multi_resume_mixed_custom_frame_i8",
                )?
                .into_pointer_value();
            let custom_tag_i32 = i32_ty.const_int(custom.op_tag as u64, false);
            let _ = self.builder.build_call(
                rt_push,
                &[custom_frame_i8.into(), custom_tag_i32.into()],
                "handle_multi_resume_mixed_custom_push",
            )?;
        }
        let same_handle_restore_top = if let Some(last) = custom_siblings.last() {
            self.builder
                .build_bit_cast(
                    last.frame_ptr,
                    i8_ptr_ty,
                    "handle_multi_resume_mixed_restore_top",
                )?
                .into_pointer_value()
        } else {
            frame_i8
        };

        let _ = self.builder.build_store(state_ptr, i32_ty.const_zero())?;
        let _ = self.builder.build_store(
            resume_used_ptr,
            self.context.bool_type().const_zero(),
        )?;
        let immediate_exec_plan = ImmediateResumeExecPlan {
            handle,
            site: &perform_site,
            out_ty,
            result_ptr,
            handler_exit: ImmediateResumeHandlerExit::None,
            finally_bb,
        };

        self.builder.build_unconditional_branch(dispatch_bb)?;

        self.builder.position_at_end(dispatch_bb);
        let state = self
            .builder
            .build_load(i32_ty, state_ptr, "multi_resume_mixed_state")?
            .into_int_value();
        let cases = [
            (i32_ty.const_zero(), state0_bb),
            (i32_ty.const_int(1, false), state1_bb),
        ];
        self.builder.build_switch(state, bad_state_bb, &cases)?;

        self.builder.position_at_end(bad_state_bb);
        self.emit_exit_with_code(span, 3)?;

        self.env.push_scope();

        self.builder.position_at_end(state0_bb);
        for custom in &custom_siblings {
            self.push_effect_unwind_target(&custom.arm.op.op.fqn, custom.catch_bb);
        }
        self.push_raise_target(main_raise_target);
        let target_ptr = self.codegen_immediate_resume_prefix_to_site(
            immediate_exec_plan,
            0,
            &handle.body.stmts,
            &immediate_binder_slots,
            resume_used_ptr,
            arm_bb,
        )?;
        self.pop_raise_target();
        for _ in custom_siblings.iter().rev() {
            self.pop_effect_unwind_target();
        }

        self.builder.position_at_end(arm_bb);
        let _ = self.builder.build_call(
            rt_swap,
            &[escape_outer_top.into()],
            "multi_resume_mixed_detach_for_immediate_arm",
        )?;
        self.env.push_scope();
        for slot in &immediate_binder_slots {
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
            resume_symbol,
            resume_value_ty,
            resume_value_ptr,
            resume_used_ptr,
            state_ptr,
            next_state: 1,
            _marker: std::marker::PhantomData,
        };
        self.push_immediate_resume_ctx(resume_ctx);
        for custom in &custom_siblings {
            self.push_effect_unwind_target(&custom.arm.op.op.fqn, finally_unwind_bb);
        }
        self.push_raise_target(finally_unwind_bb);
        let _ = self.codegen_expr_in_expected_context(&immediate_arm.body, Some(CgTy::Unit))?;
        self.pop_raise_target();
        for _ in custom_siblings.iter().rev() {
            self.pop_effect_unwind_target();
        }
        self.pop_immediate_resume_ctx();

        let arm_insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let arm_func = arm_insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;
        let resume_ok_bb = self
            .context
            .append_basic_block(arm_func, "handle_multi_resume_mixed_resume_ok");
        let resume_missing_bb = self
            .context
            .append_basic_block(arm_func, "handle_multi_resume_mixed_resume_missing");

        let used = self
            .builder
            .build_load(self.context.bool_type(), resume_used_ptr, "multi_resume_mixed_used")?
            .into_int_value();
        self.builder
            .build_conditional_branch(used, resume_ok_bb, resume_missing_bb)?;

        self.builder.position_at_end(resume_missing_bb);
        self.emit_exit_with_code(span, 3)?;

        self.builder.position_at_end(resume_ok_bb);
        let _ = self.builder.build_call(
            rt_swap,
            &[same_handle_restore_top.into()],
            "multi_resume_mixed_restore_after_immediate_arm",
        )?;
        self.builder.build_unconditional_branch(dispatch_bb)?;
        self.env.pop_scope();

        self.builder.position_at_end(state1_bb);
        for custom in &custom_siblings {
            self.push_effect_unwind_target(&custom.arm.op.op.fqn, custom.catch_bb);
        }
        self.push_raise_target(main_raise_target);
        if let Some(ptr) = resume_value_ptr {
            let llvm_ty = self.llvm_basic_type_of(span, resume_value_ty)?;
            let loaded = self
                .builder
                .build_load(llvm_ty, ptr, "multi_resume_mixed_resume_value")?;
            let value = CgValue {
                ty: resume_value_ty,
                value: Some(loaded),
            };
            let _ = self.store_local_value(span, target_ptr, resume_value_ty, value)?;
        }
        self.codegen_multi_resuming_mixed_continue_after_immediate_site(
            span,
            handle,
            &perform_site,
            out_ty,
            result_ptr,
            finally_bb,
            scanned_sites.as_slice(),
            initial_escape_binder_slots_by_site.as_slice(),
            escape_arm_entry_bbs.as_slice(),
            step_fn,
            cont_ptr,
            Some(continuation_created_ptr),
            state_ty,
            state_raw,
            state_gc_ptr,
            &outer_visible_supported,
            outer_field_base,
            &body_visible_supported,
            body_field_base,
            pc_field_idx,
        )?;
        self.pop_raise_target();
        for _ in custom_siblings.iter().rev() {
            self.pop_effect_unwind_target();
        }
        self.env.pop_scope();

        for (site_idx, entry_bb) in escape_arm_entry_bbs.iter().enumerate() {
            self.builder.position_at_end(*entry_bb);
            let _ = self.builder.build_call(
                rt_swap,
                &[escape_outer_top.into()],
                "multi_resume_mixed_detach_for_escape_arm",
            )?;
            self.builder
                .build_unconditional_branch(escape_arm_body_bbs[site_idx])?;
        }

        for (site_idx, arm_bb) in escape_arm_body_bbs.iter().enumerate() {
            let plan = &scanned_sites[site_idx];
            self.builder.position_at_end(*arm_bb);
            self.env.push_scope();
            for slot in &initial_escape_binder_slots_by_site[site_idx] {
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
                        escape_arm_unwind_bbs[site_idx],
                    );
                }
                self.push_raise_target(escape_arm_unwind_bbs[site_idx]);
            } else {
                self.push_raise_target(finally_unwind_bb);
            }
            let arm_v = self.codegen_expr_in_expected_context(&plan.arm.body, Some(out_ty))?;
            self.pop_raise_target();
            if has_sibling_nonresuming {
                for _ in custom_siblings.iter().rev() {
                    self.pop_effect_unwind_target();
                }
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
                self.builder.build_unconditional_branch(finally_bb)?;
            }

            if has_sibling_nonresuming {
                self.builder.position_at_end(escape_arm_unwind_bbs[site_idx]);
                let llvm_ref_ty = self.llvm_basic_type_of(span, CgTy::Ref)?;
                let k_loaded = self
                    .builder
                    .build_load(
                        llvm_ref_ty,
                        cont_ptr,
                        "multi_resume_mixed_k_unpin_load_unwind",
                    )?
                    .into_pointer_value();
                let unpin = self.declare_runtime_gc_unpin();
                let _ = self.builder.build_call(
                    unpin,
                    &[k_loaded.into()],
                    "multi_resume_mixed_k_unpin_unwind",
                )?;
                self.builder.build_unconditional_branch(finally_unwind_bb)?;
            }
        }

        if !has_sibling_nonresuming {
            for unwind_bb in &escape_arm_unwind_bbs {
                self.builder.position_at_end(*unwind_bb);
                self.builder.build_unreachable()?;
            }
        }

        self.builder.position_at_end(finally_unwind_bb);
        let _ = self.builder.build_call(
            rt_swap,
            &[escape_outer_top.into()],
            "multi_resume_mixed_finally_unwind_detach",
        )?;
        if let Some(finally) = handle.finally.as_ref() {
            let _ = self.codegen_block_value(finally)?;
        }
        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_none()
        {
            let cleanup_dispatch_bb = self
                .context
                .append_basic_block(func, "multi_resume_mixed_unwind_cleanup_dispatch");
            let cleanup_cont_bb = self
                .context
                .append_basic_block(func, "multi_resume_mixed_unwind_cleanup_cont");
            let cleanup_no_cont_bb = self
                .context
                .append_basic_block(func, "multi_resume_mixed_unwind_cleanup_no_cont");
            let propagate_bb = self
                .context
                .append_basic_block(func, "multi_resume_mixed_unwind_propagate");
            self.builder.build_unconditional_branch(cleanup_dispatch_bb)?;

            self.builder.position_at_end(cleanup_dispatch_bb);
            let created = self
                .builder
                .build_load(
                    self.context.bool_type(),
                    continuation_created_ptr,
                    "multi_resume_mixed_unwind_created",
                )?
                .into_int_value();
            self.builder
                .build_conditional_branch(created, cleanup_cont_bb, cleanup_no_cont_bb)?;

            self.builder.position_at_end(cleanup_cont_bb);
            let llvm_ref_ty = self.llvm_basic_type_of(span, CgTy::Ref)?;
            let k_loaded = self
                .builder
                .build_load(
                    llvm_ref_ty,
                    cont_ptr,
                    "multi_resume_mixed_unwind_k_unpin_load",
                )?
                .into_pointer_value();
            let unpin = self.declare_runtime_gc_unpin();
            let _ = self.builder.build_call(
                unpin,
                &[k_loaded.into()],
                "multi_resume_mixed_unwind_k_unpin",
            )?;
            self.builder.build_unconditional_branch(propagate_bb)?;

            self.builder.position_at_end(cleanup_no_cont_bb);
            let unpin = self.declare_runtime_gc_unpin();
            let _ = self.builder.build_call(
                unpin,
                &[state_raw.into()],
                "multi_resume_mixed_unwind_state_unpin",
            )?;
            self.builder.build_unconditional_branch(propagate_bb)?;

            self.builder.position_at_end(propagate_bb);
            if let Some(target) = outer_raise_target {
                self.builder.build_unconditional_branch(target)?;
            } else {
                let ret_ty =
                    self.current_fun_return_ty
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle unified mixed multi-resuming leaf finally unwind needs function return type",
                            at: span.into(),
                        })?;
                let value = self.default_value(span, ret_ty)?;
                self.emit_return(span, ret_ty, value)?;
            }
        }

        let cleanup_cont_bb = self
            .context
            .append_basic_block(func, "multi_resume_mixed_done_cleanup_cont");
        let cleanup_no_cont_bb = self
            .context
            .append_basic_block(func, "multi_resume_mixed_done_cleanup_no_cont");
        let result_bb = self
            .context
            .append_basic_block(func, "multi_resume_mixed_done_result");

        self.builder.position_at_end(finally_bb);
        let _ = self.builder.build_call(
            rt_swap,
            &[escape_outer_top.into()],
            "multi_resume_mixed_finally_detach",
        )?;
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
                    kind: "mixed sibling dispatch missing no-match block",
                    at: span.into(),
                });
            };

            self.builder.position_at_end(effect_dispatch_bb);
            let rt_read_tag = self.declare_runtime_effect_perform_slot_read_op_tag();
            let tag_call = self.builder.build_call(
                rt_read_tag,
                &[],
                "handle_multi_resume_mixed_dispatch_read_op_tag",
            )?;
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
            let _ = self.builder.build_call(
                rt_swap,
                &[escape_outer_top.into()],
                "handle_multi_resume_mixed_dispatch_detach",
            )?;
            self.builder.build_unconditional_branch(finally_unwind_bb)?;
        }

        if let (Some(raise_arm), Some(raise_catch_bb)) = (raise_sibling, raise_catch_bb) {
            let binder = &raise_arm.op.binders[0];
            self.builder.position_at_end(raise_catch_bb);

            let _ = self.builder.build_call(
                rt_swap,
                &[escape_outer_top.into()],
                "handle_multi_resume_mixed_raise_detach",
            )?;

            let rt_len = self.declare_runtime_effect_perform_slot_read_len_words();
            let call = self.builder.build_call(
                rt_len,
                &[],
                "multi_resume_mixed_raise_read_slot_len_words",
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
                "multi_resume_mixed_raise_slot_len_ok",
            )?;
            let len_ok_bb = self
                .context
                .append_basic_block(func, "multi_resume_mixed_raise_slot_len_ok_bb");
            let len_bad_bb = self
                .context
                .append_basic_block(func, "multi_resume_mixed_raise_slot_len_bad_bb");
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
                "multi_resume_mixed_raise_read_slot_word0",
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
                "multi_resume_mixed_raise_read_slot_word1",
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
                .build_call(rt_clear, &[], "multi_resume_mixed_raise_clear")?;

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
                        "multi_resume_mixed_raise_kind_is_int",
                    )?;
                    let ok_bb = self
                        .context
                        .append_basic_block(func, "multi_resume_mixed_raise_kind_int_ok");
                    let bad_bb = self
                        .context
                        .append_basic_block(func, "multi_resume_mixed_raise_kind_int_bad");
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
                        "multi_resume_mixed_raise_kind_is_runtime_error",
                    )?;
                    let ok_bb = self.context.append_basic_block(
                        func,
                        "multi_resume_mixed_raise_kind_runtime_error_ok",
                    );
                    let bad_bb = self.context.append_basic_block(
                        func,
                        "multi_resume_mixed_raise_kind_runtime_error_bad",
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
                        "multi_resume_mixed_raise_runtime_error_tag_i32",
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
                        "multi_resume_mixed_raise_runtime_error_tag",
                    )?;
                    agg = self.builder.build_insert_value(
                        agg,
                        payload_word_zero,
                        1,
                        "multi_resume_mixed_raise_runtime_error_payload_word",
                    )?;
                    agg = self.builder.build_insert_value(
                        agg,
                        payload_ptr_zero,
                        2,
                        "multi_resume_mixed_raise_runtime_error_payload_ptr",
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
            } else if out_ty == CgTy::Never {
                CgValue::never()
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

            let _ = self.builder.build_call(
                rt_swap,
                &[escape_outer_top.into()],
                "handle_multi_resume_mixed_custom_detach",
            )?;

            let rt_len = self.declare_runtime_effect_perform_slot_read_len_words();
            let call = self.builder.build_call(
                rt_len,
                &[],
                "multi_resume_mixed_custom_read_slot_len_words",
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
                "multi_resume_mixed_custom_slot_len_ok",
            )?;
            let len_ok_bb = self
                .context
                .append_basic_block(func, "multi_resume_mixed_custom_slot_len_ok_bb");
            let len_bad_bb = self
                .context
                .append_basic_block(func, "multi_resume_mixed_custom_slot_len_bad_bb");
            self.builder
                .build_conditional_branch(len_ok, len_ok_bb, len_bad_bb)?;

            self.builder.position_at_end(len_bad_bb);
            self.emit_exit_with_code(span, 3)?;

            self.builder.position_at_end(len_ok_bb);
            let rt_read = self.declare_runtime_effect_perform_slot_read_u64();
            let value_call =
                self.builder
                    .build_call(rt_read, &[], "multi_resume_mixed_custom_read_slot_word0")?;
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
                "multi_resume_mixed_custom_read_slot_gc_ref",
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
                .build_call(rt_clear, &[], "multi_resume_mixed_custom_clear")?;

            self.push_effect_unwind_target(&custom.arm.op.op.fqn, finally_unwind_bb);
            self.push_raise_target(finally_unwind_bb);
            let arm_v = self.codegen_expr_in_expected_context(&arm.body, Some(out_ty))?;
            self.pop_raise_target();
            self.pop_effect_unwind_target();
            let arm_v = if out_ty == CgTy::Unit {
                CgValue::unit()
            } else if out_ty == CgTy::Never {
                CgValue::never()
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
        let created = self
            .builder
            .build_load(
                self.context.bool_type(),
                continuation_created_ptr,
                "multi_resume_mixed_done_created",
            )?
            .into_int_value();
        self.builder
            .build_conditional_branch(created, cleanup_cont_bb, cleanup_no_cont_bb)?;

        self.builder.position_at_end(cleanup_cont_bb);
        let llvm_ref_ty = self.llvm_basic_type_of(span, CgTy::Ref)?;
        let k_loaded = self
            .builder
            .build_load(llvm_ref_ty, cont_ptr, "multi_resume_mixed_k_unpin_load")?
            .into_pointer_value();
        let unpin = self.declare_runtime_gc_unpin();
        let _ = self
            .builder
            .build_call(unpin, &[k_loaded.into()], "multi_resume_mixed_k_unpin")?;
        self.builder.build_unconditional_branch(result_bb)?;

        self.builder.position_at_end(cleanup_no_cont_bb);
        let unpin = self.declare_runtime_gc_unpin();
        let _ = self.builder.build_call(
            unpin,
            &[state_raw.into()],
            "multi_resume_mixed_state_unpin_no_cont",
        )?;
        self.builder.build_unconditional_branch(result_bb)?;

        self.builder.position_at_end(result_bb);
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
                    .build_load(llvm_ty, ptr, "handle_multi_resume_mixed_result")?;
                CgValue {
                    ty: out_ty,
                    value: Some(loaded),
                }
            }
        })
    }
}
