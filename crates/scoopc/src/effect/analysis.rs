//! Shared effect/state-machine analysis context and local metadata helpers.

#![allow(dead_code)]

use std::cell::Cell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::hir;
use crate::mir;
use crate::span::Span;
use crate::ty::{RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};
use scoopc_hir_facts::HirFacts;
use scoopc_hir_facts::declarations::{FieldOwnerKind, NominalKind};
use scoopc_hir_facts::globals::GlobalRootKind;
use scoopc_hir_facts::source_sites::{ConstructorCallTarget, ContinuationResumeContract};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct KnownLocalMetadata {
    pub(crate) ty: TypeId,
    pub(crate) mutable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EffectGlobalRootKind {
    TopLevelVal,
    TopLevelVar,
    ObjectSingleton,
}

#[derive(Debug, Clone)]
struct EffectFieldFact {
    owner_kind: FieldOwnerKind,
    owner: String,
    fqn: String,
    ty: TypeId,
}

/// Narrow semantic facts needed by shared effect planning.
///
/// LLVM codegen consumes this explicit query surface instead of holding the
/// complete `HirFacts` wrapper in its production context.
#[derive(Debug, Clone, Default)]
pub(crate) struct EffectAnalysisFacts {
    global_roots: HashMap<String, (EffectGlobalRootKind, Option<TypeId>)>,
    fields: Vec<EffectFieldFact>,
    callable_return_tys: HashMap<String, TypeId>,
    nominal_supertypes: HashMap<String, Vec<String>>,
    constructor_calls: HashMap<hir::CallSite, ConstructorCallTarget>,
    continuation_resumes: HashMap<hir::CallSite, ContinuationResumeContract>,
}

impl EffectAnalysisFacts {
    pub(crate) fn from_hir_facts(facts: &HirFacts) -> Self {
        let global_roots = facts
            .globals
            .roots
            .iter()
            .map(|root| {
                let kind = match root.kind {
                    GlobalRootKind::TopLevelVal => EffectGlobalRootKind::TopLevelVal,
                    GlobalRootKind::TopLevelVar => EffectGlobalRootKind::TopLevelVar,
                    GlobalRootKind::ObjectSingleton => EffectGlobalRootKind::ObjectSingleton,
                };
                (root.identity.display_name.clone(), (kind, root.ty))
            })
            .collect();
        let fields = facts
            .declarations
            .fields
            .iter()
            .map(|field| EffectFieldFact {
                owner_kind: field.owner_kind,
                owner: field.owner.as_str().to_string(),
                fqn: field.identity.display_name.clone(),
                ty: field.ty,
            })
            .collect();
        let callable_return_tys = facts
            .declarations
            .callables
            .iter()
            .map(|callable| (callable.identity.display_name.clone(), callable.return_ty))
            .collect();
        let nominal_supertypes = facts
            .declarations
            .nominals
            .iter()
            .filter(|nominal| nominal.kind == NominalKind::Class)
            .map(|nominal| {
                (
                    nominal.identity.display_name.clone(),
                    nominal
                        .direct_supertypes
                        .iter()
                        .map(|key| key.as_str().to_string())
                        .collect(),
                )
            })
            .collect();
        let constructor_calls = facts
            .source_sites
            .call_sites
            .iter()
            .filter_map(|site| {
                facts
                    .source_sites
                    .constructor_call(site.identity.source_path.as_path(), site.identity.span)
                    .map(|target| {
                        (
                            hir::CallSite::new(
                                site.identity.source_path.clone(),
                                site.identity.span,
                            ),
                            target.clone(),
                        )
                    })
            })
            .collect();
        let continuation_resumes = facts
            .source_sites
            .continuation_resumes
            .iter()
            .map(|resume| {
                (
                    hir::CallSite::new(resume.identity.source_path.clone(), resume.identity.span),
                    resume.clone(),
                )
            })
            .collect();

        Self {
            global_roots,
            fields,
            callable_return_tys,
            nominal_supertypes,
            constructor_calls,
            continuation_resumes,
        }
    }

    pub(crate) fn top_level_value_ty(&self, fqn: &str) -> Option<TypeId> {
        self.global_roots.get(fqn).and_then(|(kind, ty)| {
            matches!(
                kind,
                EffectGlobalRootKind::TopLevelVal | EffectGlobalRootKind::TopLevelVar
            )
            .then_some(*ty)
            .flatten()
        })
    }

    pub(crate) fn object_property_ty(&self, fqn: &str) -> Option<TypeId> {
        self.fields
            .iter()
            .find(|field| field.owner_kind == FieldOwnerKind::Object && field.fqn == fqn)
            .map(|field| field.ty)
    }

    pub(crate) fn fun_return_ty(&self, fqn: &str) -> Option<TypeId> {
        self.callable_return_tys.get(fqn).copied()
    }

    pub(crate) fn resolve_nominal_field_ty(
        &self,
        types: &TypeStore,
        receiver_ty: TypeId,
        field_fqn: &str,
    ) -> Option<TypeId> {
        self.resolve_struct_field_ty(types, receiver_ty, field_fqn)
            .or_else(|| self.resolve_class_field_ty(types, receiver_ty, field_fqn))
    }

    pub(crate) fn is_object_value_fqn(&self, fqn: &str) -> bool {
        self.global_roots
            .get(fqn)
            .is_some_and(|(kind, _)| *kind == EffectGlobalRootKind::ObjectSingleton)
    }

    pub(crate) fn is_object_property_fqn(&self, fqn: &str) -> bool {
        self.fields
            .iter()
            .any(|field| field.owner_kind == FieldOwnerKind::Object && field.fqn == fqn)
    }

    pub(crate) fn is_top_level_immutable_value_fqn(&self, fqn: &str) -> bool {
        self.global_roots
            .get(fqn)
            .is_some_and(|(kind, _)| *kind == EffectGlobalRootKind::TopLevelVal)
    }

    pub(crate) fn constructor_call(
        &self,
        source_path: &Path,
        span: Span,
    ) -> Option<&ConstructorCallTarget> {
        self.constructor_calls
            .get(&hir::CallSite::new(source_path.to_path_buf(), span))
    }

    pub(crate) fn continuation_resume(
        &self,
        source_path: &Path,
        span: Span,
    ) -> Option<&ContinuationResumeContract> {
        self.continuation_resumes
            .get(&hir::CallSite::new(source_path.to_path_buf(), span))
    }

    pub(crate) fn has_continuation_resume(&self, source_path: &Path, span: Span) -> bool {
        self.continuation_resume(source_path, span).is_some()
    }

    pub(crate) fn resolve_expr_concrete_type<LocalTyLookup>(
        &self,
        types: &TypeStore,
        expr: &hir::Expr,
        local_ty_lookup: &LocalTyLookup,
    ) -> Option<TypeId>
    where
        LocalTyLookup: Fn(hir::SymbolId) -> Option<TypeId>,
    {
        if crate::expr_facts::hir_ty_is_precise(types, expr.ty) {
            return Some(expr.ty);
        }

        match &expr.kind {
            hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) => local_ty_lookup(*id),
            hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) => {
                self.top_level_value_ty(fqn)
            }
            hir::ExprKind::MemberAccess { receiver, member } => {
                self.resolve_member_access_concrete_type(types, receiver, member, local_ty_lookup)
            }
            hir::ExprKind::Call { callee, .. } => {
                self.resolve_call_result_type(types, callee, local_ty_lookup)
            }
            hir::ExprKind::Block(block) => block.stmts.last().and_then(|stmt| {
                let hir::StmtKind::Expr(expr) = &stmt.kind else {
                    return None;
                };
                self.resolve_expr_concrete_type(types, expr, local_ty_lookup)
            }),
            hir::ExprKind::If {
                then_branch,
                else_branch,
                ..
            } => else_branch.as_deref().and_then(|else_branch| {
                self.resolve_common_branch_concrete_type(
                    types,
                    [then_branch.as_ref(), else_branch],
                    local_ty_lookup,
                )
            }),
            hir::ExprKind::When { arms, .. } => self.resolve_common_branch_concrete_type(
                types,
                arms.iter().map(|arm| &arm.body),
                local_ty_lookup,
            ),
            _ => None,
        }
    }

    fn resolve_common_branch_concrete_type<'a, LocalTyLookup>(
        &self,
        types: &TypeStore,
        exprs: impl IntoIterator<Item = &'a hir::Expr>,
        local_ty_lookup: &LocalTyLookup,
    ) -> Option<TypeId>
    where
        LocalTyLookup: Fn(hir::SymbolId) -> Option<TypeId>,
    {
        let mut candidate = None;
        for expr in exprs {
            let resolved = self.resolve_expr_concrete_type(types, expr, local_ty_lookup)?;
            match candidate {
                None => candidate = Some(resolved),
                Some(existing) if existing == resolved => {}
                Some(_) => return None,
            }
        }
        candidate
    }

    fn resolve_member_access_concrete_type<LocalTyLookup>(
        &self,
        types: &TypeStore,
        receiver: &hir::Expr,
        member: &hir::MemberAccess,
        local_ty_lookup: &LocalTyLookup,
    ) -> Option<TypeId>
    where
        LocalTyLookup: Fn(hir::SymbolId) -> Option<TypeId>,
    {
        let field_fqn = match member.resolved.as_ref()? {
            hir::MemberRef::Value { fqn, .. } | hir::MemberRef::ExtensionValue { fqn, .. } => fqn,
            _ => return None,
        };

        if let Some(ty) = self.top_level_value_ty(field_fqn) {
            return Some(ty);
        }
        if let Some(ty) = self.object_property_ty(field_fqn) {
            return Some(ty);
        }

        let receiver_ty = self
            .resolve_expr_concrete_type(types, receiver, local_ty_lookup)
            .unwrap_or(receiver.ty);
        self.resolve_nominal_field_ty(types, receiver_ty, field_fqn)
    }

    fn resolve_call_result_type<LocalTyLookup>(
        &self,
        types: &TypeStore,
        callee: &hir::Expr,
        local_ty_lookup: &LocalTyLookup,
    ) -> Option<TypeId>
    where
        LocalTyLookup: Fn(hir::SymbolId) -> Option<TypeId>,
    {
        if let Some(callee_ty) = self.resolve_expr_concrete_type(types, callee, local_ty_lookup)
            && let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = types.kind(callee_ty)
            && crate::expr_facts::hir_ty_is_precise(types, fun_ty.return_ty)
        {
            return Some(fun_ty.return_ty);
        }

        let fqn = match &callee.kind {
            hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) => Some(fqn.as_str()),
            hir::ExprKind::UnresolvedIdent { name } => Some(name.as_str()),
            hir::ExprKind::MemberAccess { member, .. } => match member.resolved.as_ref()? {
                hir::MemberRef::Fun { fqn, .. } | hir::MemberRef::ExtensionFun { fqn, .. } => {
                    Some(fqn.as_str())
                }
                _ => None,
            },
            _ => None,
        }?;

        if let Some(return_ty) = self.fun_return_ty(fqn)
            && crate::expr_facts::hir_ty_is_precise(types, return_ty)
        {
            return Some(return_ty);
        }

        if let Some(class_ty) = types.find_nominal_ref_by_fqn(fqn) {
            return Some(class_ty);
        }

        types.iter_ids().find(|id| {
            matches!(
                types.kind(*id),
                TypeKind::Value(ValueTypeKind::Nominal(nominal))
                    if nominal.fqn == fqn && nominal.args.is_empty()
            )
        })
    }

    fn resolve_struct_field_ty(
        &self,
        types: &TypeStore,
        receiver_ty: TypeId,
        field_fqn: &str,
    ) -> Option<TypeId> {
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = types.kind(receiver_ty) else {
            return None;
        };
        let layout_key = hir::mangle_nominal_fqn(&nominal.fqn, &nominal.args, types);
        self.lookup_field_ty_by_owner(FieldOwnerKind::Struct, &layout_key, field_fqn)
            .or_else(|| {
                (layout_key != nominal.fqn)
                    .then(|| {
                        self.lookup_field_ty_by_owner(
                            FieldOwnerKind::Struct,
                            &nominal.fqn,
                            field_fqn,
                        )
                    })
                    .flatten()
            })
    }

    fn resolve_class_field_ty(
        &self,
        types: &TypeStore,
        receiver_ty: TypeId,
        field_fqn: &str,
    ) -> Option<TypeId> {
        let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = types.kind(receiver_ty) else {
            return None;
        };
        let layout_key = hir::mangle_nominal_fqn(&nominal.fqn, &nominal.args, types);
        self.lookup_class_field_ty_by_key(&layout_key, field_fqn)
            .or_else(|| {
                (layout_key != nominal.fqn)
                    .then(|| self.lookup_class_field_ty_by_key(&nominal.fqn, field_fqn))
                    .flatten()
            })
    }

    fn lookup_class_field_ty_by_key(&self, class_key: &str, field_fqn: &str) -> Option<TypeId> {
        self.lookup_field_ty_by_owner(FieldOwnerKind::Class, class_key, field_fqn)
            .or_else(|| {
                self.nominal_supertypes
                    .get(class_key)
                    .and_then(|supertypes| supertypes.first())
                    .and_then(|super_key| {
                        self.lookup_class_field_ty_by_key(super_key.as_str(), field_fqn)
                    })
            })
    }

    fn lookup_field_ty_by_owner(
        &self,
        owner_kind: FieldOwnerKind,
        owner_key: &str,
        field_fqn: &str,
    ) -> Option<TypeId> {
        self.fields
            .iter()
            .find(|field| {
                field.owner_kind == owner_kind
                    && field.owner.as_str() == owner_key
                    && field.fqn == field_fqn
            })
            .map(|field| field.ty)
    }
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
    pub(crate) facts: Rc<EffectAnalysisFacts>,
    continuation_escape_facts: ContinuationEscapeFacts,
}

impl EffectAnalysisCtx {
    pub(crate) fn new(
        known_fun_effects: HashMap<String, bool>,
        known_local_fun_effects: HashMap<hir::SymbolId, bool>,
        known_local_metadata: HashMap<hir::SymbolId, KnownLocalMetadata>,
        current_source_path: PathBuf,
        facts: Rc<EffectAnalysisFacts>,
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
            facts,
            continuation_escape_facts: ContinuationEscapeFacts::default(),
        }
    }

    pub(crate) fn from_hir_facts(
        known_fun_effects: HashMap<String, bool>,
        known_local_fun_effects: HashMap<hir::SymbolId, bool>,
        known_local_metadata: HashMap<hir::SymbolId, KnownLocalMetadata>,
        current_source_path: PathBuf,
        hir_facts: Rc<HirFacts>,
    ) -> Self {
        Self::new(
            known_fun_effects,
            known_local_fun_effects,
            known_local_metadata,
            current_source_path,
            Rc::new(EffectAnalysisFacts::from_hir_facts(hir_facts.as_ref())),
        )
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
