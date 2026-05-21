//!  SuspendCallAnalysis and helpers that classify whether function calls may suspend.

#![allow(dead_code)]

use super::*;

pub(crate) fn function_ty_declared_effectful(types: &TypeStore, ty: TypeId) -> bool {
    matches!(
        types.kind(ty),
        TypeKind::Ref(RefTypeKind::Function(fun_ty)) if !fun_ty.effects.is_pure()
    )
}

pub(crate) fn function_ty_may_suspend(types: &TypeStore, ty: TypeId) -> bool {
    function_ty_declared_effectful(types, ty)
}

pub(crate) fn hir_ty_is_function_value(types: &TypeStore, ty: TypeId) -> bool {
    matches!(types.kind(ty), TypeKind::Ref(RefTypeKind::Function(_)))
}

pub(crate) struct SuspendCallAnalysis<'a> {
    pub(crate) types: &'a TypeStore,
    pub(crate) context: &'a EffectAnalysisCtx,
}

impl<'a> SuspendCallAnalysis<'a> {
    pub(crate) fn call_site(&self, span: Span) -> hir::CallSite {
        self.context.call_site(span)
    }

    pub(crate) fn resolve_expr_concrete_type(&self, expr: &hir::Expr) -> Option<TypeId> {
        ExprFactResolver::new(self.types, self.context.hir_facts.as_ref(), |id| {
            self.context
                .known_local_metadata
                .get(&id)
                .map(|metadata| metadata.ty)
        })
        .resolve_expr_concrete_type(expr)
    }

    pub(crate) fn block_may_suspend(
        &self,
        block: &hir::Block,
        seed_locals: &HashMap<hir::SymbolId, bool>,
    ) -> bool {
        let known_locals = self.solve_local_fun_effects_in_block(block, seed_locals);
        self.block_may_suspend_with_locals(block, &known_locals)
    }

    pub(crate) fn solve_local_fun_effects_in_block(
        &self,
        block: &hir::Block,
        seed_locals: &HashMap<hir::SymbolId, bool>,
    ) -> HashMap<hir::SymbolId, bool> {
        let mut known_locals = seed_locals.clone();
        loop {
            let before = known_locals.clone();
            self.collect_local_fun_effects_in_block(block, &mut known_locals);
            if known_locals == before {
                break;
            }
        }
        known_locals
    }

    pub(crate) fn collect_local_fun_effects_in_block(
        &self,
        block: &hir::Block,
        out: &mut HashMap<hir::SymbolId, bool>,
    ) {
        for stmt in &block.stmts {
            self.collect_local_fun_effects_in_stmt(stmt, out);
        }
    }

    pub(crate) fn collect_local_fun_effects_in_stmt(
        &self,
        stmt: &hir::Stmt,
        out: &mut HashMap<hir::SymbolId, bool>,
    ) {
        match &stmt.kind {
            hir::StmtKind::Empty
            | hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Todo(_) => {}
            hir::StmtKind::Expr(expr) => self.collect_local_fun_effects_in_expr(expr, out),
            hir::StmtKind::Val(decl) => {
                if let Some(init) = decl.init.as_ref() {
                    self.collect_local_fun_effects_in_expr(init, out);
                }
                if let Some(id) = decl.id
                    && hir_ty_is_function_value(self.types, decl.ty)
                {
                    let may_suspend = decl.init.as_ref().map_or_else(
                        || function_ty_declared_effectful(self.types, decl.ty),
                        |expr| self.function_value_may_suspend_when_called(expr, out),
                    );
                    out.insert(id, may_suspend);
                }
            }
            hir::StmtKind::Assign { lhs, rhs, .. } => {
                self.collect_local_fun_effects_in_expr(lhs, out);
                self.collect_local_fun_effects_in_expr(rhs, out);
                if let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &lhs.kind
                    && (hir_ty_is_function_value(self.types, lhs.ty)
                        || hir_ty_is_function_value(self.types, rhs.ty)
                        || out.contains_key(id))
                {
                    let may_suspend = self.function_value_may_suspend_when_called(rhs, out);
                    let entry = out.entry(*id).or_insert(false);
                    *entry |= may_suspend;
                }
            }
            hir::StmtKind::While { cond, body } => {
                self.collect_local_fun_effects_in_expr(cond, out);
                self.collect_local_fun_effects_in_block(body, out);
            }
            hir::StmtKind::Return { value } => {
                if let Some(expr) = value {
                    self.collect_local_fun_effects_in_expr(expr, out);
                }
            }
        }
    }

    pub(crate) fn collect_local_fun_effects_in_expr(
        &self,
        expr: &hir::Expr,
        out: &mut HashMap<hir::SymbolId, bool>,
    ) {
        match &expr.kind {
            hir::ExprKind::Missing
            | hir::ExprKind::Literal(_)
            | hir::ExprKind::VarRef(_)
            | hir::ExprKind::UnresolvedIdent { .. }
            | hir::ExprKind::ClassLiteral(_)
            | hir::ExprKind::Todo(_) => {}
            hir::ExprKind::Block(block) => self.collect_local_fun_effects_in_block(block, out),
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.collect_local_fun_effects_in_expr(cond, out);
                self.collect_local_fun_effects_in_expr(then_branch, out);
                if let Some(else_branch) = else_branch.as_deref() {
                    self.collect_local_fun_effects_in_expr(else_branch, out);
                }
            }
            hir::ExprKind::When { subject, arms } => {
                self.collect_local_fun_effects_in_expr(subject, out);
                for arm in arms {
                    if let Some(guard) = arm.guard.as_ref() {
                        self.collect_local_fun_effects_in_expr(guard, out);
                    }
                    self.collect_local_fun_effects_in_expr(&arm.body, out);
                }
            }
            hir::ExprKind::Call { callee, args } => {
                self.collect_local_fun_effects_in_expr(callee, out);
                for arg in args {
                    match arg {
                        hir::CallArg::Positional(expr) => {
                            self.collect_local_fun_effects_in_expr(expr, out)
                        }
                        hir::CallArg::Named { value, .. } => {
                            self.collect_local_fun_effects_in_expr(value, out)
                        }
                    }
                }
            }
            hir::ExprKind::StructLit { fields, .. } => {
                for field in fields {
                    self.collect_local_fun_effects_in_expr(&field.value, out);
                }
            }
            hir::ExprKind::TupleLit { elements } => {
                for element in elements {
                    self.collect_local_fun_effects_in_expr(element, out);
                }
            }
            hir::ExprKind::InterpolatedString { parts, .. } => {
                for part in parts {
                    if let hir::InterpolatedStringPart::Expr { expr } = part {
                        self.collect_local_fun_effects_in_expr(expr, out);
                    }
                }
            }
            hir::ExprKind::Unary { expr: inner, .. }
            | hir::ExprKind::Cast { expr: inner, .. }
            | hir::ExprKind::TypeCheck { expr: inner, .. }
            | hir::ExprKind::MemberAccess {
                receiver: inner, ..
            } => self.collect_local_fun_effects_in_expr(inner, out),
            hir::ExprKind::Binary { lhs, rhs, .. } => {
                self.collect_local_fun_effects_in_expr(lhs, out);
                self.collect_local_fun_effects_in_expr(rhs, out);
            }
            hir::ExprKind::Closure(closure) => {
                self.collect_local_fun_effects_in_expr(&closure.body, out);
            }
            hir::ExprKind::Perform { args, .. } => {
                for arg in args {
                    match arg {
                        hir::CallArg::Positional(expr) => {
                            self.collect_local_fun_effects_in_expr(expr, out)
                        }
                        hir::CallArg::Named { value, .. } => {
                            self.collect_local_fun_effects_in_expr(value, out)
                        }
                    }
                }
            }
            hir::ExprKind::Handle(handle) => {
                self.collect_local_fun_effects_in_block(&handle.body, out);
                for arm in &handle.arms {
                    self.collect_local_fun_effects_in_expr(&arm.body, out);
                }
                if let Some(finally_block) = &handle.finally {
                    self.collect_local_fun_effects_in_block(finally_block, out);
                }
            }
        }
    }

    pub(crate) fn block_may_suspend_with_locals(
        &self,
        block: &hir::Block,
        known_locals: &HashMap<hir::SymbolId, bool>,
    ) -> bool {
        block
            .stmts
            .iter()
            .any(|stmt| self.stmt_may_suspend(stmt, known_locals))
    }

    pub(crate) fn stmt_may_suspend(
        &self,
        stmt: &hir::Stmt,
        known_locals: &HashMap<hir::SymbolId, bool>,
    ) -> bool {
        match &stmt.kind {
            hir::StmtKind::Empty
            | hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Todo(_) => false,
            hir::StmtKind::Expr(expr) => self.expr_may_suspend(expr, known_locals),
            hir::StmtKind::Val(decl) => decl
                .init
                .as_ref()
                .is_some_and(|expr| self.expr_may_suspend(expr, known_locals)),
            hir::StmtKind::Assign { lhs, rhs, .. } => {
                self.expr_may_suspend(lhs, known_locals) || self.expr_may_suspend(rhs, known_locals)
            }
            hir::StmtKind::While { cond, body } => {
                self.expr_may_suspend(cond, known_locals)
                    || self.block_may_suspend(body, known_locals)
            }
            hir::StmtKind::Return { value } => value
                .as_ref()
                .is_some_and(|expr| self.expr_may_suspend(expr, known_locals)),
        }
    }

    pub(crate) fn expr_may_suspend(
        &self,
        expr: &hir::Expr,
        known_locals: &HashMap<hir::SymbolId, bool>,
    ) -> bool {
        match &expr.kind {
            hir::ExprKind::Missing
            | hir::ExprKind::Literal(_)
            | hir::ExprKind::UnresolvedIdent { .. }
            | hir::ExprKind::ClassLiteral(_)
            | hir::ExprKind::Closure(_)
            | hir::ExprKind::Todo(_) => false,
            hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) => {
                let facts = HirFactResolver::new(self.types, self.context.hir_facts.as_ref());
                facts.is_object_value_fqn(fqn) || facts.is_top_level_immutable_value_fqn(fqn)
            }
            hir::ExprKind::VarRef(hir::ValueRef::Local { .. }) => false,
            hir::ExprKind::StructLit { fields, .. } => fields
                .iter()
                .any(|field| self.expr_may_suspend(&field.value, known_locals)),
            hir::ExprKind::TupleLit { elements } => elements
                .iter()
                .any(|element| self.expr_may_suspend(element, known_locals)),
            hir::ExprKind::InterpolatedString { parts, .. } => parts.iter().any(|part| {
                matches!(
                    part,
                    hir::InterpolatedStringPart::Expr { expr }
                        if self.expr_may_suspend(expr, known_locals)
                )
            }),
            hir::ExprKind::Unary { expr: inner, .. }
            | hir::ExprKind::TypeCheck { expr: inner, .. } => {
                self.expr_may_suspend(inner, known_locals)
            }
            hir::ExprKind::Cast {
                expr: inner, op, ..
            } => matches!(op, ast::CastOp::As) || self.expr_may_suspend(inner, known_locals),
            hir::ExprKind::Block(block) => self.block_may_suspend(block, known_locals),
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.expr_may_suspend(cond, known_locals)
                    || self.expr_may_suspend(then_branch, known_locals)
                    || else_branch
                        .as_deref()
                        .is_some_and(|expr| self.expr_may_suspend(expr, known_locals))
            }
            hir::ExprKind::When { subject, arms } => {
                self.expr_may_suspend(subject, known_locals)
                    || arms.iter().any(|arm| {
                        arm.guard
                            .as_ref()
                            .is_some_and(|guard| self.expr_may_suspend(guard, known_locals))
                            || self.expr_may_suspend(&arm.body, known_locals)
                    })
            }
            hir::ExprKind::MemberAccess { receiver, member } => {
                self.expr_may_suspend(receiver, known_locals)
                    || matches!(
                        member.resolved.as_ref(),
                        Some(hir::MemberRef::Value { fqn, .. })
                            if {
                                let facts = HirFactResolver::new(self.types, self.context.hir_facts.as_ref());
                                facts.is_object_value_fqn(fqn) || facts.is_object_property_fqn(fqn)
                            }
                    )
            }
            hir::ExprKind::Binary { lhs, rhs, .. } => {
                self.expr_may_suspend(lhs, known_locals) || self.expr_may_suspend(rhs, known_locals)
            }
            hir::ExprKind::Call { callee, args } => {
                self.context
                    .hir_facts
                    .source_sites
                    .has_continuation_resume(self.context.current_source_path(), expr.span)
                    || self
                        .context
                        .hir_facts
                        .source_sites
                        .constructor_call(self.context.current_source_path(), expr.span)
                        .is_some()
                    || self.function_value_may_suspend_when_called(callee, known_locals)
                    || self.expr_may_suspend(callee, known_locals)
                    || args.iter().any(|arg| match arg {
                        hir::CallArg::Positional(expr) => self.expr_may_suspend(expr, known_locals),
                        hir::CallArg::Named { value, .. } => {
                            self.expr_may_suspend(value, known_locals)
                        }
                    })
            }
            hir::ExprKind::Perform { .. } => true,
            hir::ExprKind::Handle(handle) => self.handle_may_suspend_outward(handle, known_locals),
        }
    }

    pub(crate) fn handle_may_suspend_outward(
        &self,
        handle: &hir::HandleExpr,
        known_locals: &HashMap<hir::SymbolId, bool>,
    ) -> bool {
        let mut known_local_metadata = self.context.known_local_metadata.clone();
        collect_known_local_metadata_in_handle(handle, &mut known_local_metadata);
        let context = HandlePlanContext::new(
            self.context.known_fun_effects.clone(),
            known_locals.clone(),
            known_local_metadata,
            self.context.current_source_path().to_path_buf(),
            Rc::clone(&self.context.hir_facts),
        )
        .with_continuation_escape_facts(self.context.continuation_escape_facts().clone());

        HandleStateMachinePlan::build_with_context(self.types, handle, &context)
            .may_suspend_outward()
    }

    pub(crate) fn function_value_may_suspend_when_called(
        &self,
        expr: &hir::Expr,
        known_locals: &HashMap<hir::SymbolId, bool>,
    ) -> bool {
        let declared_effectful = self
            .resolve_expr_concrete_type(expr)
            .is_some_and(|ty| function_ty_declared_effectful(self.types, ty));
        match &expr.kind {
            hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) => self
                .context
                .known_fun_effects
                .get(fqn)
                .copied()
                .unwrap_or(declared_effectful),
            hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) => {
                known_locals.get(id).copied().unwrap_or(declared_effectful)
            }
            hir::ExprKind::MemberAccess { member, .. } => match member.resolved.as_ref() {
                Some(hir::MemberRef::Fun { fqn, .. })
                | Some(hir::MemberRef::ExtensionFun { fqn, .. }) => self
                    .context
                    .known_fun_effects
                    .get(fqn)
                    .copied()
                    .unwrap_or(declared_effectful),
                _ => declared_effectful,
            },
            hir::ExprKind::Closure(closure) => {
                let mut seed_locals = known_locals.clone();
                for param in &closure.params {
                    seed_locals.insert(
                        param.id,
                        function_ty_declared_effectful(self.types, param.ty),
                    );
                }
                self.expr_may_suspend(&closure.body, &seed_locals)
            }
            hir::ExprKind::Block(block) => block
                .stmts
                .last()
                .and_then(|stmt| match &stmt.kind {
                    hir::StmtKind::Expr(expr) => Some(expr),
                    _ => None,
                })
                .is_some_and(|expr| {
                    self.function_value_may_suspend_when_called(expr, known_locals)
                }),
            hir::ExprKind::If {
                then_branch,
                else_branch,
                ..
            } => {
                self.function_value_may_suspend_when_called(then_branch, known_locals)
                    || else_branch.as_deref().is_some_and(|expr| {
                        self.function_value_may_suspend_when_called(expr, known_locals)
                    })
            }
            hir::ExprKind::When { arms, .. } => arms
                .iter()
                .any(|arm| self.function_value_may_suspend_when_called(&arm.body, known_locals)),
            _ => declared_effectful,
        }
    }
}

pub(crate) fn collect_known_fun_call_suspendability(
    types: &TypeStore,
    fun_index: &HashMap<String, &hir::FunDecl>,
    hir_facts: Rc<HirFacts>,
    materialized_pass_view: Option<&crate::mir::MaterializedMirPassView<'_>>,
) -> HashMap<String, bool> {
    let mut pass_summary_effects = HashMap::new();
    let mut pass_controlled_fqns = HashSet::new();
    if let Some(pass_view) = materialized_pass_view {
        for family in pass_view.instances() {
            if !family.summary_is_overridden() {
                continue;
            }
            let may_outward_effect = family.summary().may_outward_effect;
            pass_summary_effects.insert(family.root_fqn().to_string(), may_outward_effect);
            pass_controlled_fqns.insert(family.root_fqn().to_string());
            for fqn in family.callable_fqns() {
                pass_summary_effects.insert(fqn.to_string(), may_outward_effect);
                pass_controlled_fqns.insert(fqn.to_string());
            }
        }
    }

    let mut known_fun_effects = fun_index
        .iter()
        .map(|(fqn, fun)| {
            (
                fqn.clone(),
                pass_summary_effects
                    .get(fqn.as_str())
                    .copied()
                    .unwrap_or_else(|| {
                        fun.body.is_none() && function_ty_declared_effectful(types, fun.ty)
                    }),
            )
        })
        .collect::<HashMap<_, _>>();

    loop {
        let snapshot = known_fun_effects.clone();
        let mut newly_effectful = Vec::new();
        let mut changed = false;
        for (fqn, fun) in fun_index {
            if known_fun_effects.get(fqn).copied().unwrap_or(false) {
                continue;
            }
            if pass_controlled_fqns.contains(fqn.as_str()) {
                continue;
            }
            let Some(body) = &fun.body else {
                continue;
            };
            let mut known_local_metadata = HashMap::new();
            collect_known_local_metadata_in_fun(fun, &mut known_local_metadata);
            let context = EffectAnalysisCtx::new(
                snapshot.clone(),
                HashMap::new(),
                known_local_metadata,
                fun.source_path.clone(),
                Rc::clone(&hir_facts),
            )
            .with_continuation_escape_facts(
                ContinuationEscapeFacts::from_pass_view_for_callable(
                    materialized_pass_view,
                    Some(fqn.as_str()),
                    fun.source_path.as_path(),
                ),
            );
            let analysis = SuspendCallAnalysis {
                types,
                context: &context,
            };
            let seed_locals = fun
                .params
                .iter()
                .map(|param| (param.id, function_ty_declared_effectful(types, param.ty)))
                .collect::<HashMap<_, _>>();
            if analysis.block_may_suspend(body, &seed_locals) {
                newly_effectful.push(fqn.clone());
            }
        }
        if !newly_effectful.is_empty() {
            changed = true;
            for fqn in newly_effectful {
                known_fun_effects.insert(fqn, true);
            }
        }
        if !changed {
            break;
        }
    }

    known_fun_effects
}

#[cfg(test)]
pub(crate) fn collect_known_local_fun_call_suspendability_in_fun(
    fun: &hir::FunDecl,
    analysis: &SuspendCallAnalysis<'_>,
) -> HashMap<hir::SymbolId, bool> {
    let seed_locals = fun
        .params
        .iter()
        .map(|param| {
            (
                param.id,
                function_ty_declared_effectful(analysis.types, param.ty),
            )
        })
        .collect::<HashMap<_, _>>();
    fun.body
        .as_ref()
        .map(|body| analysis.solve_local_fun_effects_in_block(body, &seed_locals))
        .unwrap_or(seed_locals)
}

#[cfg(test)]
pub(crate) fn collect_effect_analysis_context_for_fun(
    lowered: &hir::LoweredHir,
    owner_fun: &hir::FunDecl,
) -> EffectAnalysisCtx {
    collect_effect_analysis_context_for_fun_with_pass_view(lowered, owner_fun, None)
}

#[cfg(test)]
pub(crate) fn collect_effect_analysis_context_for_fun_with_pass_view(
    lowered: &hir::LoweredHir,
    owner_fun: &hir::FunDecl,
    materialized_pass_view: Option<&crate::mir::MaterializedMirPassView<'_>>,
) -> EffectAnalysisCtx {
    let fun_index = lowered
        .file
        .items
        .iter()
        .filter_map(|item| match item {
            hir::Item::Fun(fun) => Some((fun.fqn.clone(), fun)),
            _ => None,
        })
        .chain(lowered.member_funs.iter().map(|fun| (fun.fqn.clone(), fun)))
        .collect::<HashMap<_, _>>();

    let hir_facts = Rc::new(
        crate::pipeline::build_hir_declaration_facts_for_migration(
            lowered,
            owner_fun.source_path.as_path(),
        )
        .expect("HIR declaration facts should build for effect analysis test fixture"),
    );
    let known_fun_effects = collect_known_fun_call_suspendability(
        &lowered.types,
        &fun_index,
        Rc::clone(&hir_facts),
        materialized_pass_view,
    );

    let mut known_local_metadata = HashMap::new();
    collect_known_local_metadata_in_fun(owner_fun, &mut known_local_metadata);
    let continuation_escape_facts = ContinuationEscapeFacts::from_pass_view_for_callable(
        materialized_pass_view,
        Some(owner_fun.fqn.as_str()),
        owner_fun.source_path.as_path(),
    );
    let analysis_seed = EffectAnalysisCtx::new(
        known_fun_effects.clone(),
        HashMap::new(),
        known_local_metadata.clone(),
        owner_fun.source_path.clone(),
        Rc::clone(&hir_facts),
    )
    .with_continuation_escape_facts(continuation_escape_facts.clone());
    let analysis = SuspendCallAnalysis {
        types: &lowered.types,
        context: &analysis_seed,
    };
    let known_local_fun_effects =
        collect_known_local_fun_call_suspendability_in_fun(owner_fun, &analysis);

    EffectAnalysisCtx::new(
        known_fun_effects,
        known_local_fun_effects,
        known_local_metadata,
        owner_fun.source_path.clone(),
        hir_facts,
    )
    .with_continuation_escape_facts(continuation_escape_facts)
}
