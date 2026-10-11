use super::*;

pub(super) enum ArmMatch {
    Always,
    Predicate {
        setup: Vec<smir::Statement>,
        value: smir::Expr,
    },
    Pattern {
        steps: Vec<smir::PatternDecisionStep>,
        bindings: Vec<smir::Statement>,
    },
}

impl BodyLowerer<'_> {
    pub(super) fn lower_when_predicate(
        &mut self,
        predicate: &hir::WhenGuard,
        span: Span,
    ) -> ArmMatch {
        let mut setup = self.lower_statements(&predicate.setup);
        let value = self.lower_expr(&predicate.condition);
        self.drain_prelude(span, &mut setup);
        ArmMatch::Predicate { setup, value }
    }

    pub(super) fn lower_subject_condition(
        &mut self,
        condition: &hir::WhenCondition,
        subject: &(mir::LocalId, mir::Type),
        span: Span,
    ) -> ArmMatch {
        match condition {
            hir::WhenCondition::Always => ArmMatch::Always,
            hir::WhenCondition::Predicate(predicate) => self.lower_when_predicate(predicate, span),
            hir::WhenCondition::Case(pattern) => {
                let (subject_local, subject_ty) = subject;
                let mut steps = Vec::new();
                let mut bindings = Vec::new();
                let decision_subject = if pattern_requires_stable_variant_subject(pattern) {
                    self.new_hidden("pattern.subject", subject_ty.clone(), false)
                } else {
                    *subject_local
                };
                self.lower_pattern(
                    pattern,
                    decision_subject,
                    &mut Vec::new(),
                    subject_ty,
                    &mut steps,
                    &mut bindings,
                );
                if decision_subject != *subject_local {
                    steps.insert(
                        0,
                        smir::PatternDecisionStep::Materialize {
                            local: decision_subject,
                            init: smir::Expr::local(*subject_local, subject_ty.clone()),
                        },
                    );
                }
                ArmMatch::Pattern {
                    steps,
                    bindings: bindings
                        .into_iter()
                        .map(|(local, init)| smir::Statement {
                            kind: smir::StatementKind::ValDecl { local, init },
                            span,
                        })
                        .collect(),
                }
            }
        }
    }
}
