//!  HandlePlanBuilder: walks HIR handle expressions and emits the state machine.

#![allow(dead_code)]

use super::*;

pub(crate) struct HandlePlanBuilder<'a, 'hir> {
    pub(crate) types: &'a TypeStore,
    pub(crate) handle: &'hir hir::HandleExpr,
    pub(crate) context: &'a HandlePlanContext,
    pub(crate) known_local_fun_effects: HashMap<hir::SymbolId, bool>,
    pub(crate) next_state_id: PlanStateId,
    pub(crate) next_site_id: SuspendSiteId,
    pub(crate) next_cleanup_id: CleanupScopeId,
    pub(crate) states: Vec<PlanState>,
    pub(crate) suspend_sites: Vec<SuspendSitePlan>,
    pub(crate) arm_plans: Vec<ArmPlan>,
    pub(crate) cleanup_scopes: Vec<CleanupScopePlan>,
    pub(crate) frame_slots: HashMap<hir::SymbolId, FrameSlot>,
    pub(crate) resume_source_exprs: HashMap<SuspendSiteId, hir::Expr>,
    pub(crate) nested_handles: Vec<HandleStateMachinePlan>,
}

impl<'a, 'hir> HandlePlanBuilder<'a, 'hir> {
    pub(crate) fn snapshot_synthetic_symbol_seed<T>(&self, f: impl FnOnce() -> T) -> T {
        let saved_seed = self.context.synthetic_symbol_seed();
        let result = f();
        self.context.restore_synthetic_symbol_seed(saved_seed);
        result
    }

    pub(crate) fn nested_handle_may_suspend_outward(&self, handle: &hir::HandleExpr) -> bool {
        self.snapshot_synthetic_symbol_seed(|| {
            HandleStateMachinePlan::build_with_context(self.types, handle, self.context)
                .may_suspend_outward()
        })
    }

    pub(crate) fn arm_body_may_suspend_outward(&self, arm: &hir::HandleArm) -> bool {
        match arm.kind {
            hir::HandleArmKind::NonResuming => self.expr_contains_suspend_subtree(&arm.body),
            hir::HandleArmKind::EscapeContinuation { continuation } => {
                if self.tail_resume_arm_matches(&arm.body, continuation) {
                    self.tail_resume_arm_may_suspend_outward(&arm.body, continuation)
                } else {
                    self.expr_contains_suspend_subtree(&arm.body)
                }
            }
        }
    }

    pub(crate) fn tail_resume_arm_matches(
        &self,
        expr: &hir::Expr,
        continuation_symbol: hir::SymbolId,
    ) -> bool {
        tail_resume_arm_matches_static(expr, continuation_symbol)
    }

    pub(crate) fn tail_resume_stmt_matches(
        &self,
        stmt: &hir::Stmt,
        continuation_symbol: hir::SymbolId,
    ) -> bool {
        matches!(&stmt.kind, hir::StmtKind::Expr(expr) if tail_resume_arm_matches_static(expr, continuation_symbol))
    }

    pub(crate) fn tail_resume_arm_may_suspend_outward(
        &self,
        expr: &hir::Expr,
        continuation_symbol: hir::SymbolId,
    ) -> bool {
        if let Some(payload) = extract_tail_resume_payload_expr(expr, continuation_symbol) {
            return self.expr_contains_suspend_subtree(payload);
        }

        match &expr.kind {
            hir::ExprKind::Block(block) => {
                let Some((tail_stmt, prefix_stmts)) = block.stmts.split_last() else {
                    return true;
                };
                prefix_stmts
                    .iter()
                    .any(|stmt| self.stmt_contains_suspend_subtree(stmt))
                    || self.tail_resume_stmt_may_suspend_outward(tail_stmt, continuation_symbol)
            }
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.expr_contains_suspend_subtree(cond)
                    || self.tail_resume_arm_may_suspend_outward(then_branch, continuation_symbol)
                    || else_branch.as_deref().is_none_or(|expr| {
                        self.tail_resume_arm_may_suspend_outward(expr, continuation_symbol)
                    })
            }
            hir::ExprKind::When { subject, arms } => {
                self.expr_contains_suspend_subtree(subject)
                    || arms.iter().any(|arm| {
                        arm.guard
                            .as_ref()
                            .is_some_and(|guard| self.expr_contains_suspend_subtree(guard))
                            || self
                                .tail_resume_arm_may_suspend_outward(&arm.body, continuation_symbol)
                    })
            }
            _ => true,
        }
    }

    pub(crate) fn tail_resume_stmt_may_suspend_outward(
        &self,
        stmt: &hir::Stmt,
        continuation_symbol: hir::SymbolId,
    ) -> bool {
        let hir::StmtKind::Expr(expr) = &stmt.kind else {
            return true;
        };
        self.tail_resume_arm_may_suspend_outward(expr, continuation_symbol)
    }

    pub(crate) fn local_function_value_may_suspend_when_called(&self, expr: &hir::Expr) -> bool {
        SuspendCallAnalysis {
            types: self.types,
            context: self.context,
        }
        .function_value_may_suspend_when_called(expr, &self.known_local_fun_effects)
    }

    pub(crate) fn record_local_fun_binding_if_needed(&mut self, decl: &hir::ValDecl) {
        let Some(id) = decl.id else {
            return;
        };
        if !hir_ty_is_function_value(self.types, decl.ty) {
            return;
        }
        let may_suspend = decl.init.as_ref().map_or_else(
            || function_ty_declared_effectful(self.types, decl.ty),
            |expr| self.local_function_value_may_suspend_when_called(expr),
        );
        self.known_local_fun_effects.insert(id, may_suspend);
    }

    pub(crate) fn record_local_fun_assignment_if_needed(
        &mut self,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) {
        let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &lhs.kind else {
            return;
        };
        if !hir_ty_is_function_value(self.types, lhs.ty)
            && !hir_ty_is_function_value(self.types, rhs.ty)
            && !self.known_local_fun_effects.contains_key(id)
        {
            return;
        }
        let may_suspend = self.local_function_value_may_suspend_when_called(rhs);
        let entry = self.known_local_fun_effects.entry(*id).or_insert(false);
        *entry |= may_suspend;
    }

    pub(crate) fn new(
        types: &'a TypeStore,
        handle: &'hir hir::HandleExpr,
        context: &'a HandlePlanContext,
    ) -> Self {
        context.reserve_synthetic_symbol_floor(next_synthetic_symbol_seed(
            handle,
            &context.known_local_metadata,
        ));
        Self {
            types,
            handle,
            context,
            known_local_fun_effects: context.known_local_fun_effects.clone(),
            next_state_id: 0,
            next_site_id: 0,
            next_cleanup_id: 0,
            states: Vec::new(),
            suspend_sites: Vec::new(),
            arm_plans: Vec::new(),
            cleanup_scopes: Vec::new(),
            frame_slots: HashMap::new(),
            resume_source_exprs: HashMap::new(),
            nested_handles: Vec::new(),
        }
    }

    pub(crate) fn build(mut self) -> HandleStateMachinePlan {
        let outer_slots =
            collect_outer_scope_slots(self.handle, &self.context.known_local_metadata);
        let mut env = ScopeEnv::with_outer(outer_slots.clone());
        for slot in &outer_slots {
            self.frame_slots.insert(slot.id, slot.clone());
        }

        let entry_state = self.new_state("body.entry");
        let exit_state = self.new_state("body.exit");
        let body_end_state = self.build_block(&self.handle.body, entry_state, &mut env);

        let final_exit_state = if let Some(finally_block) = &self.handle.finally {
            let cleanup_entry = self.new_state("cleanup.finally.entry");
            let cleanup_exit = self.new_state("cleanup.finally.exit");
            let cleanup_scope_id = self.next_cleanup_id;
            self.next_cleanup_id = self.next_cleanup_id.saturating_add(1);
            self.cleanup_scopes.push(CleanupScopePlan {
                id: cleanup_scope_id,
                kind: CleanupScopeKind::Finally,
                entry_state: cleanup_entry,
                exit_state: cleanup_exit,
                note: "normal/raise edges converge through a shared finally scope".to_string(),
            });

            self.set_terminator(
                body_end_state,
                StateTerminator::CleanupEnter {
                    scope_id: cleanup_scope_id,
                    next_state: cleanup_entry,
                },
            );

            let mut cleanup_env = ScopeEnv::with_outer(outer_slots);
            let cleanup_end = self.build_block(finally_block, cleanup_entry, &mut cleanup_env);
            self.state_mut(cleanup_end)
                .actions
                .push(HandleStateOp::CleanupEdgeComplete);
            self.set_terminator(cleanup_end, StateTerminator::Goto(cleanup_exit));
            self.set_terminator(cleanup_exit, StateTerminator::Goto(exit_state));
            cleanup_exit
        } else {
            self.set_terminator(body_end_state, StateTerminator::Goto(exit_state));
            exit_state
        };

        self.state_mut(exit_state)
            .actions
            .push(HandleStateOp::ReturnToEnclosingExpression);
        self.set_terminator(exit_state, StateTerminator::ReturnHandle);

        let dispatch_plan = self.build_dispatch_plan();
        self.build_arm_states();
        self.compute_capture_sets();
        self.attach_suspend_source_paths();
        self.attach_suspend_resume_paths();
        self.materialize_resume_fragments();
        self.attach_escape_resume_targets();
        self.compute_capture_sets();
        let frame_layout = self.build_frame_layout();

        let _ = final_exit_state;

        HandleStateMachinePlan {
            handle_span: self.handle.body.span,
            result_ty: self.handle.body.ty,
            entry_state,
            states: self.states,
            suspend_sites: self.suspend_sites,
            arm_plans: self.arm_plans,
            cleanup_scopes: self.cleanup_scopes,
            frame_layout,
            dispatch_plan,
            nested_handles: self.nested_handles,
        }
    }

    pub(crate) fn resume_slot_for_site(&self, site_id: SuspendSiteId) -> Option<FrameSlot> {
        self.states.iter().find_map(|state| {
            state.actions.iter().find_map(|op| match op {
                HandleStateOp::ResumeAfterSite {
                    site_id: resume_site_id,
                    resume_slot: Some(slot),
                    ..
                } if *resume_site_id == site_id => Some(slot.clone()),
                _ => None,
            })
        })
    }

    pub(crate) fn build_block(
        &mut self,
        block: &'hir hir::Block,
        start_state: PlanStateId,
        env: &mut ScopeEnv,
    ) -> PlanStateId {
        let mut state = start_state;
        let saved_len = env.slots.len();
        for stmt in &block.stmts {
            state = self.build_stmt(stmt, state, env);
        }
        env.slots.truncate(saved_len);
        state
    }

    pub(crate) fn build_stmt(
        &mut self,
        stmt: &'hir hir::Stmt,
        current_state: PlanStateId,
        env: &mut ScopeEnv,
    ) -> PlanStateId {
        match &stmt.kind {
            hir::StmtKind::Empty => {
                self.push_action(
                    current_state,
                    HandleStateOp::StmtEmpty {
                        stmt: Box::new(stmt.clone()),
                    },
                );
                current_state
            }
            hir::StmtKind::Expr(expr) => self.build_expr(expr, current_state, env),
            hir::StmtKind::Val(decl) => {
                let init_from_last_value = self.decl_init_uses_prior_actions(decl.init.as_ref());
                let mut state = current_state;
                if let Some(init) = decl.init.as_ref() {
                    state = self.build_expr_for_consumer(init, state, env);
                }
                self.record_local_fun_binding_if_needed(decl);
                if let Some(id) = self.install_decl_slot(decl, env) {
                    self.push_action(
                        state,
                        HandleStateOp::BindLocal {
                            id,
                            decl: Box::new(decl.clone()),
                            init_from_last_value,
                        },
                    );
                } else {
                    self.push_action(
                        state,
                        HandleStateOp::DeclareAnonymousVal {
                            decl: Box::new(decl.clone()),
                            init_from_last_value,
                        },
                    );
                }
                state
            }
            hir::StmtKind::Assign { lhs, rhs, .. } => {
                let mut state = self.build_expr_for_consumer(lhs, current_state, env);
                state = self.build_expr_for_consumer(rhs, state, env);
                self.record_local_fun_assignment_if_needed(lhs, rhs);
                self.record_stmt_reads(state, stmt);
                self.push_action(
                    state,
                    HandleStateOp::Assign {
                        stmt: Box::new(stmt.clone()),
                    },
                );
                state
            }
            hir::StmtKind::While { cond, body } => {
                self.build_while(stmt, cond, body, current_state, env)
            }
            hir::StmtKind::Break { .. } => {
                self.push_action(
                    current_state,
                    HandleStateOp::Break {
                        stmt: Box::new(stmt.clone()),
                    },
                );
                self.set_terminator(current_state, StateTerminator::ReturnFromFunction);
                self.new_state("unreachable.after.break")
            }
            hir::StmtKind::Continue { .. } => {
                self.push_action(
                    current_state,
                    HandleStateOp::Continue {
                        stmt: Box::new(stmt.clone()),
                    },
                );
                self.set_terminator(current_state, StateTerminator::ReturnFromFunction);
                self.new_state("unreachable.after.continue")
            }
            hir::StmtKind::Return { value } => {
                if let Some(expr) = value {
                    let state = self.build_expr_for_consumer(expr, current_state, env);
                    self.push_action(
                        state,
                        HandleStateOp::Return {
                            stmt: Box::new(stmt.clone()),
                        },
                    );
                    self.set_terminator(state, StateTerminator::ReturnFromFunction);
                    self.new_state("unreachable.after.return")
                } else {
                    self.push_action(
                        current_state,
                        HandleStateOp::Return {
                            stmt: Box::new(stmt.clone()),
                        },
                    );
                    self.set_terminator(current_state, StateTerminator::ReturnFromFunction);
                    self.new_state("unreachable.after.return")
                }
            }
            hir::StmtKind::Todo(kind) => {
                self.push_action(
                    current_state,
                    HandleStateOp::TodoStmt {
                        stmt: Box::new(stmt.clone()),
                        kind: kind.to_string(),
                    },
                );
                current_state
            }
        }
    }

    pub(crate) fn decl_init_uses_prior_actions(&self, init: Option<&hir::Expr>) -> bool {
        init.is_some_and(|expr| self.expr_contains_suspend_subtree(expr))
    }

    pub(crate) fn install_decl_slot(
        &mut self,
        decl: &hir::ValDecl,
        env: &mut ScopeEnv,
    ) -> Option<hir::SymbolId> {
        let id = decl.id?;
        let slot = FrameSlot {
            id,
            name: decl
                .name
                .clone()
                .unwrap_or_else(|| format!("local{}", id.as_u32())),
            ty: decl.ty,
            mutable: decl.mutable,
            seed_from_outer_scope: false,
            owner_arm: None,
        };
        // Declarations are the authoritative source of slot metadata. If an
        // earlier fallback path pre-seeded this symbol as immutable /
        // outer-scope, overwrite it here.
        self.frame_slots.insert(id, slot.clone());
        env.push(slot);
        Some(id)
    }

    pub(crate) fn build_while(
        &mut self,
        stmt: &'hir hir::Stmt,
        cond: &'hir hir::Expr,
        body: &'hir hir::Block,
        current_state: PlanStateId,
        env: &mut ScopeEnv,
    ) -> PlanStateId {
        let cond_state = self.new_state("while.cond");
        self.push_action(
            cond_state,
            HandleStateOp::WhileCondHeader {
                stmt: Box::new(stmt.clone()),
            },
        );
        self.set_terminator(current_state, StateTerminator::Goto(cond_state));

        let cond_eval_state = self.build_expr_for_consumer(cond, cond_state, env);
        let body_state = self.new_state("while.body");
        let exit_state = self.new_state("while.exit");
        self.record_expr_reads(cond_eval_state, cond);
        self.set_terminator(
            cond_eval_state,
            StateTerminator::Branch {
                condition: HandleBranchCondition::WhileCond {
                    condition: Box::new(cond.clone()),
                },
                then_state: body_state,
                else_state: exit_state,
                merge_state: exit_state,
            },
        );

        let mut body_env = env.clone();
        let body_end = self.build_block(body, body_state, &mut body_env);
        self.push_action(body_end, HandleStateOp::LoopReentry { cond_state });
        self.set_terminator(body_end, StateTerminator::Goto(cond_state));
        exit_state
    }

    pub(crate) fn build_expr(
        &mut self,
        expr: &'hir hir::Expr,
        current_state: PlanStateId,
        env: &mut ScopeEnv,
    ) -> PlanStateId {
        match &expr.kind {
            hir::ExprKind::Missing => {
                self.push_action(
                    current_state,
                    HandleStateOp::ExprMissing {
                        expr: Box::new(expr.clone()),
                    },
                );
                current_state
            }
            hir::ExprKind::Literal(_) => {
                self.push_action(
                    current_state,
                    HandleStateOp::Literal {
                        expr: Box::new(expr.clone()),
                    },
                );
                current_state
            }
            hir::ExprKind::ClassLiteral(_) => {
                self.push_action(
                    current_state,
                    HandleStateOp::Literal {
                        expr: Box::new(expr.clone()),
                    },
                );
                current_state
            }
            hir::ExprKind::VarRef(hir::ValueRef::Local { id, name, .. }) => {
                let slot = self.authoritative_local_slot(*id, name, expr.ty);
                self.frame_slots.entry(*id).or_insert(slot);
                self.push_action(
                    current_state,
                    HandleStateOp::ReadLocal {
                        id: *id,
                        expr: Box::new(expr.clone()),
                    },
                );
                self.record_expr_reads(current_state, expr);
                current_state
            }
            hir::ExprKind::VarRef(value_ref) => {
                if let Some(kind) = self.classify_hidden_suspend_var_ref(value_ref) {
                    self.record_expr_reads(current_state, expr);
                    let site_id =
                        self.new_suspend_site(expr.span, kind, env.available_ids(), current_state);
                    self.push_action(
                        current_state,
                        HandleStateOp::ObjectInitAccessBoundary {
                            site_id,
                            expr: Box::new(expr.clone()),
                        },
                    );
                    self.set_terminator(current_state, StateTerminator::Suspend { site_id });
                    let resume_state = self.new_state(format!("resume.after.site{site_id}"));
                    self.record_resume_source_expr(site_id, expr);
                    self.push_action(
                        resume_state,
                        HandleStateOp::ResumeAfterSite {
                            site_id,
                            reason: ResumeAfterSiteReason::ObjectInitAccess,
                            source_span: expr.span,
                            source_ty: expr.ty,
                            resume_slot: None,
                        },
                    );
                    self.set_suspend_resume_target(site_id, resume_state);
                    return resume_state;
                }
                self.push_action(
                    current_state,
                    HandleStateOp::VarRef {
                        expr: Box::new(expr.clone()),
                    },
                );
                current_state
            }
            hir::ExprKind::UnresolvedIdent { .. } => {
                self.push_action(
                    current_state,
                    HandleStateOp::VarRef {
                        expr: Box::new(expr.clone()),
                    },
                );
                current_state
            }
            hir::ExprKind::StructLit { fields, .. } => {
                let mut state = current_state;
                for field in fields {
                    state = self.build_expr_if_suspend_subtree(&field.value, state, env);
                }
                self.push_action(
                    state,
                    HandleStateOp::StructLit {
                        expr: Box::new(expr.clone()),
                    },
                );
                state
            }
            hir::ExprKind::TupleLit { elements } => {
                let mut state = current_state;
                for element in elements {
                    state = self.build_expr_if_suspend_subtree(element, state, env);
                }
                self.push_action(
                    state,
                    HandleStateOp::TupleLit {
                        expr: Box::new(expr.clone()),
                    },
                );
                state
            }
            hir::ExprKind::InterpolatedString { parts, .. } => {
                let mut state = current_state;
                for part in parts {
                    if let hir::InterpolatedStringPart::Expr { expr } = part {
                        state = self.build_expr_if_suspend_subtree(expr, state, env);
                    }
                }
                self.push_action(
                    state,
                    HandleStateOp::InterpolatedString {
                        expr: Box::new(expr.clone()),
                    },
                );
                state
            }
            hir::ExprKind::Unary { expr: inner, .. }
            | hir::ExprKind::TypeCheck { expr: inner, .. } => {
                let state = self.build_expr_if_suspend_subtree(inner, current_state, env);
                self.record_expr_reads(state, expr);
                self.push_action(
                    state,
                    HandleStateOp::Expr {
                        expr: Box::new(expr.clone()),
                    },
                );
                state
            }
            hir::ExprKind::Cast {
                expr: inner, op, ..
            } => {
                let state = self.build_expr_if_suspend_subtree(inner, current_state, env);
                if matches!(op, hir::CastOp::As) {
                    self.record_expr_reads(state, expr);
                    let site_id = self.new_suspend_site(
                        expr.span,
                        SuspendSiteKind::RuntimeRaise {
                            reason: "ClassCastFailed".to_string(),
                        },
                        env.available_ids(),
                        state,
                    );
                    self.push_action(
                        state,
                        HandleStateOp::RuntimeRaiseBoundary {
                            site_id,
                            expr: Box::new(expr.clone()),
                        },
                    );
                    self.set_terminator(state, StateTerminator::Suspend { site_id });
                    let resume_state = self.new_state(format!("resume.after.site{site_id}"));
                    self.record_resume_source_expr(site_id, expr);
                    self.push_action(
                        resume_state,
                        HandleStateOp::ResumeAfterSite {
                            site_id,
                            reason: ResumeAfterSiteReason::RuntimeRaiseBoundary,
                            source_span: expr.span,
                            source_ty: expr.ty,
                            resume_slot: None,
                        },
                    );
                    self.set_suspend_resume_target(site_id, resume_state);
                    return resume_state;
                }
                self.record_expr_reads(state, expr);
                self.push_action(
                    state,
                    HandleStateOp::Expr {
                        expr: Box::new(expr.clone()),
                    },
                );
                state
            }
            hir::ExprKind::MemberAccess { receiver, member } => {
                let state = self.build_expr_if_suspend_subtree(receiver, current_state, env);
                if let Some(kind) = self.classify_hidden_suspend_member_access(member) {
                    self.record_expr_reads(state, expr);
                    let site_id =
                        self.new_suspend_site(expr.span, kind, env.available_ids(), state);
                    self.push_action(
                        state,
                        HandleStateOp::ObjectInitAccessBoundary {
                            site_id,
                            expr: Box::new(expr.clone()),
                        },
                    );
                    self.set_terminator(state, StateTerminator::Suspend { site_id });
                    let resume_state = self.new_state(format!("resume.after.site{site_id}"));
                    self.record_resume_source_expr(site_id, expr);
                    self.push_action(
                        resume_state,
                        HandleStateOp::ResumeAfterSite {
                            site_id,
                            reason: ResumeAfterSiteReason::ObjectInitAccess,
                            source_span: expr.span,
                            source_ty: expr.ty,
                            resume_slot: None,
                        },
                    );
                    self.set_suspend_resume_target(site_id, resume_state);
                    return resume_state;
                }
                self.record_expr_reads(state, expr);
                self.push_action(
                    state,
                    HandleStateOp::Expr {
                        expr: Box::new(expr.clone()),
                    },
                );
                state
            }
            hir::ExprKind::Binary { lhs, rhs, .. } => {
                let state = self.build_expr_if_suspend_subtree(lhs, current_state, env);
                let state = self.build_expr_if_suspend_subtree(rhs, state, env);
                self.record_expr_reads(state, expr);
                self.push_action(
                    state,
                    HandleStateOp::BinaryExpr {
                        expr: Box::new(expr.clone()),
                    },
                );
                state
            }
            hir::ExprKind::Block(block) => self.build_block(block, current_state, env),
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let cond_state = self.build_expr_for_consumer(cond, current_state, env);
                let then_state = self.new_state("if.then");
                let else_state = self.new_state("if.else");
                let merge_state = self.new_state("if.merge");
                self.record_expr_reads(cond_state, cond);
                self.set_terminator(
                    cond_state,
                    StateTerminator::Branch {
                        condition: HandleBranchCondition::IfCond {
                            condition: Box::new(cond.as_ref().clone()),
                        },
                        then_state,
                        else_state,
                        merge_state,
                    },
                );

                let mut then_env = env.clone();
                let then_end = self.build_expr(then_branch, then_state, &mut then_env);
                self.set_terminator(then_end, StateTerminator::Goto(merge_state));

                let mut else_env = env.clone();
                if let Some(else_branch) = else_branch.as_deref() {
                    let else_end = self.build_expr(else_branch, else_state, &mut else_env);
                    self.set_terminator(else_end, StateTerminator::Goto(merge_state));
                } else {
                    self.push_action(
                        else_state,
                        HandleStateOp::ImplicitElseUnit { span: expr.span },
                    );
                    self.set_terminator(else_state, StateTerminator::Goto(merge_state));
                }
                merge_state
            }
            hir::ExprKind::When { subject, arms } => {
                let mut state = self.build_expr_if_suspend_subtree(subject, current_state, env);
                for arm in arms {
                    if let Some(guard) = arm.guard.as_ref() {
                        state = self.build_expr_if_suspend_subtree(guard, state, env);
                    }
                    state = self.build_expr_if_suspend_subtree(&arm.body, state, env);
                }
                self.push_action(
                    state,
                    HandleStateOp::WhenExpr {
                        expr: Box::new(expr.clone()),
                    },
                );
                state
            }
            hir::ExprKind::Call { callee, args } => {
                let mut state = self.build_expr_if_suspend_subtree(callee, current_state, env);
                for arg in args {
                    state = match arg {
                        hir::CallArg::Positional(expr) => {
                            self.build_expr_if_suspend_subtree(expr, state, env)
                        }
                        hir::CallArg::Named { value, .. } => {
                            self.build_expr_if_suspend_subtree(value, state, env)
                        }
                    };
                }
                if let Some(kind) = self.classify_suspend_call(expr, callee) {
                    self.record_expr_reads(state, expr);
                    let site_id =
                        self.new_suspend_site(expr.span, kind, env.available_ids(), state);
                    self.push_action(
                        state,
                        HandleStateOp::SuspendCall {
                            site_id,
                            expr: Box::new(expr.clone()),
                        },
                    );
                    self.set_terminator(state, StateTerminator::Suspend { site_id });
                    let resume_state = self.new_state(format!("resume.after.site{site_id}"));
                    let resume_slot = self.new_resume_temp_slot(site_id, expr);
                    self.record_resume_source_expr(site_id, expr);
                    self.push_action(
                        resume_state,
                        HandleStateOp::ResumeAfterSite {
                            site_id,
                            reason: ResumeAfterSiteReason::Call,
                            source_span: expr.span,
                            source_ty: expr.ty,
                            resume_slot: Some(resume_slot),
                        },
                    );
                    self.set_suspend_resume_target(site_id, resume_state);
                    return resume_state;
                }
                self.record_expr_reads(state, expr);
                self.push_action(
                    state,
                    HandleStateOp::Call {
                        expr: Box::new(expr.clone()),
                    },
                );
                state
            }
            hir::ExprKind::Perform { op, args, .. } => {
                let mut state = current_state;
                for arg in args {
                    state = match arg {
                        hir::CallArg::Positional(expr) => {
                            self.build_expr_if_suspend_subtree(expr, state, env)
                        }
                        hir::CallArg::Named { value, .. } => {
                            self.build_expr_if_suspend_subtree(value, state, env)
                        }
                    };
                }
                self.record_expr_reads(state, expr);
                let site_id = self.new_suspend_site(
                    expr.span,
                    SuspendSiteKind::Perform {
                        op_fqn: op.fqn.clone(),
                    },
                    env.available_ids(),
                    state,
                );
                self.push_action(
                    state,
                    HandleStateOp::Perform {
                        op_fqn: op.fqn.clone(),
                        expr: Box::new(expr.clone()),
                    },
                );
                self.set_terminator(state, StateTerminator::Suspend { site_id });
                let resume_state = self.new_state(format!("resume.after.site{site_id}"));
                let resume_slot = self.new_resume_temp_slot(site_id, expr);
                self.record_resume_source_expr(site_id, expr);
                self.push_action(
                    resume_state,
                    HandleStateOp::ResumeAfterSite {
                        site_id,
                        reason: ResumeAfterSiteReason::Perform,
                        source_span: expr.span,
                        source_ty: expr.ty,
                        resume_slot: Some(resume_slot),
                    },
                );
                self.set_suspend_resume_target(site_id, resume_state);
                resume_state
            }
            hir::ExprKind::Handle(handle) => {
                let nested_id = self.nested_handles.len();
                let nested =
                    HandleStateMachinePlan::build_with_context(self.types, handle, self.context);
                let nested_may_suspend = nested.may_suspend_outward();
                self.nested_handles.push(nested);
                if nested_may_suspend {
                    self.record_expr_reads(current_state, expr);
                    let site_id = self.new_suspend_site(
                        expr.span,
                        SuspendSiteKind::NestedHandleBoundary {
                            detail: format!("nested#{nested_id}"),
                        },
                        env.available_ids(),
                        current_state,
                    );
                    self.push_action(
                        current_state,
                        HandleStateOp::NestedHandleBoundary {
                            site_id,
                            nested_id,
                            expr: Box::new(expr.clone()),
                        },
                    );
                    self.set_terminator(current_state, StateTerminator::Suspend { site_id });
                    let resume_state = self.new_state(format!("resume.after.site{site_id}"));
                    let resume_slot = self.new_resume_temp_slot(site_id, expr);
                    self.record_resume_source_expr(site_id, expr);
                    self.push_action(
                        resume_state,
                        HandleStateOp::ResumeAfterSite {
                            site_id,
                            reason: ResumeAfterSiteReason::NestedHandleBoundary,
                            source_span: expr.span,
                            source_ty: expr.ty,
                            resume_slot: Some(resume_slot),
                        },
                    );
                    self.set_suspend_resume_target(site_id, resume_state);
                    return resume_state;
                }
                self.push_action(
                    current_state,
                    HandleStateOp::NestedHandle {
                        nested_id,
                        expr: Box::new(expr.clone()),
                    },
                );
                current_state
            }
            hir::ExprKind::Closure(closure) => {
                self.push_action(
                    current_state,
                    HandleStateOp::Closure {
                        expr: Box::new(expr.clone()),
                    },
                );
                self.record_expr_reads(current_state, &closure.body);
                current_state
            }
            hir::ExprKind::Todo(kind) => {
                self.push_action(
                    current_state,
                    HandleStateOp::TodoExpr {
                        expr: Box::new(expr.clone()),
                        kind: kind.to_string(),
                    },
                );
                current_state
            }
        }
    }

    pub(crate) fn build_expr_for_consumer(
        &mut self,
        expr: &'hir hir::Expr,
        current_state: PlanStateId,
        env: &mut ScopeEnv,
    ) -> PlanStateId {
        if self.expr_contains_suspend_subtree(expr) {
            self.build_expr(expr, current_state, env)
        } else {
            current_state
        }
    }

    pub(crate) fn build_expr_if_suspend_subtree(
        &mut self,
        expr: &'hir hir::Expr,
        current_state: PlanStateId,
        env: &mut ScopeEnv,
    ) -> PlanStateId {
        if self.expr_contains_suspend_subtree(expr) {
            self.build_expr(expr, current_state, env)
        } else {
            current_state
        }
    }

    pub(crate) fn expr_contains_suspend_subtree(&self, expr: &hir::Expr) -> bool {
        match &expr.kind {
            hir::ExprKind::Missing
            | hir::ExprKind::Literal(_)
            | hir::ExprKind::UnresolvedIdent { .. }
            | hir::ExprKind::ClassLiteral(_)
            | hir::ExprKind::Closure(_)
            | hir::ExprKind::Todo(_) => false,
            hir::ExprKind::VarRef(value_ref) => {
                self.classify_hidden_suspend_var_ref(value_ref).is_some()
            }
            hir::ExprKind::StructLit { fields, .. } => fields
                .iter()
                .any(|field| self.expr_contains_suspend_subtree(&field.value)),
            hir::ExprKind::TupleLit { elements } => elements
                .iter()
                .any(|element| self.expr_contains_suspend_subtree(element)),
            hir::ExprKind::InterpolatedString { parts, .. } => parts.iter().any(|part| {
                matches!(
                    part,
                    hir::InterpolatedStringPart::Expr { expr }
                        if self.expr_contains_suspend_subtree(expr)
                )
            }),
            hir::ExprKind::Unary { expr: inner, .. }
            | hir::ExprKind::TypeCheck { expr: inner, .. } => {
                self.expr_contains_suspend_subtree(inner)
            }
            hir::ExprKind::Cast {
                expr: inner, op, ..
            } => matches!(op, hir::CastOp::As) || self.expr_contains_suspend_subtree(inner),
            hir::ExprKind::MemberAccess { receiver, member } => {
                self.expr_contains_suspend_subtree(receiver)
                    || self.classify_hidden_suspend_member_access(member).is_some()
            }
            hir::ExprKind::Binary { lhs, rhs, .. } => {
                self.expr_contains_suspend_subtree(lhs) || self.expr_contains_suspend_subtree(rhs)
            }
            hir::ExprKind::Block(block) => self.block_contains_suspend_subtree(block),
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.expr_contains_suspend_subtree(cond)
                    || self.expr_contains_suspend_subtree(then_branch)
                    || else_branch
                        .as_deref()
                        .is_some_and(|expr| self.expr_contains_suspend_subtree(expr))
            }
            hir::ExprKind::When { subject, arms } => {
                self.expr_contains_suspend_subtree(subject)
                    || arms.iter().any(|arm| {
                        arm.guard
                            .as_ref()
                            .is_some_and(|guard| self.expr_contains_suspend_subtree(guard))
                            || self.expr_contains_suspend_subtree(&arm.body)
                    })
            }
            hir::ExprKind::Call { callee, args } => {
                self.classify_suspend_call(expr, callee).is_some()
                    || self.expr_contains_suspend_subtree(callee)
                    || args.iter().any(|arg| match arg {
                        hir::CallArg::Positional(expr) => self.expr_contains_suspend_subtree(expr),
                        hir::CallArg::Named { value, .. } => {
                            self.expr_contains_suspend_subtree(value)
                        }
                    })
            }
            hir::ExprKind::Perform { .. } => true,
            hir::ExprKind::Handle(handle) => self.nested_handle_may_suspend_outward(handle),
        }
    }

    pub(crate) fn handle_contains_suspend_subtree(&self, handle: &hir::HandleExpr) -> bool {
        self.block_contains_suspend_subtree(&handle.body)
            || handle
                .arms
                .iter()
                .any(|arm| self.expr_contains_suspend_subtree(&arm.body))
            || handle
                .finally
                .as_ref()
                .is_some_and(|finally_block| self.block_contains_suspend_subtree(finally_block))
    }

    pub(crate) fn block_contains_suspend_subtree(&self, block: &hir::Block) -> bool {
        block
            .stmts
            .iter()
            .any(|stmt| self.stmt_contains_suspend_subtree(stmt))
    }

    pub(crate) fn stmt_contains_suspend_subtree(&self, stmt: &hir::Stmt) -> bool {
        match &stmt.kind {
            hir::StmtKind::Empty
            | hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Todo(_) => false,
            hir::StmtKind::Expr(expr) => self.expr_contains_suspend_subtree(expr),
            hir::StmtKind::Val(decl) => decl
                .init
                .as_ref()
                .is_some_and(|expr| self.expr_contains_suspend_subtree(expr)),
            hir::StmtKind::Assign { lhs, rhs, .. } => {
                self.expr_contains_suspend_subtree(lhs) || self.expr_contains_suspend_subtree(rhs)
            }
            hir::StmtKind::While { cond, body } => {
                self.expr_contains_suspend_subtree(cond)
                    || self.block_contains_suspend_subtree(body)
            }
            hir::StmtKind::Return { value } => value
                .as_ref()
                .is_some_and(|expr| self.expr_contains_suspend_subtree(expr)),
        }
    }

    pub(crate) fn classify_suspend_call(
        &self,
        expr: &hir::Expr,
        callee: &hir::Expr,
    ) -> Option<SuspendSiteKind> {
        if let Some(kind) = self.classify_builtin_suspend_call(expr.span) {
            return Some(kind);
        }

        if let Some(fqn) = try_extract_callee_fqn(callee)
            && let Some(effectful) = self.context.known_fun_effects.get(&fqn).copied()
        {
            return if effectful {
                Some(SuspendSiteKind::CallStateMachineCallee { callee: fqn })
            } else {
                None
            };
        }

        if let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &callee.kind
            && let Some(effectful) = self.known_local_fun_effects.get(id).copied()
        {
            return if effectful {
                Some(SuspendSiteKind::CallMaySuspend {
                    callee: format!("local#{}", id.as_u32()),
                })
            } else {
                None
            };
        }

        if let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &callee.kind
            && let Some(slot) = self.frame_slots.get(id)
            && let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = self.types.kind(slot.ty)
        {
            return if fun_ty.effects.is_pure() {
                None
            } else {
                Some(SuspendSiteKind::CallMaySuspend {
                    callee: format!("local#{}", id.as_u32()),
                })
            };
        }

        if let Some(target) = self
            .context
            .facts
            .constructor_call(self.context.current_source_path(), expr.span)
        {
            let class_name = if target.owner_fqn.is_empty() {
                format!("ctor@{:?}", callee.span)
            } else {
                target.owner_fqn.clone()
            };
            return Some(SuspendSiteKind::ClassCtorInit { class_name });
        }

        let callee_ty = resolve_plan_expr_concrete_type(
            self.context,
            self.types,
            callee,
            &self.context.known_local_metadata,
        )
        .unwrap_or(callee.ty);
        if let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = self.types.kind(callee_ty) {
            if fun_ty.effects.is_pure() {
                return self
                    .local_function_value_may_suspend_when_called(callee)
                    .then(|| SuspendSiteKind::CallMaySuspend {
                        callee: format!("expr@{:?}", expr.span),
                    });
            }
            return try_extract_callee_fqn(callee).map_or_else(
                || {
                    Some(SuspendSiteKind::CallMaySuspend {
                        callee: format!("expr@{:?}", expr.span),
                    })
                },
                |fqn| Some(SuspendSiteKind::CallStateMachineCallee { callee: fqn }),
            );
        }
        None
    }

    pub(crate) fn classify_builtin_suspend_call(&self, call_span: Span) -> Option<SuspendSiteKind> {
        // `Continuation.resume` 的 builtin 语义只来自上游 typecheck 已确认的 side tables；
        // segmentation 本身不再按成员名、receiver 类型或其它代码形状做推断。
        //
        // 只有 receiver continuation 的 effect row 非 Pure 时，resumed body 才会像普通
        // effectful callee 一样再次 suspend outward，outer handle 需要走
        // resume.after.call replay 主线。Pure continuation 则只保留 hidden
        // `Raise<RuntimeError>` 边界，使 `try { k.resume(...) } catch` 继续保持
        // self-contained nested-handle 语义。
        if !self
            .context
            .facts
            .has_continuation_resume(self.context.current_source_path(), call_span)
        {
            return None;
        }

        if self
            .context
            .facts
            .continuation_resume(self.context.current_source_path(), call_span)
            .is_some_and(|resume| resume.resumes_outward())
        {
            Some(SuspendSiteKind::CallMaySuspend {
                callee: "Continuation.resume".to_string(),
            })
        } else {
            Some(SuspendSiteKind::RuntimeRaise {
                reason: "Continuation.resume".to_string(),
            })
        }
    }

    pub(crate) fn classify_hidden_suspend_var_ref(
        &self,
        value_ref: &hir::ValueRef,
    ) -> Option<SuspendSiteKind> {
        let hir::ValueRef::TopLevel { fqn, .. } = value_ref else {
            return None;
        };
        if self.context.facts.is_object_value_fqn(fqn) {
            Some(SuspendSiteKind::ObjectInitAccess {
                target: fqn.clone(),
            })
        } else if self.context.facts.is_top_level_immutable_value_fqn(fqn) {
            Some(SuspendSiteKind::TopLevelValueInitAccess {
                target: fqn.clone(),
            })
        } else {
            None
        }
    }

    pub(crate) fn classify_hidden_suspend_member_access(
        &self,
        member: &hir::MemberAccess,
    ) -> Option<SuspendSiteKind> {
        let hir::MemberRef::Value { fqn, .. } = member.resolved.as_ref()? else {
            return None;
        };
        (self.context.facts.is_object_value_fqn(fqn)
            || self.context.facts.is_object_property_fqn(fqn))
        .then(|| SuspendSiteKind::ObjectInitAccess {
            target: fqn.clone(),
        })
    }

    pub(crate) fn build_dispatch_plan(&self) -> DispatchPlan {
        let mut by_op: HashMap<String, Vec<ArmPlanId>> = HashMap::new();
        for (idx, arm) in self.handle.arms.iter().enumerate() {
            by_op
                .entry(arm.op.op.fqn.clone())
                .or_default()
                .push(idx as u32);
        }
        let mut entries = by_op
            .into_iter()
            .map(|(op_fqn, arm_ids)| DispatchEntry { op_fqn, arm_ids })
            .collect::<Vec<_>>();
        entries.sort_by(|a, b| a.op_fqn.cmp(&b.op_fqn));
        DispatchPlan { entries }
    }

    pub(crate) fn build_arm_states(&mut self) {
        for (idx, arm) in self.handle.arms.iter().enumerate() {
            let arm_id = idx as ArmPlanId;
            let binder_slots = arm
                .op
                .binders
                .iter()
                .map(|binder| FrameSlot {
                    id: binder.id,
                    name: binder.name.clone(),
                    ty: binder.ty,
                    mutable: false,
                    seed_from_outer_scope: false,
                    owner_arm: Some(arm_id),
                })
                .collect::<Vec<_>>();
            for slot in &binder_slots {
                self.frame_slots.insert(slot.id, slot.clone());
            }

            let mut declared = binder_slots
                .iter()
                .map(|slot| slot.id)
                .collect::<HashSet<_>>();
            match arm.kind {
                hir::HandleArmKind::NonResuming => {}
                hir::HandleArmKind::EscapeContinuation { continuation } => {
                    declared.insert(continuation);
                }
            }
            collect_declared_local_ids_in_expr(&arm.body, &mut declared);

            let mut used = HashMap::new();
            collect_local_refs_in_expr(&arm.body, &mut used);
            let continuation_slot = match arm.kind {
                hir::HandleArmKind::EscapeContinuation { continuation } => {
                    used.get(&continuation).cloned().map(|(name, ty)| {
                        if !self.frame_slots.contains_key(&continuation) {
                            let slot = self.authoritative_local_slot(continuation, &name, ty);
                            self.frame_slots.insert(continuation, slot);
                        }
                        self.frame_slots
                            .get(&continuation)
                            .cloned()
                            .expect("escape continuation slot must exist")
                    })
                }
                hir::HandleArmKind::NonResuming => None,
            };
            let mut capture_locals = Vec::new();
            for (id, (name, ty)) in used {
                if declared.contains(&id) {
                    continue;
                }
                if !self.frame_slots.contains_key(&id) {
                    let slot = self.authoritative_local_slot(id, &name, ty);
                    self.frame_slots.insert(id, slot);
                }
                capture_locals.push(id);
            }
            capture_locals.sort_by_key(|id| id.as_u32());

            let body_may_suspend_outward = self.arm_body_may_suspend_outward(arm);
            let segmented_body = matches!(
                arm.kind,
                hir::HandleArmKind::EscapeContinuation { continuation }
                    if !self.tail_resume_arm_matches(&arm.body, continuation)
            ) && body_may_suspend_outward;
            let body_entry_state = self.new_state(format!("arm{arm_id}.body"));
            self.push_action(
                body_entry_state,
                HandleStateOp::ExecuteArmBody {
                    arm_id,
                    op_fqn: arm.op.op.fqn.clone(),
                    arm: Box::new(arm.clone()),
                    segmented_body,
                },
            );

            let arm_exit = match arm.kind {
                hir::HandleArmKind::NonResuming => ArmBodyExit::ReturnHandle,
                hir::HandleArmKind::EscapeContinuation { continuation }
                    if self.tail_resume_arm_matches(&arm.body, continuation) =>
                {
                    ArmBodyExit::ResumeMatchedSite
                }
                hir::HandleArmKind::EscapeContinuation { .. } => {
                    ArmBodyExit::MaterializeContinuation
                }
            };
            let body_end_state = if segmented_body {
                let mut arm_env = ScopeEnv::default();
                for slot in &binder_slots {
                    arm_env.push(slot.clone());
                }
                if let Some(slot) = continuation_slot.clone() {
                    arm_env.push(slot);
                }
                for local_id in &capture_locals {
                    if let Some(slot) = self.frame_slots.get(local_id).cloned() {
                        arm_env.push(slot);
                    }
                }
                self.build_expr(&arm.body, body_entry_state, &mut arm_env)
            } else {
                body_entry_state
            };
            self.set_terminator(body_end_state, StateTerminator::ArmExit(arm_exit));

            self.arm_plans.push(ArmPlan {
                id: arm_id,
                op_fqn: arm.op.op.fqn.clone(),
                effect_ty: arm.op.effect_ty,
                binder_slots,
                capture_locals,
                body_entry_state,
                body_may_suspend_outward,
            });
        }
    }

    pub(crate) fn build_frame_layout(&self) -> FrameLayoutPlan {
        let mut lifted_ids = self
            .suspend_sites
            .iter()
            .flat_map(|site| site.capture_locals.iter().copied())
            .collect::<Vec<_>>();
        lifted_ids.extend(
            self.arm_plans
                .iter()
                .flat_map(|arm| arm.capture_locals.iter().copied()),
        );
        lifted_ids.sort_by_key(|id| id.as_u32());
        lifted_ids.dedup_by_key(|id| id.as_u32());

        let mut lifted_locals = lifted_ids
            .into_iter()
            .filter_map(|id| self.frame_slots.get(&id).cloned())
            .collect::<Vec<_>>();
        lifted_locals.sort_by_key(|slot| slot.id.as_u32());

        let mut arm_binders = self
            .arm_plans
            .iter()
            .flat_map(|arm| arm.binder_slots.clone())
            .collect::<Vec<_>>();
        arm_binders.sort_by_key(|slot| (slot.owner_arm.unwrap_or(0), slot.id.as_u32()));

        FrameLayoutPlan {
            slots: self.frame_slots.clone(),
            lifted_locals,
            arm_binders,
            has_cleanup_flag: !self.cleanup_scopes.is_empty(),
            has_one_shot_flag: self.states.iter().any(|state| {
                matches!(
                    state.terminator,
                    StateTerminator::ArmExit(ArmBodyExit::MaterializeContinuation)
                )
            }),
        }
    }

    pub(crate) fn compute_capture_sets(&mut self) {
        let successors = build_successor_map(&self.states);
        let state_reads = self
            .states
            .iter()
            .map(|state| (state.id, state.reads.clone()))
            .collect::<HashMap<_, _>>();
        let suspend_state_reads = self
            .states
            .iter()
            .filter_map(|state| match state.terminator {
                StateTerminator::Suspend { site_id } => Some((site_id, state.reads.clone())),
                _ => None,
            })
            .collect::<HashMap<_, _>>();
        for site in &mut self.suspend_sites {
            let mut reachable = reachable_states(site.resume_target, &successors);
            if let Some(escape_resume_target) = site.escape_resume_target {
                reachable.extend(reachable_states(escape_resume_target, &successors));
            }
            let mut used_after = reachable
                .into_iter()
                .flat_map(|state_id| state_reads.get(&state_id).cloned().unwrap_or_default())
                .collect::<Vec<_>>();
            if matches!(
                site.kind,
                SuspendSiteKind::CallMaySuspend { .. }
                    | SuspendSiteKind::CallStateMachineCallee { .. }
            ) {
                used_after.extend(
                    suspend_state_reads
                        .get(&site.id)
                        .cloned()
                        .unwrap_or_default(),
                );
            }
            used_after.sort_by_key(|id| id.as_u32());
            used_after.dedup_by_key(|id| id.as_u32());

            let used_set = used_after.into_iter().collect::<HashSet<_>>();
            site.capture_locals = site
                .available_locals
                .iter()
                .copied()
                .filter(|id| used_set.contains(id))
                .collect::<Vec<_>>();
            site.capture_locals.sort_by_key(|id| id.as_u32());
            site.matching_arms = matching_arms(&self.arm_plans, &site.kind);
        }
    }

    pub(crate) fn attach_suspend_source_paths(&mut self) {
        let mut path = Vec::new();
        for (stmt_idx, stmt) in self.handle.body.stmts.iter().enumerate() {
            let root = SuspendSourceRoot::HandleBodyStmt {
                stmt_idx,
                stmt_span: stmt.span,
            };
            self.attach_suspend_source_paths_in_stmt(stmt, &root, &mut path);
        }
        for (arm_index, arm) in self.handle.arms.iter().enumerate() {
            let root = SuspendSourceRoot::ArmBody {
                arm_index,
                body_span: arm.body.span,
            };
            self.attach_suspend_source_paths_in_expr(&arm.body, &root, &mut path);
        }
        if let Some(finally_block) = self.handle.finally.as_ref() {
            for (stmt_idx, stmt) in finally_block.stmts.iter().enumerate() {
                let root = SuspendSourceRoot::FinallyStmt {
                    stmt_idx,
                    stmt_span: stmt.span,
                };
                self.attach_suspend_source_paths_in_stmt(stmt, &root, &mut path);
            }
        }
    }

    pub(crate) fn attach_suspend_source_paths_in_stmt(
        &mut self,
        stmt: &'hir hir::Stmt,
        root: &SuspendSourceRoot,
        path: &mut Vec<SuspendSourceFramePath>,
    ) {
        match &stmt.kind {
            hir::StmtKind::Empty
            | hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Todo(_) => {}
            hir::StmtKind::Val(decl) => {
                let Some(init) = decl.init.as_ref() else {
                    return;
                };
                self.attach_suspend_source_paths_in_expr(init, root, path);
            }
            hir::StmtKind::Expr(expr) => {
                self.attach_suspend_source_paths_in_expr(expr, root, path);
            }
            hir::StmtKind::Assign { lhs, rhs, .. } => {
                self.attach_suspend_source_paths_in_expr(lhs, root, path);
                self.attach_suspend_source_paths_in_expr(rhs, root, path);
            }
            hir::StmtKind::Return { value } => {
                if let Some(value) = value.as_ref() {
                    self.attach_suspend_source_paths_in_expr(value, root, path);
                }
            }
            hir::StmtKind::While { cond, body } => {
                self.attach_suspend_source_paths_in_expr(cond, root, path);
                for (stmt_idx, body_stmt) in body.stmts.iter().enumerate() {
                    path.push(SuspendSourceFramePath::WhileBody {
                        while_cond_span: cond.span,
                        while_body_span: body.span,
                        stmt_idx,
                    });
                    self.attach_suspend_source_paths_in_stmt(body_stmt, root, path);
                    let _ = path.pop();
                }
            }
        }
    }

    pub(crate) fn attach_suspend_source_paths_in_expr(
        &mut self,
        expr: &'hir hir::Expr,
        root: &SuspendSourceRoot,
        path: &mut Vec<SuspendSourceFramePath>,
    ) {
        self.record_suspend_source_path(expr, root, path);
        match &expr.kind {
            hir::ExprKind::Missing
            | hir::ExprKind::Literal(_)
            | hir::ExprKind::VarRef(_)
            | hir::ExprKind::UnresolvedIdent { .. }
            | hir::ExprKind::ClassLiteral(_)
            | hir::ExprKind::Closure(_)
            | hir::ExprKind::Todo(_) => {}
            hir::ExprKind::Handle(_) => {}
            hir::ExprKind::StructLit { fields, .. } => {
                for field in fields {
                    self.attach_suspend_source_paths_in_expr(&field.value, root, path);
                }
            }
            hir::ExprKind::TupleLit { elements } => {
                for element in elements {
                    self.attach_suspend_source_paths_in_expr(element, root, path);
                }
            }
            hir::ExprKind::InterpolatedString { parts, .. } => {
                for part in parts {
                    let hir::InterpolatedStringPart::Expr { expr: part_expr } = part else {
                        continue;
                    };
                    self.attach_suspend_source_paths_in_expr(part_expr, root, path);
                }
            }
            hir::ExprKind::Unary { expr: inner, .. }
            | hir::ExprKind::TypeCheck { expr: inner, .. }
            | hir::ExprKind::Cast { expr: inner, .. } => {
                self.attach_suspend_source_paths_in_expr(inner, root, path);
            }
            hir::ExprKind::Binary { lhs, rhs, .. } => {
                self.attach_suspend_source_paths_in_expr(lhs, root, path);
                self.attach_suspend_source_paths_in_expr(rhs, root, path);
            }
            hir::ExprKind::Block(block) => {
                for (stmt_idx, stmt) in block.stmts.iter().enumerate() {
                    path.push(SuspendSourceFramePath::Block {
                        block_span: block.span,
                        stmt_idx,
                    });
                    self.attach_suspend_source_paths_in_stmt(stmt, root, path);
                    let _ = path.pop();
                }
            }
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                self.attach_suspend_source_paths_in_expr(cond, root, path);
                if let hir::ExprKind::Block(block) = &then_branch.kind {
                    for (stmt_idx, stmt) in block.stmts.iter().enumerate() {
                        path.push(SuspendSourceFramePath::IfThen {
                            if_span: expr.span,
                            then_span: block.span,
                            stmt_idx,
                        });
                        self.attach_suspend_source_paths_in_stmt(stmt, root, path);
                        let _ = path.pop();
                    }
                } else {
                    self.attach_suspend_source_paths_in_expr(then_branch, root, path);
                }
                if let Some(else_expr) = else_branch.as_deref()
                    && let hir::ExprKind::Block(block) = &else_expr.kind
                {
                    for (stmt_idx, stmt) in block.stmts.iter().enumerate() {
                        path.push(SuspendSourceFramePath::IfElse {
                            if_span: expr.span,
                            else_span: block.span,
                            stmt_idx,
                        });
                        self.attach_suspend_source_paths_in_stmt(stmt, root, path);
                        let _ = path.pop();
                    }
                } else if let Some(else_expr) = else_branch.as_deref() {
                    self.attach_suspend_source_paths_in_expr(else_expr, root, path);
                }
            }
            hir::ExprKind::When { subject, arms } => {
                self.attach_suspend_source_paths_in_expr(subject, root, path);
                for (arm_index, when_arm) in arms.iter().enumerate() {
                    if let Some(guard) = when_arm.guard.as_ref() {
                        self.attach_suspend_source_paths_in_expr(guard, root, path);
                    }
                    if let hir::ExprKind::Block(block) = &when_arm.body.kind {
                        for (stmt_idx, stmt) in block.stmts.iter().enumerate() {
                            path.push(SuspendSourceFramePath::WhenArm {
                                when_span: expr.span,
                                arm_index,
                                arm_span: block.span,
                                stmt_idx,
                            });
                            self.attach_suspend_source_paths_in_stmt(stmt, root, path);
                            let _ = path.pop();
                        }
                    } else {
                        self.attach_suspend_source_paths_in_expr(&when_arm.body, root, path);
                    }
                }
            }
            hir::ExprKind::MemberAccess { receiver, .. } => {
                self.attach_suspend_source_paths_in_expr(receiver, root, path);
            }
            hir::ExprKind::Call { callee, args } => {
                self.attach_suspend_source_paths_in_expr(callee, root, path);
                for arg in args {
                    match arg {
                        hir::CallArg::Positional(arg_expr) => {
                            self.attach_suspend_source_paths_in_expr(arg_expr, root, path)
                        }
                        hir::CallArg::Named { value, .. } => {
                            self.attach_suspend_source_paths_in_expr(value, root, path)
                        }
                    }
                }
            }
            hir::ExprKind::Perform { args, .. } => {
                for arg in args {
                    match arg {
                        hir::CallArg::Positional(arg_expr) => {
                            self.attach_suspend_source_paths_in_expr(arg_expr, root, path)
                        }
                        hir::CallArg::Named { value, .. } => {
                            self.attach_suspend_source_paths_in_expr(value, root, path)
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn record_suspend_source_path(
        &mut self,
        expr: &'hir hir::Expr,
        root: &SuspendSourceRoot,
        path: &[SuspendSourceFramePath],
    ) {
        let Some(site) = self.suspend_sites.iter_mut().find(|site| {
            suspend_site_kind_matches_source_path_expr_kind(&site.kind, &expr.kind)
                && site.span == expr.span
                && site.source_path.is_none()
        }) else {
            return;
        };
        site.source_path = Some(SuspendSourcePath {
            root: root.clone(),
            frames: path.to_vec(),
        });
    }

    pub(crate) fn attach_suspend_resume_paths(&mut self) {
        for stmt in &self.handle.body.stmts {
            self.attach_suspend_resume_paths_in_stmt(stmt);
        }
        for arm in &self.handle.arms {
            self.attach_suspend_resume_paths_in_expr(
                &arm.body,
                SuspendResumeConsumer::ExprStmt,
                &mut Vec::new(),
            );
        }
        if let Some(finally_block) = self.handle.finally.as_ref() {
            for stmt in &finally_block.stmts {
                self.attach_suspend_resume_paths_in_stmt(stmt);
            }
        }
    }

    pub(crate) fn attach_suspend_resume_paths_in_stmt(&mut self, stmt: &'hir hir::Stmt) {
        match &stmt.kind {
            hir::StmtKind::Empty
            | hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Todo(_) => {}
            hir::StmtKind::Expr(expr) => {
                self.attach_suspend_resume_paths_in_expr(
                    expr,
                    SuspendResumeConsumer::ExprStmt,
                    &mut Vec::new(),
                );
            }
            hir::StmtKind::Val(decl) => {
                if let Some(init) = decl.init.as_ref() {
                    self.attach_suspend_resume_paths_in_expr(
                        init,
                        SuspendResumeConsumer::ValInit,
                        &mut Vec::new(),
                    );
                }
            }
            hir::StmtKind::Assign { lhs, rhs, .. } => {
                self.attach_suspend_resume_paths_in_expr(
                    lhs,
                    SuspendResumeConsumer::AssignLhs,
                    &mut Vec::new(),
                );
                self.attach_suspend_resume_paths_in_expr(
                    rhs,
                    SuspendResumeConsumer::AssignRhs,
                    &mut Vec::new(),
                );
            }
            hir::StmtKind::While { cond, body } => {
                self.attach_suspend_resume_paths_in_expr(
                    cond,
                    SuspendResumeConsumer::WhileCond,
                    &mut Vec::new(),
                );
                for stmt in &body.stmts {
                    self.attach_suspend_resume_paths_in_stmt(stmt);
                }
            }
            hir::StmtKind::Return { value } => {
                if let Some(expr) = value {
                    self.attach_suspend_resume_paths_in_expr(
                        expr,
                        SuspendResumeConsumer::ReturnValue,
                        &mut Vec::new(),
                    );
                }
            }
        }
    }

    pub(crate) fn attach_suspend_resume_paths_in_expr(
        &mut self,
        expr: &'hir hir::Expr,
        consumer: SuspendResumeConsumer,
        expr_frames: &mut Vec<SuspendResumeExprFrame>,
    ) {
        self.record_suspend_resume_path(expr, consumer, expr_frames);
        match &expr.kind {
            hir::ExprKind::Missing
            | hir::ExprKind::Literal(_)
            | hir::ExprKind::VarRef(_)
            | hir::ExprKind::UnresolvedIdent { .. }
            | hir::ExprKind::ClassLiteral(_)
            | hir::ExprKind::Closure(_)
            | hir::ExprKind::Todo(_) => {}
            hir::ExprKind::StructLit { fields, .. } => {
                for field in fields {
                    expr_frames.push(SuspendResumeExprFrame::StructField {
                        struct_span: expr.span,
                        field_name: field.name.clone(),
                    });
                    self.attach_suspend_resume_paths_in_expr(&field.value, consumer, expr_frames);
                    let _ = expr_frames.pop();
                }
            }
            hir::ExprKind::TupleLit { elements } => {
                for (element_index, element) in elements.iter().enumerate() {
                    expr_frames.push(SuspendResumeExprFrame::TupleElement {
                        tuple_span: expr.span,
                        element_index,
                    });
                    self.attach_suspend_resume_paths_in_expr(element, consumer, expr_frames);
                    let _ = expr_frames.pop();
                }
            }
            hir::ExprKind::InterpolatedString { parts, .. } => {
                for (part_index, part) in parts.iter().enumerate() {
                    let hir::InterpolatedStringPart::Expr { expr: part_expr } = part else {
                        continue;
                    };
                    expr_frames.push(SuspendResumeExprFrame::InterpolatedExpr {
                        string_span: expr.span,
                        part_index,
                    });
                    self.attach_suspend_resume_paths_in_expr(part_expr, consumer, expr_frames);
                    let _ = expr_frames.pop();
                }
            }
            hir::ExprKind::Unary { expr: inner, .. } => {
                expr_frames.push(SuspendResumeExprFrame::UnaryOperand {
                    expr_span: expr.span,
                });
                self.attach_suspend_resume_paths_in_expr(inner, consumer, expr_frames);
                let _ = expr_frames.pop();
            }
            hir::ExprKind::Binary { lhs, rhs, .. } => {
                expr_frames.push(SuspendResumeExprFrame::BinaryLhs {
                    binary_span: expr.span,
                });
                self.attach_suspend_resume_paths_in_expr(lhs, consumer, expr_frames);
                let _ = expr_frames.pop();

                expr_frames.push(SuspendResumeExprFrame::BinaryRhs {
                    binary_span: expr.span,
                });
                self.attach_suspend_resume_paths_in_expr(rhs, consumer, expr_frames);
                let _ = expr_frames.pop();
            }
            hir::ExprKind::TypeCheck { expr: inner, .. } => {
                expr_frames.push(SuspendResumeExprFrame::TypeCheckOperand {
                    expr_span: expr.span,
                });
                self.attach_suspend_resume_paths_in_expr(inner, consumer, expr_frames);
                let _ = expr_frames.pop();
            }
            hir::ExprKind::Cast { expr: inner, .. } => {
                expr_frames.push(SuspendResumeExprFrame::CastOperand {
                    expr_span: expr.span,
                });
                self.attach_suspend_resume_paths_in_expr(inner, consumer, expr_frames);
                let _ = expr_frames.pop();
            }
            hir::ExprKind::Block(block) => {
                for stmt in &block.stmts {
                    self.attach_suspend_resume_paths_in_stmt(stmt);
                }
            }
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                expr_frames.push(SuspendResumeExprFrame::IfCond { if_span: expr.span });
                self.attach_suspend_resume_paths_in_expr(cond, consumer, expr_frames);
                let _ = expr_frames.pop();

                expr_frames.push(SuspendResumeExprFrame::IfThenExpr { if_span: expr.span });
                self.attach_suspend_resume_paths_in_expr(then_branch, consumer, expr_frames);
                let _ = expr_frames.pop();

                if let Some(else_branch) = else_branch.as_deref() {
                    expr_frames.push(SuspendResumeExprFrame::IfElseExpr { if_span: expr.span });
                    self.attach_suspend_resume_paths_in_expr(else_branch, consumer, expr_frames);
                    let _ = expr_frames.pop();
                }
            }
            hir::ExprKind::When { subject, arms } => {
                expr_frames.push(SuspendResumeExprFrame::WhenSubject {
                    when_span: expr.span,
                });
                self.attach_suspend_resume_paths_in_expr(subject, consumer, expr_frames);
                let _ = expr_frames.pop();

                for (arm_index, arm) in arms.iter().enumerate() {
                    if let Some(guard) = arm.guard.as_ref() {
                        expr_frames.push(SuspendResumeExprFrame::WhenArmGuard {
                            when_span: expr.span,
                            arm_index,
                        });
                        self.attach_suspend_resume_paths_in_expr(guard, consumer, expr_frames);
                        let _ = expr_frames.pop();
                    }

                    expr_frames.push(SuspendResumeExprFrame::WhenArmBody {
                        when_span: expr.span,
                        arm_index,
                    });
                    self.attach_suspend_resume_paths_in_expr(&arm.body, consumer, expr_frames);
                    let _ = expr_frames.pop();
                }
            }
            hir::ExprKind::MemberAccess { receiver, .. } => {
                expr_frames.push(SuspendResumeExprFrame::MemberReceiver {
                    access_span: expr.span,
                });
                self.attach_suspend_resume_paths_in_expr(receiver, consumer, expr_frames);
                let _ = expr_frames.pop();
            }
            hir::ExprKind::Call { callee, args } => {
                expr_frames.push(SuspendResumeExprFrame::CallCallee {
                    call_span: expr.span,
                });
                self.attach_suspend_resume_paths_in_expr(callee, consumer, expr_frames);
                let _ = expr_frames.pop();

                for (arg_index, arg) in args.iter().enumerate() {
                    match arg {
                        hir::CallArg::Positional(arg_expr) => {
                            expr_frames.push(SuspendResumeExprFrame::CallArg {
                                call_span: expr.span,
                                arg_index,
                            });
                            self.attach_suspend_resume_paths_in_expr(
                                arg_expr,
                                consumer,
                                expr_frames,
                            );
                            let _ = expr_frames.pop();
                        }
                        hir::CallArg::Named {
                            name_span, value, ..
                        } => {
                            expr_frames.push(SuspendResumeExprFrame::NamedArgValue {
                                call_span: expr.span,
                                arg_index,
                                name_span: *name_span,
                            });
                            self.attach_suspend_resume_paths_in_expr(value, consumer, expr_frames);
                            let _ = expr_frames.pop();
                        }
                    }
                }
            }
            hir::ExprKind::Perform { args, .. } => {
                for (arg_index, arg) in args.iter().enumerate() {
                    let value = match arg {
                        hir::CallArg::Positional(expr) => expr,
                        hir::CallArg::Named { value, .. } => value,
                    };
                    expr_frames.push(SuspendResumeExprFrame::PerformArg {
                        perform_span: expr.span,
                        arg_index,
                    });
                    self.attach_suspend_resume_paths_in_expr(value, consumer, expr_frames);
                    let _ = expr_frames.pop();
                }
            }
            hir::ExprKind::Handle(_) => {
                // Nested handle boundaries keep their own inner state machine
                // contract. We still record the outer resume_path on the
                // boundary expression itself so inactive returns can feed the
                // authoritative nested-handle result into the outer caller-tail
                // without re-running the inner handle.
            }
        }
    }

    pub(crate) fn record_suspend_resume_path(
        &mut self,
        expr: &'hir hir::Expr,
        consumer: SuspendResumeConsumer,
        expr_frames: &[SuspendResumeExprFrame],
    ) {
        let Some(site) = self.suspend_sites.iter_mut().find(|site| {
            suspend_site_kind_matches_resume_path_expr_kind(&site.kind, &expr.kind)
                && site.span == expr.span
                && site.resume_path.is_none()
        }) else {
            return;
        };
        site.resume_path = Some(SuspendResumePath {
            consumer,
            expr_frames: expr_frames.to_vec(),
        });
    }

    pub(crate) fn new_resume_temp_slot(
        &mut self,
        site_id: SuspendSiteId,
        source_expr: &'hir hir::Expr,
    ) -> FrameSlot {
        let id = self.context.allocate_synthetic_symbol_id();
        let slot = FrameSlot {
            id,
            name: format!("__resume_site{site_id}"),
            ty: source_expr.ty,
            mutable: false,
            seed_from_outer_scope: false,
            owner_arm: None,
        };
        self.frame_slots.insert(id, slot.clone());
        slot
    }

    pub(crate) fn record_resume_source_expr(
        &mut self,
        site_id: SuspendSiteId,
        source_expr: &'hir hir::Expr,
    ) {
        self.resume_source_exprs
            .entry(site_id)
            .or_insert_with(|| source_expr.clone());
    }

    pub(crate) fn materialize_resume_fragments(&mut self) {
        let resume_paths = self
            .suspend_sites
            .iter()
            .filter_map(|site| site.resume_path.clone().map(|path| (site.id, path)))
            .collect::<HashMap<_, _>>();
        let source_paths = self
            .suspend_sites
            .iter()
            .filter_map(|site| site.source_path.clone().map(|path| (site.id, path)))
            .collect::<HashMap<_, _>>();

        let original_state_count = self.states.len();
        for state_index in 0..original_state_count {
            let state_id = self.states[state_index].id;
            let mut rewrites = self.states[state_index]
                .actions
                .iter()
                .enumerate()
                .filter_map(|(op_index, op)| match op {
                    HandleStateOp::ResumeAfterSite {
                        site_id,
                        resume_slot: Some(resume_slot),
                        ..
                    } => resume_paths.get(site_id).cloned().map(|resume_path| {
                        let source_expr = self
                            .resume_source_exprs
                            .get(site_id)
                            .unwrap_or_else(|| {
                                panic!(
                                    "resume source expr missing for site{site_id} during rewrite"
                                )
                            })
                            .clone();
                        (
                            op_index,
                            *site_id,
                            source_expr,
                            resume_path,
                            source_paths.get(site_id).cloned(),
                            resume_slot.clone(),
                        )
                    }),
                    _ => None,
                })
                .collect::<Vec<_>>();

            rewrites.sort_by_key(|entry| std::cmp::Reverse(entry.0));

            for (op_index, site_id, source_expr, resume_path, source_path, resume_slot) in rewrites
            {
                {
                    let state = &mut self.states[state_index];
                    for op in state.actions.iter_mut().skip(op_index + 1) {
                        rewrite_state_op_with_resume_slot(
                            op,
                            &source_expr,
                            &resume_path,
                            &resume_slot,
                        );
                    }
                    rewrite_state_terminator_with_resume_slot(
                        &mut state.terminator,
                        &source_expr,
                        &resume_path,
                        &resume_slot,
                    );
                }

                let Some(source_path) = source_path.as_ref() else {
                    self.clone_linear_resume_consumer_chain(
                        state_id,
                        site_id,
                        &source_expr,
                        &resume_path,
                        &resume_slot,
                    );
                    continue;
                };
                let mut allocate_synthetic_symbol_id =
                    || self.context.allocate_synthetic_symbol_id();
                let mut when_rewrite_input = MaterializedWhenResumeInput {
                    source_path,
                    source_expr: &source_expr,
                    resume_path: &resume_path,
                    resume_slot: &resume_slot,
                    allocate_synthetic_symbol_id: &mut allocate_synthetic_symbol_id,
                };
                let when_rewrite = {
                    let state = &self.states[state_index];
                    prepare_materialized_when_resume_rewrite(
                        &state.actions,
                        op_index,
                        &state.terminator,
                        &mut when_rewrite_input,
                    )
                };
                let Some(when_rewrite) = when_rewrite else {
                    self.clone_linear_resume_consumer_chain(
                        state_id,
                        site_id,
                        &source_expr,
                        &resume_path,
                        &resume_slot,
                    );
                    continue;
                };

                {
                    let state = &mut self.states[state_index];
                    if let Some(replacement_expr) = when_rewrite.replacement_expr.as_ref() {
                        for consumer_index in &when_rewrite.consumer_action_indices {
                            rewrite_state_op_replacing_expr_span(
                                &mut state.actions[*consumer_index],
                                when_rewrite.when_span,
                                replacement_expr,
                            );
                        }
                        if when_rewrite.rewrite_terminator {
                            rewrite_state_terminator_replacing_expr_span(
                                &mut state.terminator,
                                when_rewrite.when_span,
                                replacement_expr,
                            );
                        }
                    }

                    let removal_start = if when_rewrite.replacement_expr.is_some() {
                        op_index + 1
                    } else {
                        when_rewrite.when_index
                    };
                    for action_index in (removal_start..=when_rewrite.when_index).rev() {
                        state.actions.remove(action_index);
                    }
                }

                self.clone_linear_resume_consumer_chain(
                    state_id,
                    site_id,
                    &source_expr,
                    &resume_path,
                    &resume_slot,
                );
            }
        }
    }

    pub(crate) fn clone_linear_resume_consumer_chain(
        &mut self,
        resume_state_id: PlanStateId,
        site_id: SuspendSiteId,
        source_expr: &hir::Expr,
        resume_path: &SuspendResumePath,
        resume_slot: &FrameSlot,
    ) {
        let StateTerminator::Goto(first_target) = &self.state(resume_state_id).terminator else {
            return;
        };
        let first_target = *first_target;

        let candidate_spans = resume_rewrite_candidate_spans(source_expr, resume_path);
        let mut seen = HashSet::new();
        let mut chain = Vec::new();
        let mut current = first_target;

        loop {
            if !seen.insert(current) {
                return;
            }

            let state = self.state(current);
            chain.push(current);
            if state_contains_any_expr_span(state, &candidate_spans) {
                break;
            }

            let StateTerminator::Goto(next) = &state.terminator else {
                return;
            };
            current = *next;
        }

        let mut cloned_ids = Vec::with_capacity(chain.len());
        for _ in &chain {
            let cloned_id = self.next_state_id;
            self.next_state_id = self.next_state_id.saturating_add(1);
            cloned_ids.push(cloned_id);
        }

        let consumer_index = chain.len() - 1;
        let mut cloned_states = Vec::with_capacity(chain.len());
        for (idx, original_state_id) in chain.iter().copied().enumerate() {
            let mut cloned = self.state(original_state_id).clone();
            cloned.id = cloned_ids[idx];
            cloned.label = format!("{}.resume.site{site_id}.clone{idx}", cloned.label);

            if idx == consumer_index {
                for op in &mut cloned.actions {
                    rewrite_state_op_with_resume_slot(op, source_expr, resume_path, resume_slot);
                }
                rewrite_state_terminator_with_resume_slot(
                    &mut cloned.terminator,
                    source_expr,
                    resume_path,
                    resume_slot,
                );
            } else {
                cloned.terminator = StateTerminator::Goto(cloned_ids[idx + 1]);
            }

            cloned_states.push(cloned);
        }

        self.states.extend(cloned_states);
        let state = self.state_mut(resume_state_id);
        if let StateTerminator::Goto(target) = &mut state.terminator
            && *target == first_target
        {
            *target = cloned_ids[0];
        }
    }

    pub(crate) fn new_state(&mut self, label: impl Into<String>) -> PlanStateId {
        let id = self.next_state_id;
        self.next_state_id = self.next_state_id.saturating_add(1);
        self.states.push(PlanState {
            id,
            label: label.into(),
            actions: Vec::new(),
            terminator: StateTerminator::ReturnHandle,
            reads: Vec::new(),
        });
        id
    }

    pub(crate) fn push_action(&mut self, state_id: PlanStateId, action: HandleStateOp) {
        self.state_mut(state_id).actions.push(action);
    }

    pub(crate) fn state(&self, state_id: PlanStateId) -> &PlanState {
        self.states
            .iter()
            .find(|state| state.id == state_id)
            .expect("state should exist")
    }

    pub(crate) fn state_mut(&mut self, state_id: PlanStateId) -> &mut PlanState {
        self.states
            .iter_mut()
            .find(|state| state.id == state_id)
            .expect("state should exist")
    }

    pub(crate) fn set_terminator(&mut self, state_id: PlanStateId, terminator: StateTerminator) {
        self.state_mut(state_id).terminator = terminator;
    }

    pub(crate) fn new_suspend_site(
        &mut self,
        span: Span,
        kind: SuspendSiteKind,
        available_locals: Vec<hir::SymbolId>,
        owner_state: PlanStateId,
    ) -> SuspendSiteId {
        let id = self.next_site_id;
        self.next_site_id = self.next_site_id.saturating_add(1);
        let continuation_escape = self.continuation_escape_state_for_suspend_site(span, &kind);
        self.suspend_sites.push(SuspendSitePlan {
            id,
            span,
            kind,
            owner_state,
            resume_target: 0,
            escape_resume_target: None,
            matching_arms: Vec::new(),
            available_locals,
            capture_locals: Vec::new(),
            source_path: None,
            resume_path: None,
            continuation_escape,
        });
        id
    }

    pub(crate) fn continuation_escape_state_for_suspend_site(
        &self,
        span: Span,
        kind: &SuspendSiteKind,
    ) -> ContinuationEscapeState {
        if kind.is_continuation_resume_boundary() {
            self.context.continuation_escape_state_for_call_span(span)
        } else {
            ContinuationEscapeState::Unknown
        }
    }

    pub(crate) fn set_suspend_resume_target(
        &mut self,
        site_id: SuspendSiteId,
        resume_target: PlanStateId,
    ) {
        let site = self
            .suspend_sites
            .iter_mut()
            .find(|site| site.id == site_id)
            .expect("site should exist");
        site.resume_target = resume_target;
    }

    pub(crate) fn attach_escape_resume_targets(&mut self) {
        let original_state_count = self.states.len();
        let mut replay_states = Vec::<(SuspendSiteId, PlanState)>::new();
        let replayable_sites = self
            .suspend_sites
            .iter()
            .filter(|site| site.kind.needs_escape_resume_replay())
            .map(|site| site.id)
            .collect::<HashSet<_>>();

        for state in self.states.iter().take(original_state_count) {
            let Some(HandleStateOp::ResumeAfterSite {
                resume_slot: Some(_),
                ..
            }) = state.actions.first()
            else {
                continue;
            };
            let StateTerminator::Suspend { site_id } = state.terminator else {
                continue;
            };
            // Direct perform/runtime-raise continuations already resume at their
            // dedicated post-site state. Rewriting them back into an owner-state
            // replay path would duplicate earlier effects/prints and corrupt the
            // captured continuation contract.
            if !replayable_sites.contains(&site_id) {
                continue;
            }
            if state.actions.len() <= 1 {
                continue;
            }
            let Some(site) = self.suspend_sites.iter().find(|site| site.id == site_id) else {
                continue;
            };
            let replay_actions = self.escape_replay_actions_for_site(state, site);
            if replay_actions.is_empty() {
                continue;
            }

            let replay_state_id = self.next_state_id + replay_states.len() as u32;
            let replay_state = PlanState {
                id: replay_state_id,
                label: format!("{}.escape-replay.site{site_id}", state.label),
                actions: replay_actions,
                terminator: state.terminator.clone(),
                reads: state.reads.clone(),
            };
            replay_states.push((site_id, replay_state));
        }

        if replay_states.is_empty() {
            return;
        }

        self.next_state_id = self
            .next_state_id
            .saturating_add(replay_states.len() as u32);
        for (site_id, replay_state) in replay_states {
            let replay_state_id = replay_state.id;
            self.states.push(replay_state);
            let site = self
                .suspend_sites
                .iter_mut()
                .find(|site| site.id == site_id)
                .expect("escape replay target site should exist");
            site.escape_resume_target = Some(replay_state_id);
        }
    }

    pub(crate) fn escape_replay_actions_for_site(
        &self,
        state: &PlanState,
        site: &SuspendSitePlan,
    ) -> Vec<HandleStateOp> {
        let Some(source_path) = site.source_path.as_ref() else {
            return state.actions[1..].to_vec();
        };
        let root_span = source_path.root_span();

        let replay_actions = state.actions[1..]
            .iter()
            .filter(|op| state_op_within_span(op, root_span))
            .cloned()
            .collect::<Vec<_>>();

        if replay_actions.is_empty() {
            state.actions[1..].to_vec()
        } else {
            replay_actions
        }
    }

    pub(crate) fn record_stmt_reads(&mut self, _state_id: PlanStateId, _stmt: &hir::Stmt) {
        let mut used = HashSet::new();
        collect_used_locals_in_stmt_static(_stmt, &mut used);
        self.add_reads(_state_id, used);
    }

    pub(crate) fn authoritative_local_slot(
        &self,
        id: hir::SymbolId,
        name: &str,
        fallback_ty: TypeId,
    ) -> FrameSlot {
        let metadata = self.context.known_local_metadata.get(&id).copied();
        FrameSlot {
            id,
            name: name.to_string(),
            ty: metadata.map_or(fallback_ty, |meta| meta.ty),
            mutable: metadata.is_some_and(|meta| meta.mutable),
            seed_from_outer_scope: false,
            owner_arm: None,
        }
    }

    pub(crate) fn record_expr_reads(&mut self, _state_id: PlanStateId, _expr: &hir::Expr) {
        let mut used = HashSet::new();
        collect_used_locals_in_expr_static(_expr, &mut used);
        self.add_reads(_state_id, used);
    }

    pub(crate) fn add_reads(&mut self, state_id: PlanStateId, used: HashSet<hir::SymbolId>) {
        let state = self.state_mut(state_id);
        state.reads.extend(used);
        state.reads.sort_by_key(|id| id.as_u32());
        state.reads.dedup_by_key(|id| id.as_u32());
    }
}
