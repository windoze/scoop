//!  Ordinary callee suspend planning entry, escape-continuation direct-step effect-row inference and DirectStepAnalysis.

#![allow(dead_code)]

use super::*;

/// Build the shared ordinary callee suspend/resume plan from a function or closure body.
pub fn build_ordinary_callee_suspend_plan_with_context(
    types: &TypeStore,
    body: &hir::Block,
    declared_return_ty: TypeId,
    context: &mut EffectAnalysisCtx,
) -> Option<CalleeSuspendPlan> {
    let synthetic_handle = hir::HandleExpr {
        body: body.clone(),
        arms: Vec::new(),
        finally: None,
    };

    context.extend_known_local_metadata_from_handle(&synthetic_handle);

    let mut builder = HandlePlanBuilder::new(types, &synthetic_handle, context);
    let outer_slots = collect_outer_scope_slots(&synthetic_handle, &context.known_local_metadata);
    let mut env = ScopeEnv::with_outer(outer_slots.clone());
    for slot in &outer_slots {
        builder.frame_slots.insert(slot.id, slot.clone());
    }

    let entry_state = builder.new_state("ordinary.body.entry");
    let _body_end_state = builder.build_block(&synthetic_handle.body, entry_state, &mut env);
    builder.attach_suspend_source_paths();
    builder.attach_suspend_resume_paths();

    if builder.suspend_sites.is_empty() {
        return None;
    }

    let mut allocate_synthetic_symbol_id = || context.allocate_synthetic_symbol_id();
    let mut resume_sites = Vec::new();

    for site in &builder.suspend_sites {
        if !matches!(site.kind, SuspendSiteKind::Perform { .. }) {
            return None;
        }

        let source_path = site.source_path.as_ref()?;
        let resume_path = site.resume_path.as_ref()?;
        let source_expr = builder.resume_source_exprs.get(&site.id)?;
        let resume_slot = builder.resume_slot_for_site(site.id)?;
        let resume_slot_ty = ordinary_callee_resume_slot_type(
            body,
            source_path,
            resume_path,
            declared_return_ty,
            &resume_slot,
        );
        let resume_tail = build_ordinary_callee_resume_tail_block(
            &synthetic_handle.body,
            source_path,
            source_expr,
            resume_path,
            &resume_slot,
            &mut allocate_synthetic_symbol_id,
        )?;

        let saved_locals = site
            .available_locals
            .iter()
            .filter_map(|id| builder.frame_slots.get(id))
            .map(|slot| CalleeSuspendSavedLocal {
                id: slot.id(),
                name: slot.name().to_string(),
                ty: slot.ty(),
                mutable: slot.mutable(),
            })
            .collect::<Vec<_>>();

        resume_sites.push(CalleeSuspendResumeSite {
            site_id: site.id,
            span: site.span,
            saved_locals,
            resume_slot_id: resume_slot.id(),
            resume_slot_name: resume_slot.name().to_string(),
            resume_slot_ty,
            resume_tail,
        });
    }

    let mut seen_local_ids = HashSet::new();
    let mut saved_locals = Vec::new();
    for site in &resume_sites {
        for local in &site.saved_locals {
            if seen_local_ids.insert(local.id) {
                saved_locals.push(local.clone());
            }
        }
    }

    Some(CalleeSuspendPlan {
        saved_locals,
        resume_sites,
    })
}

/// `T4008b1`：为当前 `handle` 中的 escape continuation arm 计算 resumed-step effect row。
pub fn compute_escape_continuation_direct_step_effect_rows_for_handle(
    types: &TypeStore,
    handle: &hir::HandleExpr,
) -> HashMap<hir::SymbolId, EffectRow> {
    compute_escape_continuation_direct_step_effect_rows_for_handle_with_program(types, handle, None)
}

pub fn compute_escape_continuation_direct_step_effect_rows_for_handle_in_program(
    types: &TypeStore,
    handle: &hir::HandleExpr,
    object_inits: &hir::ObjectInitIndex,
    top_level_immutable_values: &hir::TopLevelImmutableValueIndex,
) -> HashMap<hir::SymbolId, EffectRow> {
    compute_escape_continuation_direct_step_effect_rows_for_handle_with_program(
        types,
        handle,
        Some(DirectStepProgramInfo {
            object_inits,
            top_level_immutable_values,
        }),
    )
}

pub fn compute_escape_continuation_direct_step_effect_rows_for_handle_with_program<'a>(
    types: &TypeStore,
    handle: &hir::HandleExpr,
    program: Option<DirectStepProgramInfo<'a>>,
) -> HashMap<hir::SymbolId, EffectRow> {
    let mut by_binder: HashMap<hir::SymbolId, Vec<TypeId>> = HashMap::new();
    for site_summary in compute_escape_continuation_direct_step_rows_by_site(types, handle, program)
    {
        by_binder
            .entry(site_summary.continuation)
            .or_default()
            .extend(site_summary.effects.terms);
    }

    by_binder
        .into_iter()
        .map(|(continuation, effects)| (continuation, EffectRow::new(effects)))
        .collect()
}

#[derive(Debug, Clone)]
pub struct EscapeSiteDirectStepRow {
    pub site_id: SuspendSiteId,
    pub continuation: hir::SymbolId,
    pub effects: EffectRow,
}

pub fn compute_escape_continuation_direct_step_rows_by_site(
    types: &TypeStore,
    handle: &hir::HandleExpr,
    program: Option<DirectStepProgramInfo<'_>>,
) -> Vec<EscapeSiteDirectStepRow> {
    let mut context = direct_step_analysis_context_for_handle(handle);
    context.extend_known_local_metadata_from_handle(handle);

    let mut builder = HandlePlanBuilder::new(types, handle, &context);
    let outer_slots = collect_outer_scope_slots(handle, &context.known_local_metadata);
    let mut env = ScopeEnv::with_outer(outer_slots.clone());
    for slot in &outer_slots {
        builder.frame_slots.insert(slot.id, slot.clone());
    }

    let entry_state = builder.new_state("body.entry");
    let exit_state = builder.new_state("body.exit");
    let body_end_state = builder.build_block(&handle.body, entry_state, &mut env);

    if let Some(finally_block) = &handle.finally {
        let cleanup_entry = builder.new_state("cleanup.finally.entry");
        let cleanup_exit = builder.new_state("cleanup.finally.exit");
        let cleanup_scope_id = builder.next_cleanup_id;
        builder.next_cleanup_id = builder.next_cleanup_id.saturating_add(1);
        builder.cleanup_scopes.push(CleanupScopePlan {
            id: cleanup_scope_id,
            kind: CleanupScopeKind::Finally,
            entry_state: cleanup_entry,
            exit_state: cleanup_exit,
            note: "normal/raise edges converge through a shared finally scope".to_string(),
        });
        builder.set_terminator(
            body_end_state,
            StateTerminator::CleanupEnter {
                scope_id: cleanup_scope_id,
                next_state: cleanup_entry,
            },
        );
        let mut cleanup_env = ScopeEnv::with_outer(outer_slots);
        let cleanup_end = builder.build_block(finally_block, cleanup_entry, &mut cleanup_env);
        builder
            .state_mut(cleanup_end)
            .actions
            .push(HandleStateOp::CleanupEdgeComplete);
        builder.set_terminator(cleanup_end, StateTerminator::Goto(cleanup_exit));
        builder.set_terminator(cleanup_exit, StateTerminator::Goto(exit_state));
    } else {
        builder.set_terminator(body_end_state, StateTerminator::Goto(exit_state));
    }

    builder
        .state_mut(exit_state)
        .actions
        .push(HandleStateOp::ReturnToEnclosingExpression);
    builder.set_terminator(exit_state, StateTerminator::ReturnHandle);

    let _dispatch_plan = builder.build_dispatch_plan();
    builder.build_arm_states();
    builder.compute_capture_sets();
    builder.attach_suspend_source_paths();
    builder.attach_suspend_resume_paths();
    builder.materialize_resume_fragments();
    builder.attach_escape_resume_targets();
    builder.compute_capture_sets();

    let mut rows = Vec::new();
    for site in &builder.suspend_sites {
        let SuspendSiteKind::Perform { op_fqn } = &site.kind else {
            continue;
        };
        let Some(source_expr) = builder.resume_source_exprs.get(&site.id) else {
            continue;
        };
        let hir::ExprKind::Perform { effect_ty, .. } = &source_expr.kind else {
            continue;
        };
        let Some(continuation) =
            select_escape_continuation_for_direct_site(handle, op_fqn, *effect_ty)
        else {
            continue;
        };
        let Some(source_path) = site.source_path.as_ref() else {
            continue;
        };
        let Some(resume_path) = site.resume_path.as_ref() else {
            continue;
        };
        let Some(resume_slot) = builder.resume_slot_for_site(site.id) else {
            continue;
        };
        let mut allocate_synthetic_symbol_id = || context.allocate_synthetic_symbol_id();
        let Some(resume_tail) = build_ordinary_callee_resume_tail_block(
            &handle.body,
            source_path,
            source_expr,
            resume_path,
            &resume_slot,
            &mut allocate_synthetic_symbol_id,
        ) else {
            continue;
        };
        let effects = summarize_direct_step_effects_in_block(
            types,
            &resume_tail,
            handle,
            &context.known_local_metadata,
            program,
        );
        rows.push(EscapeSiteDirectStepRow {
            site_id: site.id,
            continuation,
            effects,
        });
    }

    rows
}

pub fn direct_step_analysis_context_for_handle(handle: &hir::HandleExpr) -> HandlePlanContext {
    let mut known_local_metadata = HashMap::new();
    collect_known_local_metadata_in_handle(handle, &mut known_local_metadata);
    HandlePlanContext::new(
        HashMap::new(),
        HashMap::new(),
        known_local_metadata,
        PathBuf::from("<t4008b1a>"),
        Rc::new(EffectAnalysisFacts::default()),
    )
}

pub fn select_escape_continuation_for_direct_site(
    handle: &hir::HandleExpr,
    op_fqn: &str,
    effect_ty: TypeId,
) -> Option<hir::SymbolId> {
    let mut same_op_fallback = None;
    for arm in &handle.arms {
        if arm.op.op.fqn != op_fqn {
            continue;
        }
        if same_op_fallback.is_none()
            && let hir::HandleArmKind::EscapeContinuation { continuation } = arm.kind
        {
            same_op_fallback = Some(continuation);
        }

        if arm.op.effect_ty != effect_ty {
            continue;
        }
        match arm.kind {
            hir::HandleArmKind::EscapeContinuation { continuation } => return Some(continuation),
            hir::HandleArmKind::NonResuming => return None,
        }
    }
    same_op_fallback
}

pub fn summarize_direct_step_effects_in_block(
    types: &TypeStore,
    block: &hir::Block,
    handle: &hir::HandleExpr,
    known_local_metadata: &HashMap<hir::SymbolId, KnownLocalMetadata>,
    program: Option<DirectStepProgramInfo<'_>>,
) -> EffectRow {
    let analysis = DirectStepAnalysis::new(program);
    let summary = summarize_direct_step_resume_tail_block(
        types,
        block,
        handle,
        known_local_metadata,
        &analysis,
    );
    EffectRow::new(summary.effects)
}

#[derive(Debug, Clone, Copy)]
pub struct DirectStepProgramInfo<'a> {
    pub object_inits: &'a hir::ObjectInitIndex,
    pub top_level_immutable_values: &'a hir::TopLevelImmutableValueIndex,
}

#[derive(Debug, Clone)]
pub struct DirectStepAnalysis<'a> {
    pub program: Option<DirectStepProgramInfo<'a>>,
    pub hidden_boundary_stack: HashSet<String>,
}

impl<'a> DirectStepAnalysis<'a> {
    pub fn new(program: Option<DirectStepProgramInfo<'a>>) -> Self {
        Self {
            program,
            hidden_boundary_stack: HashSet::new(),
        }
    }

    pub fn for_hidden_boundary(&self, key: &str) -> Option<Self> {
        if self.hidden_boundary_stack.contains(key) {
            return None;
        }
        let mut next = self.clone();
        next.hidden_boundary_stack.insert(key.to_string());
        Some(next)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum DirectStepHandleRole {
    ResumeStep,
    HandleExpression,
}

#[derive(Debug, Clone, Copy)]
pub struct ActiveDirectStepHandleContext<'a> {
    pub handle: &'a hir::HandleExpr,
    pub role: DirectStepHandleRole,
}

#[derive(Debug, Clone, Copy)]
pub enum DirectStepMode<'a> {
    OutsideHandle,
    ActiveHandle(ActiveDirectStepHandleContext<'a>),
}

#[derive(Debug, Clone, Copy)]
pub enum DirectStepTerminalKind {
    HandleCompletion,
    TerminalStop,
}

#[derive(Debug, Clone)]
pub struct DirectStepSummary {
    pub effects: Vec<TypeId>,
    pub may_continue: bool,
    pub may_stop: bool,
}

impl DirectStepSummary {
    pub fn empty() -> Self {
        Self {
            effects: Vec::new(),
            may_continue: false,
            may_stop: false,
        }
    }

    pub fn continue_pure() -> Self {
        Self {
            effects: Vec::new(),
            may_continue: true,
            may_stop: false,
        }
    }

    pub fn stop_pure() -> Self {
        Self {
            effects: Vec::new(),
            may_continue: false,
            may_stop: true,
        }
    }

    pub fn outward(mut effects: Vec<TypeId>) -> Self {
        effects.sort();
        effects.dedup();
        Self {
            effects,
            may_continue: false,
            may_stop: false,
        }
    }

    pub fn merge_effects(&mut self, mut more: Vec<TypeId>) {
        self.effects.append(&mut more);
        self.effects.sort();
        self.effects.dedup();
    }

    pub fn merge_paths(&mut self, other: Self) {
        self.merge_effects(other.effects);
        self.may_continue |= other.may_continue;
        self.may_stop |= other.may_stop;
    }

    pub fn without_continue(&self) -> Self {
        Self {
            effects: self.effects.clone(),
            may_continue: false,
            may_stop: self.may_stop,
        }
    }
}

#[derive(Debug, Clone)]
pub enum DirectStepHiddenBoundary<'a> {
    TopLevelImmutable {
        fqn: String,
        value: &'a hir::TopLevelImmutableValue,
    },
    ObjectInit {
        fqn: String,
        init: &'a hir::ObjectInit,
    },
}

impl<'a> DirectStepHiddenBoundary<'a> {
    pub fn key(&self) -> &str {
        match self {
            DirectStepHiddenBoundary::TopLevelImmutable { fqn, .. }
            | DirectStepHiddenBoundary::ObjectInit { fqn, .. } => fqn,
        }
    }
}

pub fn summarize_direct_step_resume_tail_block(
    types: &TypeStore,
    block: &hir::Block,
    handle: &hir::HandleExpr,
    known_local_metadata: &HashMap<hir::SymbolId, KnownLocalMetadata>,
    analysis: &DirectStepAnalysis<'_>,
) -> DirectStepSummary {
    let ctx = ActiveDirectStepHandleContext {
        handle,
        role: DirectStepHandleRole::ResumeStep,
    };
    let summary = summarize_direct_step_stmt_seq(
        types,
        &block.stmts,
        DirectStepMode::ActiveHandle(ctx),
        known_local_metadata,
        analysis,
    );
    let mut out = summary.without_continue();
    if summary.may_continue {
        out.merge_paths(finalize_handle_terminal(
            types,
            ctx,
            analysis,
            DirectStepTerminalKind::HandleCompletion,
        ));
    }
    out
}

pub fn summarize_direct_step_handle_execution(
    types: &TypeStore,
    handle: &hir::HandleExpr,
    role: DirectStepHandleRole,
    analysis: &DirectStepAnalysis<'_>,
) -> DirectStepSummary {
    let mut known_local_metadata = HashMap::new();
    collect_known_local_metadata_in_handle(handle, &mut known_local_metadata);
    let ctx = ActiveDirectStepHandleContext { handle, role };
    let summary = summarize_direct_step_stmt_seq(
        types,
        &handle.body.stmts,
        DirectStepMode::ActiveHandle(ctx),
        &known_local_metadata,
        analysis,
    );
    let mut out = summary.without_continue();
    if summary.may_continue {
        out.merge_paths(finalize_handle_terminal(
            types,
            ctx,
            analysis,
            DirectStepTerminalKind::HandleCompletion,
        ));
    }
    out
}

pub fn summarize_direct_step_stmt_seq(
    types: &TypeStore,
    stmts: &[hir::Stmt],
    mode: DirectStepMode<'_>,
    known_local_metadata: &HashMap<hir::SymbolId, KnownLocalMetadata>,
    analysis: &DirectStepAnalysis<'_>,
) -> DirectStepSummary {
    let mut out = DirectStepSummary::continue_pure();
    for stmt in stmts {
        if !out.may_continue {
            break;
        }
        let step = summarize_direct_step_stmt(types, stmt, mode, known_local_metadata, analysis);
        out.merge_effects(step.effects);
        out.may_stop |= step.may_stop;
        out.may_continue = step.may_continue;
    }
    out
}

pub fn summarize_direct_step_stmt(
    types: &TypeStore,
    stmt: &hir::Stmt,
    mode: DirectStepMode<'_>,
    known_local_metadata: &HashMap<hir::SymbolId, KnownLocalMetadata>,
    analysis: &DirectStepAnalysis<'_>,
) -> DirectStepSummary {
    match &stmt.kind {
        hir::StmtKind::Empty => DirectStepSummary::continue_pure(),
        hir::StmtKind::Expr(expr) => {
            summarize_direct_step_expr(types, expr, mode, known_local_metadata, analysis)
        }
        hir::StmtKind::Val(decl) => decl
            .init
            .as_ref()
            .map(|expr| {
                summarize_direct_step_expr(types, expr, mode, known_local_metadata, analysis)
            })
            .unwrap_or_else(DirectStepSummary::continue_pure),
        hir::StmtKind::Assign { lhs, rhs, .. } => {
            let lhs_summary =
                summarize_direct_step_expr(types, lhs, mode, known_local_metadata, analysis);
            let mut out = lhs_summary.without_continue();
            if lhs_summary.may_continue {
                let rhs_summary =
                    summarize_direct_step_expr(types, rhs, mode, known_local_metadata, analysis);
                out.merge_paths(rhs_summary);
            }
            out
        }
        hir::StmtKind::Return { value } => summarize_direct_step_return_stmt(
            types,
            value.as_ref(),
            mode,
            known_local_metadata,
            analysis,
        ),
        hir::StmtKind::While { cond, body } => {
            let cond_summary =
                summarize_direct_step_expr(types, cond, mode, known_local_metadata, analysis);
            let mut out = cond_summary.without_continue();
            if cond_summary.may_continue {
                let body_summary = summarize_direct_step_stmt_seq(
                    types,
                    &body.stmts,
                    mode,
                    known_local_metadata,
                    analysis,
                );
                out.merge_effects(body_summary.effects);
                out.may_stop |= body_summary.may_stop;
                // 仍保留保守 loop union 近似；更细的 break/continue
                // path-sensitive 语义不在 T4008b1b 范围。
                out.may_continue = true;
            }
            out
        }
        hir::StmtKind::Break { .. } | hir::StmtKind::Continue { .. } => {
            DirectStepSummary::stop_pure()
        }
        hir::StmtKind::Todo(_) => DirectStepSummary::continue_pure(),
    }
}

pub fn summarize_direct_step_return_stmt(
    types: &TypeStore,
    value: Option<&hir::Expr>,
    mode: DirectStepMode<'_>,
    known_local_metadata: &HashMap<hir::SymbolId, KnownLocalMetadata>,
    analysis: &DirectStepAnalysis<'_>,
) -> DirectStepSummary {
    let value_summary = value
        .map(|expr| summarize_direct_step_expr(types, expr, mode, known_local_metadata, analysis))
        .unwrap_or_else(DirectStepSummary::continue_pure);
    let mut out = value_summary.without_continue();
    if value_summary.may_continue {
        match mode {
            DirectStepMode::OutsideHandle => out.may_stop = true,
            DirectStepMode::ActiveHandle(ctx) => out.merge_paths(finalize_handle_terminal(
                types,
                ctx,
                analysis,
                DirectStepTerminalKind::TerminalStop,
            )),
        }
    }
    out
}

pub fn summarize_direct_step_expr(
    types: &TypeStore,
    expr: &hir::Expr,
    mode: DirectStepMode<'_>,
    known_local_metadata: &HashMap<hir::SymbolId, KnownLocalMetadata>,
    analysis: &DirectStepAnalysis<'_>,
) -> DirectStepSummary {
    match &expr.kind {
        hir::ExprKind::Missing
        | hir::ExprKind::Literal(_)
        | hir::ExprKind::UnresolvedIdent { .. }
        | hir::ExprKind::ClassLiteral(_)
        | hir::ExprKind::Closure(_)
        | hir::ExprKind::Todo(_) => DirectStepSummary::continue_pure(),
        hir::ExprKind::VarRef(value_ref) => {
            if let Some(boundary) =
                classify_direct_step_hidden_boundary_for_value_ref(analysis.program, value_ref)
            {
                summarize_hidden_boundary_access(types, boundary, mode, analysis)
            } else {
                DirectStepSummary::continue_pure()
            }
        }
        hir::ExprKind::Block(block) => summarize_direct_step_stmt_seq(
            types,
            &block.stmts,
            mode,
            known_local_metadata,
            analysis,
        ),
        hir::ExprKind::Unary { expr: inner, .. }
        | hir::ExprKind::Cast { expr: inner, .. }
        | hir::ExprKind::TypeCheck { expr: inner, .. } => {
            summarize_direct_step_expr(types, inner, mode, known_local_metadata, analysis)
        }
        hir::ExprKind::MemberAccess { receiver, member } => {
            let receiver_summary =
                summarize_direct_step_expr(types, receiver, mode, known_local_metadata, analysis);
            let mut out = receiver_summary.without_continue();
            if !receiver_summary.may_continue {
                return out;
            }
            if let Some(boundary) =
                classify_direct_step_hidden_boundary_for_member_access(analysis.program, member)
            {
                out.merge_paths(summarize_hidden_boundary_access(
                    types, boundary, mode, analysis,
                ));
            } else {
                out.may_continue = true;
            }
            out
        }
        hir::ExprKind::StructLit { fields, .. } => summarize_direct_step_exprs(
            types,
            fields.iter().map(|field| &field.value),
            mode,
            known_local_metadata,
            analysis,
        ),
        hir::ExprKind::TupleLit { elements } => summarize_direct_step_exprs(
            types,
            elements.iter(),
            mode,
            known_local_metadata,
            analysis,
        ),
        hir::ExprKind::InterpolatedString { parts, .. } => summarize_direct_step_exprs(
            types,
            parts.iter().filter_map(|part| match part {
                hir::InterpolatedStringPart::Expr { expr } => Some(expr),
                hir::InterpolatedStringPart::Text { .. } => None,
            }),
            mode,
            known_local_metadata,
            analysis,
        ),
        hir::ExprKind::Binary { lhs, rhs, .. } => {
            let lhs_summary =
                summarize_direct_step_expr(types, lhs, mode, known_local_metadata, analysis);
            let mut out = lhs_summary.without_continue();
            if lhs_summary.may_continue {
                let rhs_summary =
                    summarize_direct_step_expr(types, rhs, mode, known_local_metadata, analysis);
                out.merge_paths(rhs_summary);
            }
            out
        }
        hir::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            let cond_summary =
                summarize_direct_step_expr(types, cond, mode, known_local_metadata, analysis);
            let mut out = cond_summary.without_continue();
            if cond_summary.may_continue {
                let then_summary = summarize_direct_step_expr(
                    types,
                    then_branch,
                    mode,
                    known_local_metadata,
                    analysis,
                );
                let else_summary = else_branch
                    .as_deref()
                    .map(|expr| {
                        summarize_direct_step_expr(
                            types,
                            expr,
                            mode,
                            known_local_metadata,
                            analysis,
                        )
                    })
                    .unwrap_or_else(DirectStepSummary::continue_pure);
                out.merge_paths(then_summary);
                out.merge_paths(else_summary);
            }
            out
        }
        hir::ExprKind::When { subject, arms } => {
            let subject_summary =
                summarize_direct_step_expr(types, subject, mode, known_local_metadata, analysis);
            let mut out = subject_summary.without_continue();
            if !subject_summary.may_continue {
                return out;
            }
            if arms.is_empty() {
                out.may_continue = true;
                return out;
            }
            let mut branch_union = DirectStepSummary::empty();
            for arm in arms {
                let guard_summary = arm
                    .guard
                    .as_ref()
                    .map(|guard| {
                        summarize_direct_step_expr(
                            types,
                            guard,
                            mode,
                            known_local_metadata,
                            analysis,
                        )
                    })
                    .unwrap_or_else(DirectStepSummary::continue_pure);
                let mut branch = guard_summary.without_continue();
                if guard_summary.may_continue {
                    branch.merge_paths(summarize_direct_step_expr(
                        types,
                        &arm.body,
                        mode,
                        known_local_metadata,
                        analysis,
                    ));
                }
                branch_union.merge_paths(branch);
            }
            out.merge_paths(branch_union);
            out
        }
        hir::ExprKind::Call { callee, args } => summarize_direct_step_call_expr(
            types,
            callee,
            args,
            mode,
            known_local_metadata,
            analysis,
        ),
        hir::ExprKind::Perform {
            effect_ty,
            op,
            args,
        } => summarize_direct_step_perform_expr(
            types,
            *effect_ty,
            &op.fqn,
            args,
            mode,
            known_local_metadata,
            analysis,
        ),
        hir::ExprKind::Handle(handle) => match mode {
            DirectStepMode::OutsideHandle => summarize_direct_step_handle_execution(
                types,
                handle,
                DirectStepHandleRole::HandleExpression,
                analysis,
            ),
            DirectStepMode::ActiveHandle(ctx) => finalize_boundary_summary_in_mode(
                types,
                summarize_direct_step_handle_execution(
                    types,
                    handle,
                    DirectStepHandleRole::HandleExpression,
                    analysis,
                ),
                DirectStepMode::ActiveHandle(ctx),
                analysis,
            ),
        },
    }
}

pub fn summarize_direct_step_call_expr(
    types: &TypeStore,
    callee: &hir::Expr,
    args: &[hir::CallArg],
    mode: DirectStepMode<'_>,
    known_local_metadata: &HashMap<hir::SymbolId, KnownLocalMetadata>,
    analysis: &DirectStepAnalysis<'_>,
) -> DirectStepSummary {
    let callee_summary =
        summarize_direct_step_expr(types, callee, mode, known_local_metadata, analysis);
    let mut out = callee_summary.without_continue();
    if !callee_summary.may_continue {
        return out;
    }

    let args_summary =
        summarize_direct_step_call_args(types, args, mode, known_local_metadata, analysis);
    out.merge_effects(args_summary.effects.clone());
    out.may_stop |= args_summary.may_stop;
    if !args_summary.may_continue {
        return out;
    }

    let direct_effects =
        direct_effect_terms_from_callable_expr(types, callee, known_local_metadata);
    match mode {
        DirectStepMode::OutsideHandle => {
            out.merge_effects(direct_effects.clone());
            out.may_continue = direct_effects.is_empty();
            out
        }
        DirectStepMode::ActiveHandle(_) => {
            let mut boundary = DirectStepSummary::continue_pure();
            boundary.merge_effects(direct_effects);
            out.merge_paths(finalize_boundary_summary_in_mode(
                types, boundary, mode, analysis,
            ));
            out
        }
    }
}

pub fn summarize_direct_step_perform_expr(
    types: &TypeStore,
    effect_ty: TypeId,
    op_fqn: &str,
    args: &[hir::CallArg],
    mode: DirectStepMode<'_>,
    known_local_metadata: &HashMap<hir::SymbolId, KnownLocalMetadata>,
    analysis: &DirectStepAnalysis<'_>,
) -> DirectStepSummary {
    let args_summary =
        summarize_direct_step_call_args(types, args, mode, known_local_metadata, analysis);
    let mut out = args_summary.without_continue();
    if !args_summary.may_continue {
        return out;
    }

    match mode {
        DirectStepMode::OutsideHandle => {
            out.merge_paths(DirectStepSummary::outward(vec![effect_ty]));
            out
        }
        DirectStepMode::ActiveHandle(ctx) => {
            if let Some(arm) = first_matching_arm_for_direct_perform(ctx.handle, op_fqn, effect_ty)
            {
                out.merge_paths(summarize_direct_step_dispatch_arm(
                    types, arm, ctx, analysis,
                ));
            } else {
                out.merge_paths(finalize_handle_outward(
                    types,
                    ctx,
                    analysis,
                    vec![effect_ty],
                ));
            }
            out
        }
    }
}

pub fn summarize_hidden_boundary_access(
    types: &TypeStore,
    boundary: DirectStepHiddenBoundary<'_>,
    mode: DirectStepMode<'_>,
    analysis: &DirectStepAnalysis<'_>,
) -> DirectStepSummary {
    let boundary_summary = summarize_hidden_boundary(types, boundary, analysis);
    finalize_boundary_summary_in_mode(types, boundary_summary, mode, analysis)
}

pub fn summarize_hidden_boundary(
    types: &TypeStore,
    boundary: DirectStepHiddenBoundary<'_>,
    analysis: &DirectStepAnalysis<'_>,
) -> DirectStepSummary {
    let Some(next_analysis) = analysis.for_hidden_boundary(boundary.key()) else {
        return DirectStepSummary::stop_pure();
    };
    match boundary {
        DirectStepHiddenBoundary::TopLevelImmutable { value, .. } => value
            .init
            .as_ref()
            .map(|init| {
                let mut known_local_metadata = HashMap::new();
                collect_known_local_metadata_in_expr(init, &mut known_local_metadata);
                summarize_direct_step_expr(
                    types,
                    init,
                    DirectStepMode::OutsideHandle,
                    &known_local_metadata,
                    &next_analysis,
                )
            })
            .unwrap_or_else(DirectStepSummary::continue_pure),
        DirectStepHiddenBoundary::ObjectInit { init, .. } => {
            let mut known_local_metadata = HashMap::new();
            for step in &init.steps {
                match step {
                    hir::ObjectInitStep::PropertyInit { init, .. } => {
                        collect_known_local_metadata_in_expr(init, &mut known_local_metadata);
                    }
                    hir::ObjectInitStep::InitBlock { block } => {
                        collect_known_local_metadata_in_block(block, &mut known_local_metadata);
                    }
                }
            }

            let mut out = DirectStepSummary::continue_pure();
            for step in &init.steps {
                if !out.may_continue {
                    break;
                }
                let step_summary = match step {
                    hir::ObjectInitStep::PropertyInit { init, .. } => summarize_direct_step_expr(
                        types,
                        init,
                        DirectStepMode::OutsideHandle,
                        &known_local_metadata,
                        &next_analysis,
                    ),
                    hir::ObjectInitStep::InitBlock { block } => summarize_direct_step_stmt_seq(
                        types,
                        &block.stmts,
                        DirectStepMode::OutsideHandle,
                        &known_local_metadata,
                        &next_analysis,
                    ),
                };
                out.merge_effects(step_summary.effects);
                out.may_stop |= step_summary.may_stop;
                out.may_continue = step_summary.may_continue;
            }
            out
        }
    }
}

pub fn finalize_boundary_summary_in_mode(
    types: &TypeStore,
    boundary_summary: DirectStepSummary,
    mode: DirectStepMode<'_>,
    analysis: &DirectStepAnalysis<'_>,
) -> DirectStepSummary {
    match mode {
        DirectStepMode::OutsideHandle => boundary_summary,
        DirectStepMode::ActiveHandle(ctx) => {
            let mut out = DirectStepSummary::empty();
            if boundary_summary.may_continue {
                out.may_continue = true;
            }
            if boundary_summary.may_stop {
                out.merge_paths(finalize_handle_terminal(
                    types,
                    ctx,
                    analysis,
                    DirectStepTerminalKind::TerminalStop,
                ));
            }
            if !boundary_summary.effects.is_empty() {
                out.merge_paths(dispatch_boundary_effects_through_active_handle(
                    types,
                    &boundary_summary.effects,
                    ctx,
                    analysis,
                ));
            }
            out
        }
    }
}

pub fn dispatch_boundary_effects_through_active_handle(
    types: &TypeStore,
    effects: &[TypeId],
    ctx: ActiveDirectStepHandleContext<'_>,
    analysis: &DirectStepAnalysis<'_>,
) -> DirectStepSummary {
    let mut out = DirectStepSummary::empty();
    for effect_ty in effects {
        let matching_arms = ctx
            .handle
            .arms
            .iter()
            .filter(|arm| arm.op.effect_ty == *effect_ty)
            .collect::<Vec<_>>();
        if matching_arms.is_empty() {
            out.merge_paths(finalize_handle_outward(
                types,
                ctx,
                analysis,
                vec![*effect_ty],
            ));
            continue;
        }
        for arm in matching_arms {
            out.merge_paths(summarize_direct_step_dispatch_arm(
                types, arm, ctx, analysis,
            ));
        }
    }
    out
}

pub fn summarize_direct_step_dispatch_arm(
    types: &TypeStore,
    arm: &hir::HandleArm,
    ctx: ActiveDirectStepHandleContext<'_>,
    analysis: &DirectStepAnalysis<'_>,
) -> DirectStepSummary {
    let known_local_metadata = collect_known_local_metadata_in_handle_arm(arm);
    let arm_summary = summarize_direct_step_expr(
        types,
        &arm.body,
        DirectStepMode::OutsideHandle,
        &known_local_metadata,
        analysis,
    );

    let mut out = DirectStepSummary::empty();
    if !arm_summary.effects.is_empty() {
        out.merge_paths(finalize_handle_outward(
            types,
            ctx,
            analysis,
            arm_summary.effects.clone(),
        ));
    }
    if arm_summary.may_stop {
        out.merge_paths(finalize_handle_terminal(
            types,
            ctx,
            analysis,
            DirectStepTerminalKind::TerminalStop,
        ));
    }
    if arm_summary.may_continue {
        match arm.kind {
            hir::HandleArmKind::EscapeContinuation { continuation }
                if tail_resume_arm_matches_static(&arm.body, continuation) =>
            {
                out.may_continue = true
            }
            hir::HandleArmKind::NonResuming | hir::HandleArmKind::EscapeContinuation { .. } => {
                out.merge_paths(finalize_handle_terminal(
                    types,
                    ctx,
                    analysis,
                    DirectStepTerminalKind::HandleCompletion,
                ));
            }
        }
    }
    out
}

pub fn finalize_handle_terminal(
    types: &TypeStore,
    ctx: ActiveDirectStepHandleContext<'_>,
    analysis: &DirectStepAnalysis<'_>,
    kind: DirectStepTerminalKind,
) -> DirectStepSummary {
    let cleanup = summarize_direct_step_handle_finally(types, ctx.handle, analysis);
    let mut out = cleanup.without_continue();
    if cleanup.may_continue {
        match kind {
            DirectStepTerminalKind::HandleCompletion => match ctx.role {
                DirectStepHandleRole::ResumeStep => out.may_stop = true,
                DirectStepHandleRole::HandleExpression => out.may_continue = true,
            },
            DirectStepTerminalKind::TerminalStop => out.may_stop = true,
        }
    }
    out
}

pub fn finalize_handle_outward(
    types: &TypeStore,
    ctx: ActiveDirectStepHandleContext<'_>,
    analysis: &DirectStepAnalysis<'_>,
    effects: Vec<TypeId>,
) -> DirectStepSummary {
    let cleanup = summarize_direct_step_handle_finally(types, ctx.handle, analysis);
    let mut out = cleanup.without_continue();
    if cleanup.may_continue {
        out.merge_effects(effects);
    }
    out
}

pub fn summarize_direct_step_handle_finally(
    types: &TypeStore,
    handle: &hir::HandleExpr,
    analysis: &DirectStepAnalysis<'_>,
) -> DirectStepSummary {
    let Some(finally_block) = handle.finally.as_ref() else {
        return DirectStepSummary::continue_pure();
    };
    let mut known_local_metadata = HashMap::new();
    collect_known_local_metadata_in_block(finally_block, &mut known_local_metadata);
    summarize_direct_step_stmt_seq(
        types,
        &finally_block.stmts,
        DirectStepMode::OutsideHandle,
        &known_local_metadata,
        analysis,
    )
}

pub fn classify_direct_step_hidden_boundary_for_value_ref<'a>(
    program: Option<DirectStepProgramInfo<'a>>,
    value_ref: &hir::ValueRef,
) -> Option<DirectStepHiddenBoundary<'a>> {
    let program = program?;
    let hir::ValueRef::TopLevel { fqn, .. } = value_ref else {
        return None;
    };
    if let Some(init) = program.object_inits.get(fqn) {
        return Some(DirectStepHiddenBoundary::ObjectInit {
            fqn: fqn.clone(),
            init,
        });
    }
    program.top_level_immutable_values.get(fqn).map(|value| {
        DirectStepHiddenBoundary::TopLevelImmutable {
            fqn: fqn.clone(),
            value,
        }
    })
}

pub fn classify_direct_step_hidden_boundary_for_member_access<'a>(
    program: Option<DirectStepProgramInfo<'a>>,
    member: &hir::MemberAccess,
) -> Option<DirectStepHiddenBoundary<'a>> {
    let program = program?;
    let hir::MemberRef::Value { fqn, .. } = member.resolved.as_ref()? else {
        return None;
    };
    let (owner_fqn, _) = fqn.rsplit_once('.')?;
    program
        .object_inits
        .get(owner_fqn)
        .map(|init| DirectStepHiddenBoundary::ObjectInit {
            fqn: owner_fqn.to_string(),
            init,
        })
}

pub fn summarize_direct_step_call_args<'a>(
    types: &TypeStore,
    args: impl IntoIterator<Item = &'a hir::CallArg>,
    mode: DirectStepMode<'_>,
    known_local_metadata: &HashMap<hir::SymbolId, KnownLocalMetadata>,
    analysis: &DirectStepAnalysis<'_>,
) -> DirectStepSummary {
    let mut out = DirectStepSummary::continue_pure();
    for arg in args {
        if !out.may_continue {
            break;
        }
        let summary = match arg {
            hir::CallArg::Positional(expr) => {
                summarize_direct_step_expr(types, expr, mode, known_local_metadata, analysis)
            }
            hir::CallArg::Named { value, .. } => {
                summarize_direct_step_expr(types, value, mode, known_local_metadata, analysis)
            }
        };
        out.merge_effects(summary.effects);
        out.may_stop |= summary.may_stop;
        out.may_continue = summary.may_continue;
    }
    out
}

pub fn summarize_direct_step_exprs<'a>(
    types: &TypeStore,
    exprs: impl IntoIterator<Item = &'a hir::Expr>,
    mode: DirectStepMode<'_>,
    known_local_metadata: &HashMap<hir::SymbolId, KnownLocalMetadata>,
    analysis: &DirectStepAnalysis<'_>,
) -> DirectStepSummary {
    let mut out = DirectStepSummary::continue_pure();
    for expr in exprs {
        if !out.may_continue {
            break;
        }
        let summary = summarize_direct_step_expr(types, expr, mode, known_local_metadata, analysis);
        out.merge_effects(summary.effects);
        out.may_stop |= summary.may_stop;
        out.may_continue = summary.may_continue;
    }
    out
}

pub fn direct_effect_terms_from_callable_expr(
    types: &TypeStore,
    callee: &hir::Expr,
    known_local_metadata: &HashMap<hir::SymbolId, KnownLocalMetadata>,
) -> Vec<TypeId> {
    let callee_ty = match types.kind(callee.ty) {
        TypeKind::Ref(RefTypeKind::Function(_)) => callee.ty,
        _ => match &callee.kind {
            hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) => {
                known_local_metadata.get(id).map(|metadata| metadata.ty)
            }
            _ => None,
        }
        .unwrap_or(callee.ty),
    };

    match types.kind(callee_ty) {
        TypeKind::Ref(RefTypeKind::Function(fun_ty)) => fun_ty.effects.terms.clone(),
        _ => Vec::new(),
    }
}

pub fn first_matching_arm_for_direct_perform<'a>(
    handle: &'a hir::HandleExpr,
    op_fqn: &str,
    effect_ty: TypeId,
) -> Option<&'a hir::HandleArm> {
    let mut same_op_fallback = None;
    for arm in &handle.arms {
        if arm.op.op.fqn != op_fqn {
            continue;
        }
        if same_op_fallback.is_none() {
            same_op_fallback = Some(arm);
        }
        if arm.op.effect_ty == effect_ty {
            return Some(arm);
        }
    }
    same_op_fallback
}

pub fn ordinary_callee_resume_slot_type(
    body: &hir::Block,
    source_path: &SuspendSourcePath,
    resume_path: &SuspendResumePath,
    declared_return_ty: TypeId,
    resume_slot: &FrameSlot,
) -> TypeId {
    match resume_path.consumer {
        SuspendResumeConsumer::ExprStmt
            if source_path.frames.is_empty()
                && source_path
                    .handle_body_stmt_idx()
                    .is_some_and(|stmt_idx| stmt_idx + 1 == body.stmts.len()) =>
        {
            declared_return_ty
        }
        SuspendResumeConsumer::ReturnValue
            if source_path.frames.is_empty() && source_path.handle_body_stmt_idx().is_some() =>
        {
            declared_return_ty
        }
        _ => resume_slot.ty(),
    }
}

#[cfg(test)]
mod plan_tests {
    use crate::parser::parse_file;
    use crate::resolve::Index;
    use crate::session::{Session, SessionOptions};
    use crate::source::SourceFile;
    use crate::typecheck;

    use super::*;

    #[test]
    pub fn continuation_escape_facts_enter_handle_planning_input() {
        let source_text = r#"
package a

import scoop.core.*

fun demo(k: Continuation<Int, Int>): Int {
    val result: Int = try {
        k.resume(1)
        11
    } catch (e: RuntimeError) {
        22
    }
    result
}
"#;
        let lowered = lower_typed_single_source(source_text);
        let source = SourceFile::new_virtual("<mem>", source_text);
        let session = test_session();
        let materialized = crate::mir::materialize_for_dump(&session, &source)
            .expect("materialized MIR should be available");
        let pass_view = materialized.pass_view();

        let (fun, handle) = first_handle_in_file(&lowered.file).expect("expected a handle");
        let resume_call_site = lowered
            .continuation_resume_call_sites
            .iter()
            .next()
            .expect("expected a Continuation.resume call site");

        let context_without_facts = collect_effect_analysis_context_for_fun(&lowered, fun);
        assert_eq!(
            context_without_facts.continuation_escape_state_for_call_span(resume_call_site.span),
            ContinuationEscapeState::Unknown,
            "missing MIR escape facts must stay conservative"
        );

        let context =
            collect_effect_analysis_context_for_fun_with_pass_view(&lowered, fun, Some(&pass_view));
        assert_eq!(
            context.continuation_escape_state_for_call_span(resume_call_site.span),
            ContinuationEscapeState::LocalResumeOnly,
            "MIR escape facts should be projected into EffectAnalysisCtx by call site"
        );

        let plan = HandleStateMachinePlan::build_with_context(&lowered.types, handle, &context);
        let resume_site = plan
            .suspend_sites
            .iter()
            .find(|site| site.kind.is_continuation_resume_boundary())
            .expect("Continuation.resume should create a hidden suspend site");
        assert_eq!(
            resume_site.continuation_escape,
            ContinuationEscapeState::LocalResumeOnly,
            "handle planning should record the continuation escape state on the suspend site"
        );
    }

    #[test]
    pub fn escaping_continuation_facts_enter_handle_planning_input() {
        let source_text = r#"
package a

import scoop.core.*

fun consume(k: Continuation<Int, Int>) {}

fun demo(k: Continuation<Int, Int>): Int {
    consume(k)
    val result: Int = try {
        k.resume(1)
        11
    } catch (e: RuntimeError) {
        22
    }
    result
}
"#;
        let lowered = lower_typed_single_source(source_text);
        let source = SourceFile::new_virtual("<mem>", source_text);
        let session = test_session();
        let materialized = crate::mir::materialize_for_dump(&session, &source)
            .expect("materialized MIR should be available");
        let pass_view = materialized.pass_view();

        let (fun, handle) = first_handle_in_file(&lowered.file).expect("expected a handle");
        let resume_call_site = lowered
            .continuation_resume_call_sites
            .iter()
            .next()
            .expect("expected a Continuation.resume call site");
        let context =
            collect_effect_analysis_context_for_fun_with_pass_view(&lowered, fun, Some(&pass_view));
        assert_eq!(
            context.continuation_escape_state_for_call_span(resume_call_site.span),
            ContinuationEscapeState::Escaping,
            "a continuation passed across a call boundary should project as escaping"
        );

        let plan = HandleStateMachinePlan::build_with_context(&lowered.types, handle, &context);
        let resume_site = plan
            .suspend_sites
            .iter()
            .find(|site| site.kind.is_continuation_resume_boundary())
            .expect("Continuation.resume should create a hidden suspend site");
        assert_eq!(
            resume_site.continuation_escape,
            ContinuationEscapeState::Escaping,
            "handle planning should retain escaping continuation facts"
        );
    }

    #[test]
    pub fn non_tail_escape_arm_with_outward_suspend_builds_inner_resume_site() {
        let lowered = lower_typed_single_source(
            r#"
package a

import scoop.core.*

effect Ask {
    fun current(): Int
}

effect Inner {
    fun enter(): Int
}

effect Boom {
    fun next(): Int
}

class Cell(var saved: Continuation<Int, Int, eff Boom>?, var total: Int)

fun start(cell: Cell): Int / Boom {
    return handle {
        val seed: Int = Ask.current()
        val nested: Int = handle {
            val x: Int = Inner.enter()
            val y: Int = Boom.next()
            x + y
        } with {
            Inner.enter(), k -> {
                val resumed: Int = try {
                    k.resume(7)
                } catch (e: RuntimeError) {
                    0
                }
                resumed + 1
            }
        }
        cell.total = seed + nested
        seed + nested
    } with {
        Ask.current(), k -> {
            cell.saved = Some(k)
            0 - 1
        }
    }
}
"#,
        );

        let (fun, handle) = first_handle_in_file(&lowered.file).expect("expected outer handle");
        let context = collect_effect_analysis_context_for_fun_with_pass_view(&lowered, fun, None);
        let plan = HandleStateMachinePlan::build_with_context(&lowered.types, handle, &context);
        fn has_resume_site(plan: &HandleStateMachinePlan) -> bool {
            plan.suspend_sites
                .iter()
                .any(|site| site.kind.is_continuation_resume_boundary())
                || plan.nested_handles.iter().any(has_resume_site)
        }

        let nested = plan
            .nested_handles
            .first()
            .expect("expected nested handle plan inside start");
        assert!(
            has_resume_site(nested),
            "inner handle arm body should materialize a first-class Continuation.resume suspend site instead of staying opaque"
        );
    }

    #[test]
    pub fn non_tail_escape_arm_nested_handle_boundary_escape_replay_keeps_arm_tail() {
        let source_text = r#"
package a

import scoop.core.*

effect Inner {
    fun enter(): Int
}

effect Boom {
    fun next(): Int
}

class Cell(var saved: Continuation<Int, Int>?)

fun demo(cell: Cell): Int {
    return handle {
        val nested: Int = handle {
            val x: Int = Inner.enter()
            val y: Int = Boom.next()
            x + y
        } with {
            Inner.enter(), k -> {
                val resumed: Int = try {
                    k.resume(7)
                } catch (e: RuntimeError) {
                    0
                }
                println("inner_arm_after_resume")
                resumed + 1
            }
        }
        println("after_nested")
        println(nested)
        nested
    } with {
        Boom.next(), k -> {
            cell.saved = Some(k)
            18
        }
    }
}
"#;
        let source = SourceFile::new_virtual("<mem>", source_text);
        let lowered = lower_typed_single_source(source_text);
        let (fun, handle) = first_handle_in_file(&lowered.file).expect("expected outer handle");
        let context = collect_effect_analysis_context_for_fun_with_pass_view(&lowered, fun, None);
        let plan = HandleStateMachinePlan::build_with_context(&lowered.types, handle, &context);
        let inner = plan
            .nested_handles
            .first()
            .expect("expected nested handle plan for Inner.enter arm");

        let boundary_site = inner
            .suspend_sites
            .iter()
            .find(|site| {
                matches!(site.kind, SuspendSiteKind::NestedHandleBoundary { .. })
                    && site
                        .source_path
                        .as_ref()
                        .is_some_and(|path| path.label().starts_with("arm#0"))
            })
            .expect("arm-body try/catch boundary should keep an arm-rooted source path");

        assert_eq!(
            boundary_site
                .source_path
                .as_ref()
                .expect("source path should exist")
                .label(),
            "arm#0 -> block[0]",
            "nested-handle boundary inside escape arm should be rooted at the arm body instead of falling back to opaque top-level replay"
        );

        let replay_state = inner
            .states
            .iter()
            .find(|state| state.id == boundary_site.resume_target)
            .expect("nested-handle boundary resume target should exist");
        let replay_snippets = replay_state
            .actions
            .iter()
            .filter_map(|op| state_action_source_snippet(op, &source))
            .collect::<Vec<_>>();

        assert!(
            replay_snippets
                .iter()
                .any(|snippet| snippet.contains("inner_arm_after_resume")),
            "nested-handle boundary resume fragment should keep the arm-body post-resume print instead of stopping at the inner try/nested-handle result: {replay_snippets:#?}"
        );
        assert!(
            replay_snippets
                .iter()
                .any(|snippet| snippet.contains("resumed + 1")),
            "nested-handle boundary resume fragment should keep the arm tail expression after nested-handle replay: {replay_snippets:#?}"
        );
    }

    #[test]
    pub fn direct_step_effect_rows_include_direct_effectful_call_after_escape_site() {
        let lowered = lower_typed_single_source(
            r#"
package a

import scoop.core.*

effect Ask {
    fun current(): Int
}

effect Boom {
    fun boom(code: Int): Int
}

fun demo(): Int / (Boom) {
    return handle {
        val burst: (Int) -> Int / (Boom) = { seed: Int ->
            Boom.boom(seed)
        }
        val value: Int = Ask.current()
        burst(value)
    } with {
        Ask.current(), k -> 7
    }
}
"#,
        );
        let handle = first_handle_in_file(&lowered.file)
            .map(|(_, handle)| handle)
            .expect("expected a handle");
        let continuation = only_escape_continuation_symbol(handle);

        let rows =
            compute_escape_continuation_direct_step_effect_rows_for_handle(&lowered.types, handle);
        let row = rows
            .get(&continuation)
            .expect("expected a direct-step effect row for escape continuation binder");

        assert_eq!(effect_row_terms(&lowered.types, row), ["a.Boom"]);
    }

    #[test]
    pub fn direct_step_rows_stop_at_next_escape_boundary() {
        let lowered = lower_typed_single_source(
            r#"
package a

import scoop.core.*

effect Ask {
    fun current(): Int
}

effect Boom {
    fun boom(code: Int): Int
}

fun demo(): Int / (Boom) {
    return handle {
        val first: Int = Ask.current()
        val second: Int = Ask.current()
        Boom.boom(second)
    } with {
        Ask.current(), k -> 7
    }
}
"#,
        );
        let handle = first_handle_in_file(&lowered.file)
            .map(|(_, handle)| handle)
            .expect("expected a handle");
        let continuation = only_escape_continuation_symbol(handle);

        let mut rows =
            compute_escape_continuation_direct_step_rows_by_site(&lowered.types, handle, None);
        rows.sort_by_key(|row| row.site_id);

        assert_eq!(
            rows.len(),
            2,
            "expected two handled Ask.current escape sites"
        );
        assert_eq!(rows[0].continuation, continuation);
        assert_eq!(rows[1].continuation, continuation);
        assert!(
            rows[0].effects.is_pure(),
            "first site should stop before the second escape boundary, found {:?}",
            effect_row_terms(&lowered.types, &rows[0].effects)
        );
        assert_eq!(
            effect_row_terms(&lowered.types, &rows[1].effects),
            ["a.Boom"]
        );
    }

    #[test]
    pub fn direct_step_rows_include_immediate_resume_arm_body_effects() {
        let lowered = lower_typed_single_source(
            r#"
package a

import scoop.core.*

effect Ask {
    fun current(): Int
}

effect Yield {
    fun next(): Int
}

effect Boom {
    fun boom(code: Int): Int
}

fun demo(): Int / (Boom) {
    return handle {
        val first: Int = Ask.current()
        val second: Int = Yield.next()
        first + second
    } with {
        Ask.current(), k -> 7
        Yield.next() , k -> {
            val _: Int = Boom.boom(41)
            k.resume(3)
        }
    }
}
"#,
        );
        let handle = first_handle_in_file(&lowered.file)
            .map(|(_, handle)| handle)
            .expect("expected a handle");
        let continuation = only_escape_continuation_symbol(handle);

        let rows =
            compute_escape_continuation_direct_step_effect_rows_for_handle(&lowered.types, handle);
        let row = rows
            .get(&continuation)
            .expect("expected a direct-step effect row for escape continuation binder");

        assert_eq!(effect_row_terms(&lowered.types, row), ["a.Boom"]);
    }

    #[test]
    pub fn direct_step_rows_include_escape_arm_body_effects_at_next_boundary() {
        let lowered = lower_typed_single_source(
            r#"
package a

import scoop.core.*

effect Ask {
    fun current(): Int
}

effect Boom {
    fun boom(code: Int): Int
}

fun demo(): Int / (Boom) {
    return handle {
        val first: Int = Ask.current()
        val second: Int = Ask.current()
        first + second
    } with {
        Ask.current(), k -> {
            val _: Int = Boom.boom(9)
            7
        }
    }
}
"#,
        );
        let handle = first_handle_in_file(&lowered.file)
            .map(|(_, handle)| handle)
            .expect("expected a handle");
        let mut rows =
            compute_escape_continuation_direct_step_rows_by_site(&lowered.types, handle, None);
        rows.sort_by_key(|row| row.site_id);

        assert_eq!(
            rows.len(),
            2,
            "expected two handled Ask.current escape sites"
        );
        assert_eq!(
            effect_row_terms(&lowered.types, &rows[0].effects),
            ["a.Boom"]
        );
        assert!(
            rows[1].effects.is_pure(),
            "second site should not count its own arm body as resumed tail, found {:?}",
            effect_row_terms(&lowered.types, &rows[1].effects)
        );
    }

    #[test]
    pub fn direct_step_rows_include_finally_effects_after_resumed_tail_completion() {
        let lowered = lower_typed_single_source(
            r#"
package a

import scoop.core.*

effect Ask {
    fun current(): Int
}

effect Boom {
    fun boom(code: Int): Int
}

fun demo(): Int / (Boom) {
    return handle {
        val value: Int = Ask.current()
        value + 1
    } with {
        Ask.current(), k -> 7
    } finally {
        val _: Int = Boom.boom(5)
    }
}
"#,
        );
        let handle = first_handle_in_file(&lowered.file)
            .map(|(_, handle)| handle)
            .expect("expected a handle");
        let continuation = only_escape_continuation_symbol(handle);

        let rows =
            compute_escape_continuation_direct_step_effect_rows_for_handle(&lowered.types, handle);
        let row = rows
            .get(&continuation)
            .expect("expected a direct-step effect row for escape continuation binder");

        assert_eq!(effect_row_terms(&lowered.types, row), ["a.Boom"]);
    }

    #[test]
    pub fn direct_step_rows_include_nested_handle_boundary_effects() {
        let lowered = lower_typed_single_source(
            r#"
package a

import scoop.core.*

effect Ask {
    fun current(): Int
}

effect Yield {
    fun next(): Int
}

effect Boom {
    fun boom(code: Int): Int
}

fun demo(): Int / (Boom) {
    return handle {
        val seed: Int = Ask.current()
        val nested: Int = handle {
            Yield.next()
        } with {
            Raise.raise(err: RuntimeError) -> 0
        }
        seed + nested
    } with {
        Ask.current(), k -> 7
        Yield.next() , k -> {
            val _: Int = Boom.boom(11)
            k.resume(5)
        }
    }
}
"#,
        );
        let handle = first_handle_in_file(&lowered.file)
            .map(|(_, handle)| handle)
            .expect("expected a handle");
        let continuation = only_escape_continuation_symbol(handle);

        let rows =
            compute_escape_continuation_direct_step_effect_rows_for_handle(&lowered.types, handle);
        let row = rows
            .get(&continuation)
            .expect("expected a direct-step effect row for escape continuation binder");

        assert_eq!(effect_row_terms(&lowered.types, row), ["a.Boom"]);
    }

    pub fn effect_row_terms(types: &TypeStore, row: &EffectRow) -> Vec<String> {
        row.terms
            .iter()
            .map(|ty| types.display(*ty).to_string())
            .collect()
    }

    pub fn direct_step_program_info(lowered: &hir::LoweredHir) -> DirectStepProgramInfo<'_> {
        DirectStepProgramInfo {
            object_inits: &lowered.object_inits,
            top_level_immutable_values: &lowered.top_level_immutable_values,
        }
    }

    pub fn only_escape_continuation_symbol(handle: &hir::HandleExpr) -> hir::SymbolId {
        handle
            .arms
            .iter()
            .find_map(|arm| match arm.kind {
                hir::HandleArmKind::EscapeContinuation { continuation } => Some(continuation),
                hir::HandleArmKind::NonResuming => None,
            })
            .expect("expected an escape continuation arm")
    }

    pub fn state_action_source_snippet(op: &HandleStateOp, source: &SourceFile) -> Option<String> {
        match op {
            HandleStateOp::StmtEmpty { stmt }
            | HandleStateOp::Assign { stmt }
            | HandleStateOp::Break { stmt }
            | HandleStateOp::Continue { stmt }
            | HandleStateOp::Return { stmt }
            | HandleStateOp::TodoStmt { stmt, .. }
            | HandleStateOp::WhileCondHeader { stmt } => Some(source.slice(stmt.span).to_string()),
            HandleStateOp::BindLocal { decl, .. }
            | HandleStateOp::DeclareAnonymousVal { decl, .. } => decl
                .init
                .as_ref()
                .map(|init| source.slice(init.span).to_string()),
            HandleStateOp::ExprMissing { expr }
            | HandleStateOp::Literal { expr }
            | HandleStateOp::ReadLocal { expr, .. }
            | HandleStateOp::ObjectInitAccessBoundary { expr, .. }
            | HandleStateOp::VarRef { expr }
            | HandleStateOp::StructLit { expr }
            | HandleStateOp::TupleLit { expr }
            | HandleStateOp::InterpolatedString { expr }
            | HandleStateOp::Expr { expr }
            | HandleStateOp::RuntimeRaiseBoundary { expr, .. }
            | HandleStateOp::BinaryExpr { expr }
            | HandleStateOp::WhenExpr { expr }
            | HandleStateOp::SuspendCall { expr, .. }
            | HandleStateOp::Call { expr }
            | HandleStateOp::Perform { expr, .. }
            | HandleStateOp::NestedHandleBoundary { expr, .. }
            | HandleStateOp::NestedHandle { expr, .. }
            | HandleStateOp::Closure { expr }
            | HandleStateOp::TodoExpr { expr, .. } => Some(source.slice(expr.span).to_string()),
            HandleStateOp::ResumeAfterSite { source_span, .. } => {
                Some(source.slice(*source_span).to_string())
            }
            HandleStateOp::ImplicitElseUnit { span } => Some(source.slice(*span).to_string()),
            HandleStateOp::CleanupEdgeComplete
            | HandleStateOp::ReturnToEnclosingExpression
            | HandleStateOp::LoopReentry { .. }
            | HandleStateOp::ExecuteArmBody { .. } => None,
        }
    }

    pub fn lower_typed_single_source(source_text: &str) -> hir::LoweredHir {
        let session = test_session();
        let source = SourceFile::new_virtual("<mem>", source_text);
        let mut ast = parse_file(&source).expect("parse");

        let index = {
            let mut pairs = Vec::new();
            for file in session.sysroot().index_files() {
                pairs.push((&file.source, &file.ast));
            }
            pairs.push((&source, &ast));
            Index::build(&pairs).expect("index")
        };

        let headers =
            crate::resolve::check_file_headers(&source, &ast, &index).expect("resolve headers");
        crate::resolve::check_file_bodies(&source, &mut ast, &index, &headers)
            .expect("resolve bodies");

        let mut typecheck_types = TypeStore::new();
        let builtins = typecheck_types.intern_builtins();
        let mut env = typecheck::TypeEnv::from_sysroot(session.sysroot(), &index).expect("env");
        env.extend_from_file(&source, &ast, &index)
            .expect("extend type env");

        typecheck::check_file_annotations(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut typecheck_types,
            builtins,
        )
        .expect("check annotations");
        typecheck::check_file_type_refs(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut typecheck_types,
            builtins,
        )
        .expect("check type refs");
        typecheck::check_file_exprs(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut typecheck_types,
            builtins,
        )
        .expect("check exprs");

        let mut unit = Vec::new();
        for file in session.sysroot().index_files() {
            unit.push((&file.source, &file.ast));
        }
        unit.push((&source, &ast));

        hir::lower_for_compilation_unit_multi_files(
            &source,
            &index,
            &unit,
            &[(&source, &ast)],
            &[],
            &typecheck_types,
        )
        .expect("lower")
    }

    pub fn test_session() -> Session {
        Session::with_options(SessionOptions::new()).expect("session")
    }

    pub fn first_handle_in_file(file: &hir::File) -> Option<(&hir::FunDecl, &hir::HandleExpr)> {
        for item in &file.items {
            if let hir::Item::Fun(fun) = item
                && let Some(body) = &fun.body
                && let Some(handle) = first_handle_in_block(body)
            {
                return Some((fun, handle));
            }
        }
        None
    }

    pub fn first_handle_in_block(block: &hir::Block) -> Option<&hir::HandleExpr> {
        for stmt in &block.stmts {
            if let Some(handle) = first_handle_in_stmt(stmt) {
                return Some(handle);
            }
        }
        None
    }

    pub fn first_handle_in_stmt(stmt: &hir::Stmt) -> Option<&hir::HandleExpr> {
        match &stmt.kind {
            hir::StmtKind::Expr(expr) => first_handle_in_expr(expr),
            hir::StmtKind::Val(decl) => decl.init.as_ref().and_then(first_handle_in_expr),
            hir::StmtKind::Assign { lhs, rhs, .. } => {
                first_handle_in_expr(lhs).or_else(|| first_handle_in_expr(rhs))
            }
            hir::StmtKind::While { cond, body } => {
                first_handle_in_expr(cond).or_else(|| first_handle_in_block(body))
            }
            hir::StmtKind::Return { value } => value.as_ref().and_then(first_handle_in_expr),
            hir::StmtKind::Empty
            | hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Todo(_) => None,
        }
    }

    pub fn first_handle_in_expr(expr: &hir::Expr) -> Option<&hir::HandleExpr> {
        match &expr.kind {
            hir::ExprKind::Handle(handle) => Some(handle),
            hir::ExprKind::Block(block) => first_handle_in_block(block),
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => first_handle_in_expr(cond)
                .or_else(|| first_handle_in_expr(then_branch))
                .or_else(|| else_branch.as_deref().and_then(first_handle_in_expr)),
            hir::ExprKind::When { subject, arms } => first_handle_in_expr(subject).or_else(|| {
                arms.iter()
                    .find_map(|arm| arm.guard.as_ref().and_then(first_handle_in_expr))
                    .or_else(|| arms.iter().find_map(|arm| first_handle_in_expr(&arm.body)))
            }),
            hir::ExprKind::Call { callee, args } => first_handle_in_expr(callee).or_else(|| {
                args.iter().find_map(|arg| match arg {
                    hir::CallArg::Positional(expr) => first_handle_in_expr(expr),
                    hir::CallArg::Named { value, .. } => first_handle_in_expr(value),
                })
            }),
            hir::ExprKind::StructLit { fields, .. } => fields
                .iter()
                .find_map(|field| first_handle_in_expr(&field.value)),
            hir::ExprKind::TupleLit { elements } => elements.iter().find_map(first_handle_in_expr),
            hir::ExprKind::InterpolatedString { parts, .. } => parts.iter().find_map(|part| {
                let hir::InterpolatedStringPart::Expr { expr } = part else {
                    return None;
                };
                first_handle_in_expr(expr)
            }),
            hir::ExprKind::Unary { expr: inner, .. }
            | hir::ExprKind::Cast { expr: inner, .. }
            | hir::ExprKind::TypeCheck { expr: inner, .. }
            | hir::ExprKind::MemberAccess {
                receiver: inner, ..
            } => first_handle_in_expr(inner),
            hir::ExprKind::Binary { lhs, rhs, .. } => {
                first_handle_in_expr(lhs).or_else(|| first_handle_in_expr(rhs))
            }
            hir::ExprKind::Closure(closure) => first_handle_in_expr(&closure.body),
            hir::ExprKind::Perform { args, .. } => args.iter().find_map(|arg| match arg {
                hir::CallArg::Positional(expr) => first_handle_in_expr(expr),
                hir::CallArg::Named { value, .. } => first_handle_in_expr(value),
            }),
            hir::ExprKind::Missing
            | hir::ExprKind::Literal(_)
            | hir::ExprKind::VarRef(_)
            | hir::ExprKind::UnresolvedIdent { .. }
            | hir::ExprKind::ClassLiteral(_)
            | hir::ExprKind::Todo(_) => None,
        }
    }
}
