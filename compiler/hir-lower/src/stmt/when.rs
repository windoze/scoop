use crate::expr::SmartCastFacts;

use super::*;

mod conditions;
mod coverage;
mod subject;
mod values;

struct Subject {
    value: hir::Expr,
    reference: ast::Expr,
}

/// A checked head retains its bindings while result inference may postpone
/// the body. Conditions are resolved once, in source order.
struct ArmHead {
    condition: hir::WhenCondition,
    guard: Option<hir::WhenGuard>,
    scope: Scopes,
    facts: SmartCastFacts,
    coverage: Vec<hir::Pattern>,
    requires_expected: bool,
    body_reachable: bool,
}

impl Lowerer {
    pub(super) fn lower_when(
        &mut self,
        when: &ast::When,
        out: &mut Vec<hir::Statement>,
    ) -> Option<hir::StatementKind> {
        self.in_when_scope(|this| {
            let subject = this.lower_when_subject(when, out)?;
            let mut heads = Vec::with_capacity(when.arms.len());
            let mut arms = Vec::with_capacity(when.arms.len());
            let mut reachable = true;
            let mut valid = true;
            for arm in &when.arms {
                this.push_scope();
                let head = this.prepare_when_head(arm, subject.as_ref(), &mut reachable);
                this.pop_scope();
                let Some(head) = head else {
                    valid = false;
                    continue;
                };
                let body = this.in_when_arm(&head, |this| this.lower_block(&arm.body));
                arms.push(hir::WhenArm {
                    condition: head.condition.clone(),
                    guard: head.guard.clone(),
                    body,
                    span: arm.span,
                });
                heads.push(head);
            }
            let else_body = when.else_body.as_ref().map(|body| this.lower_block(body));
            if !valid {
                return None;
            }
            let fallback = this.when_fallback(when, subject.as_ref(), &heads, else_body, false)?;
            Some(hir::StatementKind::When(hir::When {
                subject: subject.map(|subject| subject.value),
                arms,
                fallback,
            }))
        })
    }

    pub(crate) fn lower_when_expression(
        &mut self,
        when: &ast::When,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        self.in_when_scope(|this| this.lower_when_value(when, sink, expected))
    }

    fn in_when_scope<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        self.push_scope();
        let facts = self.smart_casts.clone();
        let result = f(self);
        self.smart_casts = facts;
        self.pop_scope();
        result
    }

    fn in_when_arm<T>(&mut self, head: &ArmHead, f: impl FnOnce(&mut Self) -> T) -> T {
        let scope = std::mem::replace(&mut self.scopes, head.scope.clone());
        let facts = std::mem::replace(&mut self.smart_casts, head.facts.clone());
        self.local_function_scopes.push();
        let result = f(self);
        self.local_function_scopes.pop();
        self.scopes = scope;
        self.smart_casts = facts;
        result
    }

    fn prepare_when_arms(
        &mut self,
        when: &ast::When,
        subject: Option<&Subject>,
    ) -> Option<(Vec<ArmHead>, bool)> {
        let mut heads = Vec::with_capacity(when.arms.len());
        let mut reachable = true;
        let mut valid = true;
        for arm in &when.arms {
            self.push_scope();
            let head = self.prepare_when_head(arm, subject, &mut reachable);
            self.pop_scope();
            match head {
                Some(head) => heads.push(head),
                None => valid = false,
            }
        }
        valid.then_some((heads, reachable))
    }

    fn when_fallback(
        &mut self,
        when: &ast::When,
        subject: Option<&Subject>,
        heads: &[ArmHead],
        else_body: Option<Vec<hir::Statement>>,
        value_position: bool,
    ) -> Option<hir::WhenFallback> {
        if let Some(body) = else_body {
            return Some(hir::WhenFallback::Else(body));
        }
        let Some(subject) = subject else {
            if value_position {
                self.error(
                    when.span,
                    "non-exhaustive when: a subjectless value when requires `else`".into(),
                );
                return None;
            }
            return Some(hir::WhenFallback::Fallthrough);
        };
        let has_case = heads
            .iter()
            .any(|head| matches!(head.condition, hir::WhenCondition::Case(_)));
        if !value_position
            && !has_case
            && !matches!(self.types[subject.value.ty], Type::Boolean | Type::Enum(_))
        {
            return Some(hir::WhenFallback::Fallthrough);
        }
        let patterns: Vec<_> = heads
            .iter()
            .filter(|head| head.guard.is_none())
            .flat_map(|head| head.coverage.iter().cloned())
            .collect();
        let guarded = heads.iter().any(|head| head.guard.is_some());
        self.prove_exhaustiveness(when.span, subject.value.ty, &patterns, guarded)
            .map(hir::WhenFallback::Impossible)
    }
}
