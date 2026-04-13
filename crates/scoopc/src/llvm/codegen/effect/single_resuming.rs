fn resume_frame_same_structure<'a>(a: &ResumeFrame<'a>, b: &ResumeFrame<'a>) -> bool {
    match (a, b) {
        (
            ResumeFrame::IfThen { if_expr: a_e, .. },
            ResumeFrame::IfThen { if_expr: b_e, .. },
        ) => std::ptr::eq(*a_e, *b_e),
        (
            ResumeFrame::IfElse { if_expr: a_e, .. },
            ResumeFrame::IfElse { if_expr: b_e, .. },
        ) => std::ptr::eq(*a_e, *b_e),
        (
            ResumeFrame::WhenArm {
                when_expr: a_e,
                arm_index: a_i,
                ..
            },
            ResumeFrame::WhenArm {
                when_expr: b_e,
                arm_index: b_i,
                ..
            },
        ) => std::ptr::eq(*a_e, *b_e) && a_i == b_i,
        (
            ResumeFrame::WhileBody { while_body: a_b, .. },
            ResumeFrame::WhileBody { while_body: b_b, .. },
        ) => std::ptr::eq(*a_b, *b_b),
        (ResumeFrame::Block { block: a_b, .. }, ResumeFrame::Block { block: b_b, .. }) => {
            std::ptr::eq(*a_b, *b_b)
        }
        _ => false,
    }
}

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    fn record_escape_decl_info_in_stmts<'hir>(
        stmts: &'hir [hir::Stmt],
        decl_map: &mut HashMap<hir::SymbolId, DeclInfo<'hir>>,
    ) {
        for stmt in stmts {
            match &stmt.kind {
                hir::StmtKind::Val(decl) => {
                    if let Some(id) = decl.id {
                        decl_map.entry(id).or_insert(DeclInfo { decl });
                    }
                }
                hir::StmtKind::Expr(expr) => Self::record_escape_decl_info_in_expr(expr, decl_map),
                hir::StmtKind::While { body, .. } => {
                    Self::record_escape_decl_info_in_stmts(&body.stmts, decl_map);
                }
                hir::StmtKind::Assign { .. }
                | hir::StmtKind::Return { .. }
                | hir::StmtKind::Empty
                | hir::StmtKind::Break { .. }
                | hir::StmtKind::Continue { .. }
                | hir::StmtKind::Todo(_) => {}
            }
        }
    }

    fn record_escape_decl_info_in_expr<'hir>(
        expr: &'hir hir::Expr,
        decl_map: &mut HashMap<hir::SymbolId, DeclInfo<'hir>>,
    ) {
        match &expr.kind {
            hir::ExprKind::If {
                then_branch,
                else_branch,
                ..
            } => {
                if let hir::ExprKind::Block(block) = &then_branch.kind {
                    Self::record_escape_decl_info_in_stmts(&block.stmts, decl_map);
                }
                if let Some(else_expr) = else_branch.as_deref()
                    && let hir::ExprKind::Block(block) = &else_expr.kind
                {
                    Self::record_escape_decl_info_in_stmts(&block.stmts, decl_map);
                }
            }
            hir::ExprKind::When { arms, .. } => {
                for when_arm in arms {
                    if let hir::ExprKind::Block(block) = &when_arm.body.kind {
                        Self::record_escape_decl_info_in_stmts(&block.stmts, decl_map);
                    }
                }
            }
            hir::ExprKind::Block(block) => {
                Self::record_escape_decl_info_in_stmts(&block.stmts, decl_map);
            }
            _ => {}
        }
    }

    fn collect_escape_decl_map<'hir>(
        handle: &'hir hir::HandleExpr,
    ) -> HashMap<hir::SymbolId, DeclInfo<'hir>> {
        let mut decl_map = HashMap::new();
        Self::record_escape_decl_info_in_stmts(&handle.body.stmts, &mut decl_map);
        decl_map
    }

    fn codegen_immediate_resume_stmt_unit(
        &mut self,
        stmt: &hir::Stmt,
    ) -> Result<(), LlvmEmitError> {
        match &stmt.kind {
            hir::StmtKind::Empty => Ok(()),
            hir::StmtKind::Val(decl) => self.codegen_val_decl(decl),
            hir::StmtKind::Assign { lhs, eq_span, rhs } => {
                self.codegen_assign_stmt(*eq_span, lhs, rhs)
            }
            hir::StmtKind::Expr(expr) => {
                let _ = self.codegen_expr(expr)?;
                Ok(())
            }
            hir::StmtKind::Return { .. } => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "`return` inside handle resume body",
                at: stmt.span.into(),
            }),
            hir::StmtKind::While { .. }
            | hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Todo(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "statement inside handle resume body",
                at: stmt.span.into(),
            }),
        }
    }

    fn codegen_immediate_resume_site_binding<'hir>(
        &mut self,
        site: &'hir ImmediateResumeSite<'hir>,
        decl: &'hir hir::ValDecl,
        arm_dispatch: ImmediateResumeArmDispatch<'_, 'ctx>,
        reuse_target_ptr: Option<PointerValue<'ctx>>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let Some(init) = decl.init.as_ref() else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle resume body (missing perform init)",
                at: decl.span.into(),
            });
        };
        let hir::ExprKind::Perform { op, args } = &init.kind else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle resume body (expected perform binding)",
                at: init.span.into(),
            });
        };
        if op.fqn != site.op.fqn {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle resume op mismatch",
                at: op.span.into(),
            });
        }

        let resume_value_ty = self
            .cg_ty_of(decl.ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "handle resume perform value type",
                at: decl.span.into(),
            })?;

        let target_ptr = if let Some(ptr) = reuse_target_ptr {
            self.env.insert(
                site.id,
                CgLocal {
                    hir_ty: Some(decl.ty),
                    ty: resume_value_ty,
                    ptr,
                    mutable: decl.mutable,
                },
            );
            ptr
        } else if let Some(local) = self.env.get(site.id) {
            if local.ty != resume_value_ty {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle resume perform value type",
                    at: decl.span.into(),
                });
            }
            local.ptr
        } else {
            let name = decl.name.as_deref().unwrap_or("perform_value");
            let ptr = self.create_entry_alloca(decl.span, name, resume_value_ty)?;
            self.env.insert(
                site.id,
                CgLocal {
                    hir_ty: Some(decl.ty),
                    ty: resume_value_ty,
                    ptr,
                    mutable: decl.mutable,
                },
            );
            ptr
        };

        for (slot_idx, arg) in args.iter().enumerate() {
            let hir::CallArg::Positional(expr) = arg else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle resume perform args (named arg not supported)",
                    at: decl.span.into(),
                });
            };
            let slot = &arm_dispatch.binder_slots[slot_idx];
            if slot.ty == CgTy::Unit {
                continue;
            }
            let v = self.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
            let v = self.coerce_value(expr.span, v, slot.ty)?;
            let _stored = self.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
        }

        let _ = self.builder.build_store(
            arm_dispatch.resume_used_ptr,
            self.context.bool_type().const_int(0, false),
        )?;
        self.builder
            .build_unconditional_branch(arm_dispatch.arm_bb)?;
        Ok(target_ptr)
    }

    fn codegen_immediate_resume_while_iteration_to_site<'hir>(
        &mut self,
        plan: ImmediateResumeExecPlan<'hir, 'ctx>,
        depth: usize,
        while_body: &'hir hir::Block,
        arm_dispatch: ImmediateResumeArmDispatch<'_, 'ctx>,
        reuse_target_ptr: Option<PointerValue<'ctx>>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let perform_stmt_idx = plan.site.resume_path[depth].stmt_idx();
        for (idx, stmt) in while_body.stmts.iter().enumerate() {
            if idx < perform_stmt_idx {
                self.codegen_immediate_resume_stmt_unit(stmt)?;
                continue;
            }

            let hir::StmtKind::Val(decl) = &stmt.kind else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle resume body (expected perform binding)",
                    at: stmt.span.into(),
                });
            };
            return self.codegen_immediate_resume_site_binding(
                plan.site,
                decl,
                arm_dispatch,
                reuse_target_ptr,
            );
        }

        Err(LlvmEmitError::UnsupportedMainBody {
            kind: "handle resume body (perform site missing)",
            at: plan.site.decl.span.into(),
        })
    }

    fn codegen_immediate_resume_while_tail_and_continue<'hir>(
        &mut self,
        plan: ImmediateResumeExecPlan<'hir, 'ctx>,
        depth: usize,
        start_idx: usize,
        while_frame: (&'hir hir::Expr, &'hir hir::Block),
        target_ptr: PointerValue<'ctx>,
        arm_dispatch: ImmediateResumeArmDispatch<'_, 'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let (while_cond, while_body) = while_frame;
        for stmt in while_body.stmts.iter().skip(start_idx) {
            self.codegen_immediate_resume_stmt_unit(stmt)?;
        }
        self.env.pop_scope();

        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: while_body.span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: while_body.span.into(),
            })?;
        let cond_bb = self
            .context
            .append_basic_block(func, "handle_resume_while_cond");
        let body_bb = self
            .context
            .append_basic_block(func, "handle_resume_while_body");
        let after_bb = self
            .context
            .append_basic_block(func, "handle_resume_while_after");

        self.builder.build_unconditional_branch(cond_bb)?;

        self.builder.position_at_end(cond_bb);
        let cond_v = self.codegen_expr_in_expected_context(while_cond, Some(CgTy::Bool))?;
        let cond_v = self.coerce_value(while_cond.span, cond_v, CgTy::Bool)?;
        let cond_i1 = cond_v.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "handle resume body (while condition value)",
            at: while_cond.span.into(),
        })?;
        self.builder
            .build_conditional_branch(cond_i1, body_bb, after_bb)?;

        self.builder.position_at_end(after_bb);
        self.codegen_immediate_resume_continue_after_frame_and_finalize(plan, depth)?;

        self.builder.position_at_end(body_bb);
        self.env.push_scope();
        let _ = self.codegen_immediate_resume_while_iteration_to_site(
            plan,
            depth,
            while_body,
            arm_dispatch,
            Some(target_ptr),
        )?;
        Ok(())
    }

    fn codegen_immediate_resume_finalize_body(
        &mut self,
        value_span: crate::span::Span,
        out_ty: CgTy,
        value: CgValue<'ctx>,
        result_ptr: Option<PointerValue<'ctx>>,
        handler_exit: ImmediateResumeHandlerExit<'ctx>,
        finally_bb: inkwell::basic_block::BasicBlock<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let value = if out_ty == CgTy::Unit {
            CgValue::unit()
        } else {
            self.coerce_value(value_span, value, out_ty)?
        };
        if let Some(ptr) = result_ptr {
            let _ = self.store_local_value(value_span, ptr, out_ty, value)?;
        }

        match handler_exit {
            ImmediateResumeHandlerExit::None => {}
            ImmediateResumeHandlerExit::PopFrame(handler_frame_ptr) => {
                let rt_pop = self.declare_runtime_effect_handler_stack_pop();
                let i8_ptr_ty = self.llvm_i8_ptr_type();
                let frame_i8 = self.builder.build_bit_cast(
                    handler_frame_ptr,
                    i8_ptr_ty,
                    "handle_resume_effect_frame_i8",
                )?;
                let _ = self.builder.build_call(
                    rt_pop,
                    &[frame_i8.into()],
                    "handle_resume_effect_pop",
                )?;
            }
            ImmediateResumeHandlerExit::SwapTop(new_top) => {
                let rt_swap = self.declare_runtime_effect_handler_stack_swap_top();
                let _ = self.builder.build_call(
                    rt_swap,
                    &[new_top.into()],
                    "handle_resume_effect_swap_top",
                )?;
            }
        }
        self.builder.build_unconditional_branch(finally_bb)?;
        Ok(())
    }

    fn codegen_immediate_resume_top_level_tail_and_finalize(
        &mut self,
        handle: &hir::HandleExpr,
        start_idx: usize,
        out_ty: CgTy,
        result_ptr: Option<PointerValue<'ctx>>,
        handler_exit: ImmediateResumeHandlerExit<'ctx>,
        finally_bb: inkwell::basic_block::BasicBlock<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let mut value: CgValue<'ctx> = CgValue::unit();
        for (idx, stmt) in handle.body.stmts.iter().enumerate().skip(start_idx) {
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

        self.codegen_immediate_resume_finalize_body(
            handle.body.span,
            out_ty,
            value,
            result_ptr,
            handler_exit,
            finally_bb,
        )
    }

    fn codegen_immediate_resume_continue_after_frame_and_finalize<'hir>(
        &mut self,
        plan: ImmediateResumeExecPlan<'hir, 'ctx>,
        completed_depth: usize,
    ) -> Result<(), LlvmEmitError> {
        if completed_depth == 0 {
            return self.codegen_immediate_resume_top_level_tail_and_finalize(
                plan.handle,
                plan.site.top_level_stmt_idx + 1,
                plan.out_ty,
                plan.result_ptr,
                plan.handler_exit,
                plan.finally_bb,
            );
        }

        let parent_start_idx = plan.site.resume_path[completed_depth - 1].stmt_idx() + 1;
        self.codegen_immediate_resume_frame_tail_and_continue(
            plan,
            completed_depth - 1,
            parent_start_idx,
        )
    }

    fn codegen_immediate_resume_frame_tail_and_continue<'hir>(
        &mut self,
        plan: ImmediateResumeExecPlan<'hir, 'ctx>,
        depth: usize,
        start_idx: usize,
    ) -> Result<(), LlvmEmitError> {
        let stmts = match &plan.site.resume_path[depth] {
            ImmediateResumeFrame::Block { block, .. } => &block.stmts,
            ImmediateResumeFrame::IfThen { then_block, .. } => &then_block.stmts,
            ImmediateResumeFrame::IfElse { else_block, .. } => &else_block.stmts,
            ImmediateResumeFrame::WhileBody { while_body, .. } => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle resume body (while frame needs specialized lowering)",
                    at: while_body.span.into(),
                });
            }
        };
        for stmt in stmts.iter().skip(start_idx) {
            self.codegen_immediate_resume_stmt_unit(stmt)?;
        }
        self.env.pop_scope();
        self.codegen_immediate_resume_continue_after_frame_and_finalize(plan, depth)
    }

    fn codegen_immediate_resume_non_intercept_branch_and_continue<'hir>(
        &mut self,
        plan: ImmediateResumeExecPlan<'hir, 'ctx>,
        depth: usize,
        branch_expr: Option<&'hir hir::Expr>,
    ) -> Result<(), LlvmEmitError> {
        let saved_env = self.env.clone();
        if let Some(expr) = branch_expr {
            let _ = self.codegen_expr(expr)?;
        }
        if let Some(bb) = self.builder.get_insert_block()
            && bb.get_terminator().is_none()
        {
            self.codegen_immediate_resume_continue_after_frame_and_finalize(plan, depth)?;
        }
        self.env = saved_env;
        Ok(())
    }

    fn codegen_immediate_resume_prefix_to_site<'hir>(
        &mut self,
        plan: ImmediateResumeExecPlan<'hir, 'ctx>,
        depth: usize,
        stmts: &'hir [hir::Stmt],
        binder_slots: &[ImmediateResumeBinderSlot<'ctx>],
        resume_used_ptr: PointerValue<'ctx>,
        arm_bb: inkwell::basic_block::BasicBlock<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let target_stmt_idx = if depth == 0 {
            plan.site.top_level_stmt_idx
        } else {
            plan.site.resume_path[depth - 1].stmt_idx()
        };
        for (idx, stmt) in stmts.iter().enumerate() {
            if idx < target_stmt_idx {
                self.codegen_immediate_resume_stmt_unit(stmt)?;
                continue;
            }

            if depth == plan.site.resume_path.len() {
                let hir::StmtKind::Val(decl) = &stmt.kind else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle resume body (expected perform binding)",
                        at: stmt.span.into(),
                    });
                };
                return self.codegen_immediate_resume_site_binding(
                    plan.site,
                    decl,
                    ImmediateResumeArmDispatch {
                        binder_slots,
                        resume_used_ptr,
                        arm_bb,
                    },
                    None,
                );
            }

            match &plan.site.resume_path[depth] {
                ImmediateResumeFrame::Block {
                    block: expected_block,
                    ..
                } => {
                    let hir::StmtKind::Expr(expr) = &stmt.kind else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle resume body (expected block statement)",
                            at: stmt.span.into(),
                        });
                    };
                    let hir::ExprKind::Block(block) = &expr.kind else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle resume body (expected block statement)",
                            at: expr.span.into(),
                        });
                    };
                    if !std::ptr::eq(block, *expected_block) {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle resume body (block path mismatch)",
                            at: expr.span.into(),
                        });
                    }

                    self.env.push_scope();
                    return self.codegen_immediate_resume_prefix_to_site(
                        plan,
                        depth + 1,
                        &block.stmts,
                        binder_slots,
                        resume_used_ptr,
                        arm_bb,
                    );
                }
                ImmediateResumeFrame::IfThen {
                    if_expr,
                    then_block,
                    ..
                } => {
                    let hir::StmtKind::Expr(expr) = &stmt.kind else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle resume body (expected if statement)",
                            at: stmt.span.into(),
                        });
                    };
                    if !std::ptr::eq(expr, *if_expr) {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle resume body (if path mismatch)",
                            at: expr.span.into(),
                        });
                    }
                    let hir::ExprKind::If {
                        cond,
                        then_branch: _,
                        else_branch,
                    } = &expr.kind
                    else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle resume body (expected if statement)",
                            at: expr.span.into(),
                        });
                    };

                    let cond_v = self.codegen_expr_in_expected_context(cond, Some(CgTy::Bool))?;
                    let cond_v = self.coerce_value(cond.span, cond_v, CgTy::Bool)?;
                    let cond_i1 = cond_v.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle resume body (if condition value)",
                        at: cond.span.into(),
                    })?;

                    let insert_block = self.builder.get_insert_block().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "builder has no insert block",
                            at: expr.span.into(),
                        },
                    )?;
                    let func =
                        insert_block
                            .get_parent()
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "builder has no parent function",
                                at: expr.span.into(),
                            })?;
                    let then_bb = self
                        .context
                        .append_basic_block(func, "handle_resume_if_then");
                    let else_bb = self
                        .context
                        .append_basic_block(func, "handle_resume_if_else");
                    self.builder
                        .build_conditional_branch(cond_i1, then_bb, else_bb)?;

                    self.builder.position_at_end(else_bb);
                    self.codegen_immediate_resume_non_intercept_branch_and_continue(
                        plan,
                        depth,
                        else_branch.as_deref(),
                    )?;

                    self.builder.position_at_end(then_bb);
                    self.env.push_scope();
                    return self.codegen_immediate_resume_prefix_to_site(
                        plan,
                        depth + 1,
                        &then_block.stmts,
                        binder_slots,
                        resume_used_ptr,
                        arm_bb,
                    );
                }
                ImmediateResumeFrame::IfElse {
                    if_expr,
                    else_block,
                    ..
                } => {
                    let hir::StmtKind::Expr(expr) = &stmt.kind else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle resume body (expected if statement)",
                            at: stmt.span.into(),
                        });
                    };
                    if !std::ptr::eq(expr, *if_expr) {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle resume body (if path mismatch)",
                            at: expr.span.into(),
                        });
                    }
                    let hir::ExprKind::If {
                        cond,
                        then_branch,
                        else_branch: _,
                    } = &expr.kind
                    else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle resume body (expected if statement)",
                            at: expr.span.into(),
                        });
                    };

                    let cond_v = self.codegen_expr_in_expected_context(cond, Some(CgTy::Bool))?;
                    let cond_v = self.coerce_value(cond.span, cond_v, CgTy::Bool)?;
                    let cond_i1 = cond_v.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle resume body (if condition value)",
                        at: cond.span.into(),
                    })?;

                    let insert_block = self.builder.get_insert_block().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "builder has no insert block",
                            at: expr.span.into(),
                        },
                    )?;
                    let func =
                        insert_block
                            .get_parent()
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "builder has no parent function",
                                at: expr.span.into(),
                            })?;
                    let then_bb = self
                        .context
                        .append_basic_block(func, "handle_resume_if_then");
                    let else_bb = self
                        .context
                        .append_basic_block(func, "handle_resume_if_else");
                    self.builder
                        .build_conditional_branch(cond_i1, then_bb, else_bb)?;

                    self.builder.position_at_end(then_bb);
                    self.codegen_immediate_resume_non_intercept_branch_and_continue(
                        plan,
                        depth,
                        Some(then_branch),
                    )?;

                    self.builder.position_at_end(else_bb);
                    self.env.push_scope();
                    return self.codegen_immediate_resume_prefix_to_site(
                        plan,
                        depth + 1,
                        &else_block.stmts,
                        binder_slots,
                        resume_used_ptr,
                        arm_bb,
                    );
                }
                ImmediateResumeFrame::WhileBody {
                    while_cond: expected_cond,
                    while_body: expected_body,
                    ..
                } => {
                    let hir::StmtKind::While { cond, body } = &stmt.kind else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle resume body (expected while statement)",
                            at: stmt.span.into(),
                        });
                    };
                    if !std::ptr::eq(cond, *expected_cond) || !std::ptr::eq(body, *expected_body) {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle resume body (while path mismatch)",
                            at: stmt.span.into(),
                        });
                    }

                    let insert_block = self.builder.get_insert_block().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "builder has no insert block",
                            at: stmt.span.into(),
                        },
                    )?;
                    let func =
                        insert_block
                            .get_parent()
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "builder has no parent function",
                                at: stmt.span.into(),
                            })?;
                    let cond_bb = self
                        .context
                        .append_basic_block(func, "handle_resume_while_cond");
                    let body_bb = self
                        .context
                        .append_basic_block(func, "handle_resume_while_body");
                    let after_bb = self
                        .context
                        .append_basic_block(func, "handle_resume_while_after");

                    self.builder.build_unconditional_branch(cond_bb)?;

                    self.builder.position_at_end(cond_bb);
                    let cond_v = self.codegen_expr_in_expected_context(cond, Some(CgTy::Bool))?;
                    let cond_v = self.coerce_value(cond.span, cond_v, CgTy::Bool)?;
                    let cond_i1 = cond_v.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle resume body (while condition value)",
                        at: cond.span.into(),
                    })?;
                    self.builder
                        .build_conditional_branch(cond_i1, body_bb, after_bb)?;

                    self.builder.position_at_end(after_bb);
                    self.codegen_immediate_resume_continue_after_frame_and_finalize(plan, depth)?;

                    self.builder.position_at_end(body_bb);
                    self.env.push_scope();
                    return self.codegen_immediate_resume_while_iteration_to_site(
                        plan,
                        depth,
                        body,
                        ImmediateResumeArmDispatch {
                            binder_slots,
                            resume_used_ptr,
                            arm_bb,
                        },
                        None,
                    );
                }
            }
        }

        Err(LlvmEmitError::UnsupportedMainBody {
            kind: "handle resume body (perform site missing)",
            at: plan.site.decl.span.into(),
        })
    }

    pub(super) fn codegen_handle_expr_unified_single_immediate_resume_leaf<'hir>(
        &mut self,
        ctx: UnifiedSingleResumingLeafCtx<'hir, '_>,
        resume_symbol: hir::SymbolId,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let UnifiedSingleResumingLeafCtx {
            span,
            handle,
            state_machine_plan,
            arm,
            arm_id,
            out_ty,
        } = ctx;
        let Some(perform_site) = Self::resolve_immediate_resume_site_from_plan(
            handle,
            state_machine_plan,
            arm_id,
            &arm.op.op.fqn,
        )?
        else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle resume body (missing perform)",
                at: span.into(),
            });
        };
        let perform_idx = perform_site.top_level_stmt_idx;
        let perform_decl = perform_site.decl;
        let perform_op = perform_site.op;
        let perform_args = perform_site.args;

        if perform_op.fqn != arm.op.op.fqn {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle resume op mismatch",
                at: perform_op.span.into(),
            });
        }

        let resume_value_ty =
            self.cg_ty_of(perform_decl.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle resume perform value type",
                    at: perform_decl.span.into(),
                })?;

        if arm.op.binders.len() != perform_args.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle resume binder arity mismatch",
                at: arm.op.span.into(),
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

        let handler_frame_ty = self.llvm_effect_handler_frame_type();
        let handler_frame_ptr = self.create_entry_alloca_raw(
            span,
            "handle_resume_effect_frame",
            handler_frame_ty.into(),
        )?;

        let dispatch_bb = self
            .context
            .append_basic_block(func, "handle_resume_dispatch");
        let state0_bb = self
            .context
            .append_basic_block(func, "handle_resume_state0");
        let state1_bb = self
            .context
            .append_basic_block(func, "handle_resume_state1");
        let arm_bb = self.context.append_basic_block(func, "handle_resume_arm");
        let done_bb = self.context.append_basic_block(func, "handle_resume_done");
        let bad_state_bb = self
            .context
            .append_basic_block(func, "handle_resume_bad_state");
        let finally_bb = self
            .context
            .append_basic_block(func, "handle_resume_finally");
        let finally_unwind_bb = self
            .context
            .append_basic_block(func, "handle_resume_finally_unwind");

        let i32_ty = self.context.i32_type();
        let state_ptr = self.create_entry_alloca_raw(span, "handle_state", i32_ty.into())?;
        let resume_used_ptr = self.create_entry_alloca_raw(
            span,
            "handle_resume_used",
            self.context.bool_type().into(),
        )?;
        let resume_value_ptr = if resume_value_ty == CgTy::Unit {
            None
        } else {
            Some(self.create_entry_alloca(span, "handle_resume_value", resume_value_ty)?)
        };

        let result_ptr = if out_ty == CgTy::Unit {
            None
        } else {
            Some(self.create_entry_alloca(span, "handle_result", out_ty)?)
        };
        let exec_plan = ImmediateResumeExecPlan {
            handle,
            site: &perform_site,
            out_ty,
            result_ptr,
            handler_exit: ImmediateResumeHandlerExit::PopFrame(handler_frame_ptr),
            finally_bb,
        };

        let mut binder_slots: Vec<ImmediateResumeBinderSlot<'ctx>> = Vec::new();
        for binder in &arm.op.binders {
            let binder_ty = self
                .cg_ty_of(binder.ty)
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

        let _ = self.builder.build_store(state_ptr, i32_ty.const_zero())?;
        let _ = self.builder.build_store(
            resume_used_ptr,
            self.context.bool_type().const_int(0, false),
        )?;

        let rt_push = self.declare_runtime_effect_handler_stack_push();
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let frame_i8 = self.builder.build_bit_cast(
            handler_frame_ptr,
            i8_ptr_ty,
            "handle_resume_effect_frame_i8",
        )?;
        let resume_tag = self.effect_op_tag(&arm.op.op.fqn);
        let op_tag_i32 = self.context.i32_type().const_int(resume_tag as u64, false);
        let _ = self.builder.build_call(
            rt_push,
            &[frame_i8.into(), op_tag_i32.into()],
            "handle_resume_effect_push",
        )?;

        self.builder.build_unconditional_branch(dispatch_bb)?;

        self.builder.position_at_end(dispatch_bb);
        let state = self
            .builder
            .build_load(i32_ty, state_ptr, "handle_state")?
            .into_int_value();
        let cases = [
            (i32_ty.const_int(0, false), state0_bb),
            (i32_ty.const_int(1, false), state1_bb),
        ];
        self.builder.build_switch(state, bad_state_bb, &cases)?;

        self.builder.position_at_end(bad_state_bb);
        self.emit_exit_with_code(span, 3)?;

        self.env.push_scope();

        self.builder.position_at_end(state0_bb);
        self.push_raise_target(finally_unwind_bb);
        let target_ptr = self.codegen_immediate_resume_prefix_to_site(
            exec_plan,
            0,
            &handle.body.stmts,
            &binder_slots,
            resume_used_ptr,
            arm_bb,
        )?;
        self.pop_raise_target();

        self.builder.position_at_end(arm_bb);

        let rt_set_active = self.declare_runtime_effect_handler_stack_set_active();
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let frame_i8 = self.builder.build_bit_cast(
            handler_frame_ptr,
            i8_ptr_ty,
            "handle_resume_effect_frame_i8",
        )?;
        let inactive = self.context.i32_type().const_zero();
        let _ = self.builder.build_call(
            rt_set_active,
            &[frame_i8.into(), inactive.into()],
            "handle_resume_effect_inactive",
        )?;

        self.env.push_scope();
        for slot in &binder_slots {
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
        self.push_raise_target(finally_unwind_bb);
        let _ = self.codegen_expr_in_expected_context(&arm.body, Some(CgTy::Unit))?;
        self.pop_raise_target();
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
            .append_basic_block(func, "handle_resume_arm_ok");
        let resume_missing_bb = self
            .context
            .append_basic_block(func, "handle_resume_arm_missing");

        let used = self
            .builder
            .build_load(self.context.bool_type(), resume_used_ptr, "resume_used")?
            .into_int_value();
        self.builder
            .build_conditional_branch(used, resume_ok_bb, resume_missing_bb)?;

        self.builder.position_at_end(resume_missing_bb);
        self.emit_exit_with_code(span, 3)?;

        self.builder.position_at_end(resume_ok_bb);

        let rt_set_active = self.declare_runtime_effect_handler_stack_set_active();
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let frame_i8 = self.builder.build_bit_cast(
            handler_frame_ptr,
            i8_ptr_ty,
            "handle_resume_effect_frame_i8",
        )?;
        let active = self.context.i32_type().const_int(1, false);
        let _ = self.builder.build_call(
            rt_set_active,
            &[frame_i8.into(), active.into()],
            "handle_resume_effect_active",
        )?;

        self.builder.build_unconditional_branch(dispatch_bb)?;

        self.env.pop_scope();

        self.builder.position_at_end(state1_bb);
        self.push_raise_target(finally_unwind_bb);

        if let Some(ptr) = resume_value_ptr {
            let llvm_ty = self.llvm_basic_type_of(span, resume_value_ty)?;
            let loaded = self.builder.build_load(llvm_ty, ptr, "resume_value")?;
            let v = CgValue {
                ty: resume_value_ty,
                value: Some(loaded),
            };
            let _stored = self.store_local_value(span, target_ptr, resume_value_ty, v)?;
        }

        if perform_site.resume_path.is_empty() {
            self.codegen_immediate_resume_top_level_tail_and_finalize(
                handle,
                perform_idx + 1,
                out_ty,
                result_ptr,
                ImmediateResumeHandlerExit::PopFrame(handler_frame_ptr),
                finally_bb,
            )?;
        } else {
            let last_depth = perform_site.resume_path.len() - 1;
            let start_idx = perform_site.resume_path[last_depth].stmt_idx() + 1;
            match &perform_site.resume_path[last_depth] {
                ImmediateResumeFrame::WhileBody {
                    while_cond,
                    while_body,
                    ..
                } => {
                    self.codegen_immediate_resume_while_tail_and_continue(
                        exec_plan,
                        last_depth,
                        start_idx,
                        (while_cond, while_body),
                        target_ptr,
                        ImmediateResumeArmDispatch {
                            binder_slots: &binder_slots,
                            resume_used_ptr,
                            arm_bb,
                        },
                    )?;
                }
                _ => {
                    self.codegen_immediate_resume_frame_tail_and_continue(
                        exec_plan, last_depth, start_idx,
                    )?;
                }
            }
        }
        self.pop_raise_target();

        self.env.pop_scope();

        self.builder.position_at_end(finally_unwind_bb);
        let rt_pop = self.declare_runtime_effect_handler_stack_pop();
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let frame_i8 = self.builder.build_bit_cast(
            handler_frame_ptr,
            i8_ptr_ty,
            "handle_resume_effect_frame_i8",
        )?;
        let _ = self
            .builder
            .build_call(rt_pop, &[frame_i8.into()], "handle_resume_effect_pop")?;
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
                            kind: "handle resume finally unwind needs function return type",
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
                let loaded = self.builder.build_load(llvm_ty, ptr, "handle_result")?;
                CgValue {
                    ty: out_ty,
                    value: Some(loaded),
                }
            }
        })
    }

    pub(super) fn codegen_immediate_resume_call(
        &mut self,
        span: crate::span::Span,
        args: &[hir::CallArg],
        expected: Option<CgTy>,
        ctx: ImmediateResumeCtx<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "resume() arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(value_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "resume() named arg",
                at: span.into(),
            });
        };

        let value = self.codegen_expr_in_expected_context(value_expr, Some(ctx.resume_value_ty))?;
        let value = self.coerce_value(value_expr.span, value, ctx.resume_value_ty)?;

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

        let ok_bb = self.context.append_basic_block(func, "resume_ok");
        let err_bb = self.context.append_basic_block(func, "resume_twice");
        let cont_bb = self.context.append_basic_block(func, "resume_cont");

        let used = self
            .builder
            .build_load(self.context.bool_type(), ctx.resume_used_ptr, "resume_used")?
            .into_int_value();
        self.builder.build_conditional_branch(used, err_bb, ok_bb)?;

        self.builder.position_at_end(err_bb);
        self.emit_exit_with_code(span, 3)?;

        self.builder.position_at_end(ok_bb);
        let _ = self.builder.build_store(
            ctx.resume_used_ptr,
            self.context.bool_type().const_int(1, false),
        )?;

        if let Some(ptr) = ctx.resume_value_ptr {
            let Some(raw) = value.value else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "resume(value) arg value",
                    at: value_expr.span.into(),
                });
            };
            let _ = self.builder.build_store(ptr, raw)?;
        }

        let _ = self.builder.build_store(
            ctx.state_ptr,
            self.context
                .i32_type()
                .const_int(ctx.next_state as u64, false),
        )?;

        self.builder.build_unconditional_branch(cont_bb)?;

        self.builder.position_at_end(cont_bb);

        Ok(match expected {
            Some(ty) => self.default_value(span, ty)?,
            None => CgValue::unit(),
        })
    }
}
