use crate::expr::SmartCastFacts;

use super::*;

mod conditions;
mod coverage;
mod primary;
mod sealed;
mod subject;
mod values;

struct Subject {
    value: hir::Expr,
    reference: ast::Expr,
}

/// A checked head retains its bindings while result inference may postpone
/// the body. Conditions are resolved once, in source order.
struct ArmHead<C = hir::WhenCondition> {
    condition: C,
    guard: Option<hir::WhenGuard>,
    scope: Scopes,
    facts: SmartCastFacts,
    coverage: Vec<hir::Pattern>,
    context_requirement: ExpressionContextRequirement,
    body_reachable: bool,
}

type LoweredStatementArms<C> = (Vec<hir::WhenArm<C>>, Vec<ArmHead<C>>);

impl Lowerer {
    pub(super) fn lower_when(
        &mut self,
        when: &ast::When,
        out: &mut Vec<hir::Statement>,
    ) -> Option<hir::StatementKind> {
        self.in_when_scope(|this| {
            let result = match this.lower_when_subject(when, out)? {
                Some(subject) => {
                    let prepared = this.lower_statement_arms(when, |this, arm, reachable| {
                        this.prepare_when_head(arm, reachable, |this| {
                            this.subject_primary(arm, &subject)
                        })
                    });
                    let else_body = when.else_body.as_ref().map(|body| this.lower_block(body));
                    let (arms, heads) = prepared?;
                    let fallback =
                        this.subject_when_fallback(when, &subject, &heads, else_body, false)?;
                    hir::When::Subject {
                        subject: subject.value,
                        arms,
                        fallback,
                    }
                }
                None => {
                    let prepared = this.lower_statement_arms(when, |this, arm, reachable| {
                        this.prepare_when_head(arm, reachable, |this| this.conditional_primary(arm))
                    });
                    let else_body = when.else_body.as_ref().map(|body| this.lower_block(body));
                    let (arms, _) = prepared?;
                    let fallback = this.conditional_when_fallback(when, else_body, false)?;
                    hir::When::Conditions { arms, fallback }
                }
            };
            Some(hir::StatementKind::When(result))
        })
    }

    fn lower_statement_arms<C: Clone>(
        &mut self,
        when: &ast::When,
        mut prepare: impl FnMut(&mut Self, &ast::WhenArm, &mut bool) -> Option<ArmHead<C>>,
    ) -> Option<LoweredStatementArms<C>> {
        let mut heads = Vec::with_capacity(when.arms.len());
        let mut arms = Vec::with_capacity(when.arms.len());
        let mut reachable = true;
        let mut valid = true;
        for arm in &when.arms {
            self.push_scope();
            let head = prepare(self, arm, &mut reachable);
            self.pop_scope();
            let Some(head) = head else {
                valid = false;
                continue;
            };
            let body = self.in_when_arm(&head, |this| this.lower_block(&arm.body));
            arms.push(hir::WhenArm {
                condition: head.condition.clone(),
                guard: head.guard.clone(),
                body,
                span: arm.span,
            });
            heads.push(head);
        }
        valid.then_some((arms, heads))
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

    fn in_when_arm<C, T>(&mut self, head: &ArmHead<C>, f: impl FnOnce(&mut Self) -> T) -> T {
        let scope = std::mem::replace(&mut self.scopes, head.scope.clone());
        let facts = std::mem::replace(&mut self.smart_casts, head.facts.clone());
        self.local_function_scopes.push();
        let result = f(self);
        self.local_function_scopes.pop();
        self.scopes = scope;
        self.smart_casts = facts;
        result
    }

    fn prepare_when_arms<C>(
        &mut self,
        when: &ast::When,
        mut prepare: impl FnMut(&mut Self, &ast::WhenArm, &mut bool) -> Option<ArmHead<C>>,
    ) -> Option<(Vec<ArmHead<C>>, bool)> {
        let mut heads = Vec::with_capacity(when.arms.len());
        let mut reachable = true;
        let mut valid = true;
        for arm in &when.arms {
            self.push_scope();
            let head = prepare(self, arm, &mut reachable);
            self.pop_scope();
            match head {
                Some(head) => heads.push(head),
                None => valid = false,
            }
        }
        valid.then_some((heads, reachable))
    }

    fn subject_when_fallback(
        &mut self,
        when: &ast::When,
        subject: &Subject,
        heads: &[ArmHead],
        else_body: Option<Vec<hir::Statement>>,
        value_position: bool,
    ) -> Option<hir::WhenFallback> {
        if let Some(body) = else_body {
            return Some(hir::WhenFallback::Else(body));
        }
        if self.sealed_subtypes(subject.value.ty).is_some() {
            return self
                .prove_sealed_when(when, subject.value.ty)
                .map(hir::WhenFallback::Impossible);
        }
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
    fn conditional_when_fallback(
        &mut self,
        when: &ast::When,
        else_body: Option<Vec<hir::Statement>>,
        value_position: bool,
    ) -> Option<hir::WhenFallback<std::convert::Infallible>> {
        if let Some(body) = else_body {
            Some(hir::WhenFallback::Else(body))
        } else if value_position {
            self.error(
                when.span,
                "non-exhaustive when: a subjectless value when requires `else`".into(),
            );
            None
        } else {
            Some(hir::WhenFallback::Fallthrough)
        }
    }
}
