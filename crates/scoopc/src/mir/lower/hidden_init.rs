//! HiddenInitEffectAnalyzer for top-level init effect inference.

#![allow(dead_code)]

use super::*;

pub(in crate::mir::lower) struct HiddenInitEffectAnalyzer<'a> {
    pub(in crate::mir::lower) lowered: &'a hir::LoweredHir,
}

impl<'a> HiddenInitEffectAnalyzer<'a> {
    pub(in crate::mir::lower) fn new(lowered: &'a hir::LoweredHir) -> Self {
        Self { lowered }
    }

    pub(in crate::mir::lower) fn class_ctor_effect_row(
        &self,
        class_fqn: &str,
        ctor_span: Option<Span>,
    ) -> EffectRow {
        let mut visiting = HashSet::new();
        EffectRow::new(self.class_ctor_effect_terms(class_fqn, ctor_span, &mut visiting))
    }

    pub(in crate::mir::lower) fn object_init_effect_row(&self, object_fqn: &str) -> EffectRow {
        let mut visiting = HashSet::new();
        EffectRow::new(self.object_init_effect_terms(object_fqn, &mut visiting))
    }

    pub(in crate::mir::lower) fn top_level_immutable_value_effect_row(
        &self,
        value_fqn: &str,
    ) -> EffectRow {
        let mut visiting = HashSet::new();
        EffectRow::new(self.top_level_immutable_value_effect_terms(value_fqn, &mut visiting))
    }

    pub(in crate::mir::lower) fn class_ctor_effect_terms(
        &self,
        class_fqn: &str,
        ctor_span: Option<Span>,
        visiting: &mut HashSet<String>,
    ) -> Vec<TypeId> {
        let Some(class) = self.lookup_class_init(class_fqn) else {
            return Vec::new();
        };
        let key = format!("class:{}:{:?}", class.fqn, ctor_span);
        if !visiting.insert(key.clone()) {
            return Vec::new();
        }

        let mut terms = Vec::new();
        if let Some(super_fqn) = class.super_class_fqn.as_deref() {
            terms.extend(self.class_ctor_effect_terms(super_fqn, None, visiting));
        }
        terms.extend(self.scan_call_args(
            &class.super_ctor_args,
            class.source_path.as_path(),
            visiting,
        ));

        let selected_ctor = ctor_span
            .and_then(|span| class.ctors.iter().find(|ctor| ctor.span == span))
            .or_else(|| {
                if ctor_span.is_none() && class.ctors.len() == 1 {
                    class.ctors.first()
                } else {
                    None
                }
            });
        if let Some(ctor) = selected_ctor {
            for param in &ctor.params {
                if let Some(default_value) = param.default_value.as_ref() {
                    terms.extend(self.scan_expr(
                        default_value,
                        class.source_path.as_path(),
                        visiting,
                    ));
                }
            }
            if let Some(delegation) = ctor.delegation.as_ref() {
                terms.extend(self.scan_call_args(
                    &delegation.args,
                    class.source_path.as_path(),
                    visiting,
                ));
            }
        }

        for step in &class.steps {
            match step {
                hir::ClassInitStep::PropertyInit { init, .. } => {
                    terms.extend(self.scan_expr(init, class.source_path.as_path(), visiting));
                }
                hir::ClassInitStep::InitBlock { block } => {
                    terms.extend(self.scan_block(block, class.source_path.as_path(), visiting));
                }
            }
        }
        if let Some(ctor) = selected_ctor
            && let Some(body) = ctor.body.as_ref()
        {
            terms.extend(self.scan_block(body, class.source_path.as_path(), visiting));
        }

        visiting.remove(&key);
        terms
    }

    pub(in crate::mir::lower) fn object_init_effect_terms(
        &self,
        object_fqn: &str,
        visiting: &mut HashSet<String>,
    ) -> Vec<TypeId> {
        let Some(object_init) = self.lowered.object_inits.get(object_fqn) else {
            return Vec::new();
        };
        let key = format!("object:{object_fqn}");
        if !visiting.insert(key.clone()) {
            return Vec::new();
        }

        let mut terms = Vec::new();
        for step in &object_init.steps {
            match step {
                hir::ObjectInitStep::PropertyInit { init, .. } => {
                    terms.extend(self.scan_expr(init, object_init.source_path.as_path(), visiting));
                }
                hir::ObjectInitStep::InitBlock { block } => {
                    terms.extend(self.scan_block(
                        block,
                        object_init.source_path.as_path(),
                        visiting,
                    ));
                }
            }
        }

        visiting.remove(&key);
        terms
    }

    pub(in crate::mir::lower) fn top_level_immutable_value_effect_terms(
        &self,
        value_fqn: &str,
        visiting: &mut HashSet<String>,
    ) -> Vec<TypeId> {
        let Some(value) = self.lowered.top_level_immutable_values.get(value_fqn) else {
            return Vec::new();
        };
        let key = format!("top-level-val:{value_fqn}");
        if !visiting.insert(key.clone()) {
            return Vec::new();
        }

        let terms = value
            .init
            .as_ref()
            .map(|init| self.scan_expr(init, value.source_path.as_path(), visiting))
            .unwrap_or_default();
        visiting.remove(&key);
        terms
    }

    pub(in crate::mir::lower) fn lookup_class_init(
        &self,
        class_fqn: &str,
    ) -> Option<&'a hir::ClassInit> {
        self.lowered.class_inits.get(class_fqn).or_else(|| {
            self.lowered
                .class_inits
                .values()
                .find(|class| class.fqn == class_fqn)
        })
    }

    pub(in crate::mir::lower) fn scan_block(
        &self,
        block: &hir::Block,
        source_path: &std::path::Path,
        visiting: &mut HashSet<String>,
    ) -> Vec<TypeId> {
        let mut terms = Vec::new();
        for stmt in &block.stmts {
            terms.extend(self.scan_stmt(stmt, source_path, visiting));
        }
        terms
    }

    pub(in crate::mir::lower) fn scan_stmt(
        &self,
        stmt: &hir::Stmt,
        source_path: &std::path::Path,
        visiting: &mut HashSet<String>,
    ) -> Vec<TypeId> {
        match &stmt.kind {
            hir::StmtKind::Expr(expr) => self.scan_expr(expr, source_path, visiting),
            hir::StmtKind::Val(decl) => decl
                .init
                .as_ref()
                .map(|expr| self.scan_expr(expr, source_path, visiting))
                .unwrap_or_default(),
            hir::StmtKind::Assign { lhs, rhs, .. } => {
                let mut terms = self.scan_expr(lhs, source_path, visiting);
                terms.extend(self.scan_expr(rhs, source_path, visiting));
                terms
            }
            hir::StmtKind::While { cond, body } => {
                let mut terms = self.scan_expr(cond, source_path, visiting);
                terms.extend(self.scan_block(body, source_path, visiting));
                terms
            }
            hir::StmtKind::Return { value } => value
                .as_ref()
                .map(|expr| self.scan_expr(expr, source_path, visiting))
                .unwrap_or_default(),
            hir::StmtKind::Empty
            | hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Todo(_) => Vec::new(),
        }
    }

    pub(in crate::mir::lower) fn scan_expr(
        &self,
        expr: &hir::Expr,
        source_path: &std::path::Path,
        visiting: &mut HashSet<String>,
    ) -> Vec<TypeId> {
        match &expr.kind {
            hir::ExprKind::Missing
            | hir::ExprKind::Literal(_)
            | hir::ExprKind::ClassLiteral(_)
            | hir::ExprKind::UnresolvedIdent { .. }
            | hir::ExprKind::Closure(_)
            | hir::ExprKind::Todo(_) => Vec::new(),
            hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) => {
                let mut terms = self.object_init_effect_terms(fqn, visiting);
                terms.extend(self.top_level_immutable_value_effect_terms(fqn, visiting));
                terms
            }
            hir::ExprKind::VarRef(_) => Vec::new(),
            hir::ExprKind::Block(block) => self.scan_block(block, source_path, visiting),
            hir::ExprKind::Unary { expr, .. }
            | hir::ExprKind::Cast { expr, .. }
            | hir::ExprKind::TypeCheck { expr, .. } => self.scan_expr(expr, source_path, visiting),
            hir::ExprKind::Binary { lhs, rhs, .. } => {
                let mut terms = self.scan_expr(lhs, source_path, visiting);
                terms.extend(self.scan_expr(rhs, source_path, visiting));
                terms
            }
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let mut terms = self.scan_expr(cond, source_path, visiting);
                terms.extend(self.scan_expr(then_branch, source_path, visiting));
                if let Some(else_branch) = else_branch.as_deref() {
                    terms.extend(self.scan_expr(else_branch, source_path, visiting));
                }
                terms
            }
            hir::ExprKind::When { subject, arms } => {
                let mut terms = self.scan_expr(subject, source_path, visiting);
                for arm in arms {
                    if let Some(guard) = arm.guard.as_ref() {
                        terms.extend(self.scan_expr(guard, source_path, visiting));
                    }
                    terms.extend(self.scan_expr(&arm.body, source_path, visiting));
                }
                terms
            }
            hir::ExprKind::MemberAccess { receiver, member } => {
                let mut terms = self.scan_expr(receiver, source_path, visiting);
                if let Some(hir::MemberRef::Value { fqn, .. }) = member.resolved.as_ref()
                    && let Some((owner_fqn, _)) = fqn.rsplit_once('.')
                {
                    terms.extend(self.object_init_effect_terms(owner_fqn, visiting));
                }
                terms
            }
            hir::ExprKind::StructLit { fields, .. } => {
                let mut terms = Vec::new();
                for field in fields {
                    terms.extend(self.scan_expr(&field.value, source_path, visiting));
                }
                terms
            }
            hir::ExprKind::TupleLit { elements } => {
                let mut terms = Vec::new();
                for element in elements {
                    terms.extend(self.scan_expr(element, source_path, visiting));
                }
                terms
            }
            hir::ExprKind::InterpolatedString { parts, .. } => {
                let mut terms = Vec::new();
                for part in parts {
                    if let hir::InterpolatedStringPart::Expr { expr } = part {
                        terms.extend(self.scan_expr(expr, source_path, visiting));
                    }
                }
                terms
            }
            hir::ExprKind::Call { callee, args } => {
                let mut terms = self.scan_expr(callee, source_path, visiting);
                terms.extend(self.scan_call_args(args, source_path, visiting));
                if let Some(info) = self
                    .lowered
                    .ctor_call_sites
                    .get(&hir::CallSite::new(source_path.to_path_buf(), expr.span))
                {
                    terms.extend(self.class_ctor_effect_terms(
                        &info.class_fqn,
                        info.ctor_span,
                        visiting,
                    ));
                } else if let TypeKind::Ref(RefTypeKind::Function(fun_ty)) =
                    self.lowered.types.kind(callee.ty)
                {
                    terms.extend(fun_ty.effects.terms.iter().copied());
                }
                terms
            }
            hir::ExprKind::Perform {
                effect_ty, args, ..
            } => {
                let mut terms = self.scan_call_args(args, source_path, visiting);
                terms.push(*effect_ty);
                terms
            }
            hir::ExprKind::Handle(handle) => {
                let mut terms = self.scan_block(&handle.body, source_path, visiting);
                for arm in &handle.arms {
                    terms.extend(self.scan_expr(&arm.body, source_path, visiting));
                }
                if let Some(finally) = handle.finally.as_ref() {
                    terms.extend(self.scan_block(finally, source_path, visiting));
                }
                terms
            }
        }
    }

    pub(in crate::mir::lower) fn scan_call_args(
        &self,
        args: &[hir::CallArg],
        source_path: &std::path::Path,
        visiting: &mut HashSet<String>,
    ) -> Vec<TypeId> {
        let mut terms = Vec::new();
        for arg in args {
            match arg {
                hir::CallArg::Positional(expr) => {
                    terms.extend(self.scan_expr(expr, source_path, visiting));
                }
                hir::CallArg::Named { value, .. } => {
                    terms.extend(self.scan_expr(value, source_path, visiting));
                }
            }
        }
        terms
    }
}
