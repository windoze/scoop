//! Shared effect/state-machine analysis context and local metadata helpers.

#![allow(dead_code)]

use std::cell::Cell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::hir;
use crate::mir;
use crate::source_site_migration_facts::SourceSiteMigrationFacts;
use crate::span::Span;
use crate::ty::TypeId;
use scoopc_hir_facts::HirFacts;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct KnownLocalMetadata {
    pub(crate) ty: TypeId,
    pub(crate) mutable: bool,
}

/// Continuation escape state exposed to effect/state-machine planning.
///
/// This is deliberately coarser than the MIR-local fact: planning only needs to know whether a
/// `Continuation.resume(...)` call site is proven to stay local, is known to involve an escaping
/// continuation, or has no trustworthy fact and must be treated conservatively.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum ContinuationEscapeState {
    LocalResumeOnly,
    Escaping,
    #[default]
    Unknown,
}

impl ContinuationEscapeState {
    #[cfg(test)]
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::LocalResumeOnly => "local-resume-only",
            Self::Escaping => "escaping",
            Self::Unknown => "unknown",
        }
    }

    pub(crate) fn structural_signature(self) -> usize {
        match self {
            Self::LocalResumeOnly => 1,
            Self::Escaping => 2,
            Self::Unknown => 3,
        }
    }

    fn from_mir_status(status: mir::EscapeStatus) -> Self {
        match status {
            mir::EscapeStatus::NonEscaping => Self::LocalResumeOnly,
            mir::EscapeStatus::Escapes => Self::Escaping,
            mir::EscapeStatus::Unknown => Self::Unknown,
        }
    }

    fn combine(self, other: Self) -> Self {
        match (self, other) {
            (Self::Escaping, _) | (_, Self::Escaping) => Self::Escaping,
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            (Self::LocalResumeOnly, Self::LocalResumeOnly) => Self::LocalResumeOnly,
        }
    }
}

/// Call-site keyed continuation escape facts consumed by shared effect analysis.
#[derive(Debug, Clone, Default)]
pub(crate) struct ContinuationEscapeFacts {
    by_call_site: HashMap<hir::CallSite, ContinuationEscapeState>,
}

impl ContinuationEscapeFacts {
    pub(crate) fn from_pass_view_for_callable(
        pass_view: Option<&mir::MaterializedMirPassView<'_>>,
        callable_fqn: Option<&str>,
        source_path: &Path,
    ) -> Self {
        let Some(pass_view) = pass_view else {
            return Self::default();
        };
        let Some(callable_fqn) = callable_fqn else {
            return Self::default();
        };

        let mut out = Self::default();
        for (fact_fqn, callable_facts) in pass_view.escape_facts().callables() {
            if !callable_fqn_matches_owner(fact_fqn, callable_fqn) {
                continue;
            }
            for continuation in callable_facts.continuations() {
                let state = ContinuationEscapeState::from_mir_status(continuation.status);
                for span in &continuation.resume_call_spans {
                    out.insert(hir::CallSite::new(source_path.to_path_buf(), *span), state);
                }
            }
        }
        out
    }

    pub(crate) fn status_for_call_site(
        &self,
        call_site: &hir::CallSite,
    ) -> ContinuationEscapeState {
        self.by_call_site
            .get(call_site)
            .copied()
            .unwrap_or(ContinuationEscapeState::Unknown)
    }

    fn insert(&mut self, call_site: hir::CallSite, state: ContinuationEscapeState) {
        self.by_call_site
            .entry(call_site)
            .and_modify(|existing| *existing = existing.combine(state))
            .or_insert(state);
    }
}

fn callable_fqn_matches_owner(candidate: &str, owner: &str) -> bool {
    candidate == owner
        || candidate
            .strip_prefix(owner)
            .is_some_and(|suffix| suffix.starts_with("::<") || suffix.starts_with('.'))
}

/// Backend-agnostic shared analysis input for effect/state-machine planning.
#[derive(Debug, Clone)]
pub(crate) struct EffectAnalysisCtx {
    pub(crate) known_fun_effects: HashMap<String, bool>,
    pub(crate) known_local_fun_effects: HashMap<hir::SymbolId, bool>,
    pub(crate) known_local_metadata: HashMap<hir::SymbolId, KnownLocalMetadata>,
    next_synthetic_symbol_raw: Cell<u32>,
    current_source_path: PathBuf,
    pub(crate) hir_facts: Rc<HirFacts>,
    pub(crate) source_site_facts: Rc<SourceSiteMigrationFacts>,
    continuation_escape_facts: ContinuationEscapeFacts,
}

impl EffectAnalysisCtx {
    pub(crate) fn new(
        known_fun_effects: HashMap<String, bool>,
        known_local_fun_effects: HashMap<hir::SymbolId, bool>,
        known_local_metadata: HashMap<hir::SymbolId, KnownLocalMetadata>,
        current_source_path: PathBuf,
        hir_facts: Rc<HirFacts>,
        source_site_facts: Rc<SourceSiteMigrationFacts>,
    ) -> Self {
        let next_synthetic_symbol_raw = known_local_metadata
            .keys()
            .copied()
            .map(hir::SymbolId::as_u32)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        Self {
            known_fun_effects,
            known_local_fun_effects,
            known_local_metadata,
            next_synthetic_symbol_raw: Cell::new(next_synthetic_symbol_raw),
            current_source_path,
            hir_facts,
            source_site_facts,
            continuation_escape_facts: ContinuationEscapeFacts::default(),
        }
    }

    pub(crate) fn current_source_path(&self) -> &Path {
        &self.current_source_path
    }

    pub(crate) fn call_site(&self, span: Span) -> hir::CallSite {
        hir::CallSite::new(self.current_source_path.clone(), span)
    }

    pub(crate) fn with_continuation_escape_facts(mut self, facts: ContinuationEscapeFacts) -> Self {
        self.continuation_escape_facts = facts;
        self
    }

    pub(crate) fn continuation_escape_facts(&self) -> &ContinuationEscapeFacts {
        &self.continuation_escape_facts
    }

    pub(crate) fn continuation_escape_state_for_call_span(
        &self,
        span: Span,
    ) -> ContinuationEscapeState {
        self.continuation_escape_facts
            .status_for_call_site(&self.call_site(span))
    }

    pub(crate) fn reserve_synthetic_symbol_floor(&self, floor: u32) {
        let current = self.next_synthetic_symbol_raw.get();
        if floor > current {
            self.next_synthetic_symbol_raw.set(floor);
        }
    }

    pub(crate) fn synthetic_symbol_seed(&self) -> u32 {
        self.next_synthetic_symbol_raw.get()
    }

    pub(crate) fn restore_synthetic_symbol_seed(&self, seed: u32) {
        self.next_synthetic_symbol_raw.set(seed);
    }

    pub(crate) fn allocate_synthetic_symbol_id(&self) -> hir::SymbolId {
        let raw = self.next_synthetic_symbol_raw.get();
        self.next_synthetic_symbol_raw.set(raw.saturating_add(1));
        hir::SymbolId::from_raw(raw)
    }

    pub(crate) fn extend_known_local_metadata_from_handle(&mut self, handle: &hir::HandleExpr) {
        collect_known_local_metadata_in_handle(handle, &mut self.known_local_metadata);
    }
}

pub(crate) fn collect_known_local_metadata_in_handle(
    handle: &hir::HandleExpr,
    out: &mut HashMap<hir::SymbolId, KnownLocalMetadata>,
) {
    collect_known_local_metadata_in_block(&handle.body, out);
    for arm in &handle.arms {
        collect_known_local_metadata_in_expr(&arm.body, out);
    }
    if let Some(finally_block) = handle.finally.as_ref() {
        collect_known_local_metadata_in_block(finally_block, out);
    }
}

pub(crate) fn collect_known_local_metadata_in_fun(
    fun: &hir::FunDecl,
    out: &mut HashMap<hir::SymbolId, KnownLocalMetadata>,
) {
    for param in &fun.params {
        out.insert(
            param.id,
            KnownLocalMetadata {
                ty: param.ty,
                mutable: false,
            },
        );
    }
    if let Some(body) = &fun.body {
        collect_known_local_metadata_in_block(body, out);
    }
}

pub(crate) fn collect_known_local_metadata_in_block(
    block: &hir::Block,
    out: &mut HashMap<hir::SymbolId, KnownLocalMetadata>,
) {
    for stmt in &block.stmts {
        collect_known_local_metadata_in_stmt(stmt, out);
    }
}

fn collect_known_local_metadata_in_stmt(
    stmt: &hir::Stmt,
    out: &mut HashMap<hir::SymbolId, KnownLocalMetadata>,
) {
    match &stmt.kind {
        hir::StmtKind::Val(decl) => {
            if let Some(id) = decl.id {
                out.insert(
                    id,
                    KnownLocalMetadata {
                        ty: decl.ty,
                        mutable: decl.mutable,
                    },
                );
            }
            if let Some(init) = decl.init.as_ref() {
                collect_known_local_metadata_in_expr(init, out);
            }
        }
        hir::StmtKind::Expr(expr) => collect_known_local_metadata_in_expr(expr, out),
        hir::StmtKind::Assign { lhs, rhs, .. } => {
            collect_known_local_metadata_in_expr(lhs, out);
            collect_known_local_metadata_in_expr(rhs, out);
        }
        hir::StmtKind::While { cond, body } => {
            collect_known_local_metadata_in_expr(cond, out);
            collect_known_local_metadata_in_block(body, out);
        }
        hir::StmtKind::Return { value } => {
            if let Some(expr) = value {
                collect_known_local_metadata_in_expr(expr, out);
            }
        }
        hir::StmtKind::Empty
        | hir::StmtKind::Break { .. }
        | hir::StmtKind::Continue { .. }
        | hir::StmtKind::Todo(_) => {}
    }
}

pub(crate) fn collect_known_local_metadata_in_expr(
    expr: &hir::Expr,
    out: &mut HashMap<hir::SymbolId, KnownLocalMetadata>,
) {
    match &expr.kind {
        hir::ExprKind::Block(block) => collect_known_local_metadata_in_block(block, out),
        hir::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_known_local_metadata_in_expr(cond, out);
            collect_known_local_metadata_in_expr(then_branch, out);
            if let Some(else_branch) = else_branch.as_deref() {
                collect_known_local_metadata_in_expr(else_branch, out);
            }
        }
        hir::ExprKind::When { subject, arms } => {
            collect_known_local_metadata_in_expr(subject, out);
            for arm in arms {
                if let Some(guard) = arm.guard.as_ref() {
                    collect_known_local_metadata_in_expr(guard, out);
                }
                collect_known_local_metadata_in_expr(&arm.body, out);
            }
        }
        hir::ExprKind::Closure(closure) => {
            for param in &closure.params {
                out.insert(
                    param.id,
                    KnownLocalMetadata {
                        ty: param.ty,
                        mutable: false,
                    },
                );
            }
            collect_known_local_metadata_in_expr(&closure.body, out);
        }
        hir::ExprKind::StructLit { fields, .. } => {
            for field in fields {
                collect_known_local_metadata_in_expr(&field.value, out);
            }
        }
        hir::ExprKind::TupleLit { elements } => {
            for element in elements {
                collect_known_local_metadata_in_expr(element, out);
            }
        }
        hir::ExprKind::InterpolatedString { parts, .. } => {
            for part in parts {
                if let hir::InterpolatedStringPart::Expr { expr } = part {
                    collect_known_local_metadata_in_expr(expr, out);
                }
            }
        }
        hir::ExprKind::Unary { expr: inner, .. }
        | hir::ExprKind::Cast { expr: inner, .. }
        | hir::ExprKind::TypeCheck { expr: inner, .. }
        | hir::ExprKind::MemberAccess {
            receiver: inner, ..
        } => collect_known_local_metadata_in_expr(inner, out),
        hir::ExprKind::Binary { lhs, rhs, .. } => {
            collect_known_local_metadata_in_expr(lhs, out);
            collect_known_local_metadata_in_expr(rhs, out);
        }
        hir::ExprKind::Call { callee, args } => {
            collect_known_local_metadata_in_expr(callee, out);
            for arg in args {
                match arg {
                    hir::CallArg::Positional(expr) => {
                        collect_known_local_metadata_in_expr(expr, out)
                    }
                    hir::CallArg::Named { value, .. } => {
                        collect_known_local_metadata_in_expr(value, out)
                    }
                }
            }
        }
        hir::ExprKind::Perform { args, .. } => {
            for arg in args {
                match arg {
                    hir::CallArg::Positional(expr) => {
                        collect_known_local_metadata_in_expr(expr, out)
                    }
                    hir::CallArg::Named { value, .. } => {
                        collect_known_local_metadata_in_expr(value, out)
                    }
                }
            }
        }
        hir::ExprKind::Handle(handle) => {
            collect_known_local_metadata_in_handle(handle, out);
        }
        hir::ExprKind::Missing
        | hir::ExprKind::Literal(_)
        | hir::ExprKind::VarRef(_)
        | hir::ExprKind::UnresolvedIdent { .. }
        | hir::ExprKind::ClassLiteral(_)
        | hir::ExprKind::Todo(_) => {}
    }
}

pub(crate) fn collect_known_local_metadata_in_handle_arm(
    arm: &hir::HandleArm,
) -> HashMap<hir::SymbolId, KnownLocalMetadata> {
    let mut out = HashMap::new();
    for binder in &arm.op.binders {
        out.insert(
            binder.id,
            KnownLocalMetadata {
                ty: binder.ty,
                mutable: false,
            },
        );
    }
    collect_known_local_metadata_in_expr(&arm.body, &mut out);
    out
}
