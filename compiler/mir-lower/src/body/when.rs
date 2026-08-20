use super::*;

mod conditions;
use conditions::ArmMatch;

impl BodyLowerer<'_> {
    pub(super) fn lower_when(
        &mut self,
        when: &hir::When,
        span: Span,
        out: &mut Vec<smir::Statement>,
    ) {
        match when {
            hir::When::Subject {
                subject,
                arms,
                fallback,
            } => {
                let ty = self.lower_type(subject.ty);
                let init = self.lower_expr(subject);
                self.drain_prelude(span, out);
                let local = self.new_hidden("when", ty.clone(), false);
                out.push(smir::Statement {
                    kind: smir::StatementKind::ValDecl { local, init },
                    span,
                });
                let subject = (local, ty);
                out.extend(
                    self.lower_arms(arms, fallback, span, |this, condition, span| {
                        this.lower_subject_condition(condition, &subject, span)
                    }),
                );
            }
            hir::When::Conditions { arms, fallback } => {
                out.extend(
                    self.lower_arms(
                        arms,
                        fallback,
                        span,
                        |this, condition, span| match condition {
                            hir::WhenPredicate::Predicate(predicate) => {
                                this.lower_when_predicate(predicate, span)
                            }
                            hir::WhenPredicate::Always => ArmMatch::Always,
                        },
                    ),
                );
            }
        }
    }

    /// Conditions and guards retain their source order; only the final edge
    /// uses the checked fallback category.
    fn lower_arms<C, P>(
        &mut self,
        arms: &[hir::WhenArm<C>],
        fallback: &hir::WhenFallback<P>,
        fallback_span: Span,
        condition: impl Copy + Fn(&mut Self, &C, Span) -> ArmMatch,
    ) -> Vec<smir::Statement> {
        let Some((arm, rest)) = arms.split_first() else {
            return match fallback {
                hir::WhenFallback::Else(body) => self.lower_statements(body),
                hir::WhenFallback::Fallthrough => Vec::new(),
                hir::WhenFallback::Impossible(_) => vec![smir::Statement {
                    kind: smir::StatementKind::Unreachable,
                    span: fallback_span,
                }],
            };
        };
        match condition(self, &arm.condition, arm.span) {
            ArmMatch::Always => {
                self.lower_matched_arm(arm, rest, fallback, fallback_span, condition)
            }
            ArmMatch::Predicate { mut setup, value } => {
                let then_body =
                    self.lower_matched_arm(arm, rest, fallback, fallback_span, condition);
                let next = self.lower_arms(rest, fallback, fallback_span, condition);
                setup.push(smir::Statement {
                    kind: smir::StatementKind::If {
                        cond: value,
                        then_body,
                        else_body: non_empty(next),
                    },
                    span: arm.span,
                });
                setup
            }
            ArmMatch::Pattern {
                steps,
                mut bindings,
            } => {
                bindings.extend(self.lower_matched_arm(
                    arm,
                    rest,
                    fallback,
                    fallback_span,
                    condition,
                ));
                if steps.is_empty() {
                    return bindings;
                }
                let else_body = self.lower_arms(rest, fallback, fallback_span, condition);
                vec![smir::Statement {
                    kind: smir::StatementKind::PatternDecision(smir::PatternDecision {
                        steps,
                        then_body: bindings,
                        else_body,
                    }),
                    span: arm.span,
                }]
            }
        }
    }

    fn lower_matched_arm<C, P>(
        &mut self,
        arm: &hir::WhenArm<C>,
        rest: &[hir::WhenArm<C>],
        fallback: &hir::WhenFallback<P>,
        fallback_span: Span,
        condition: impl Copy + Fn(&mut Self, &C, Span) -> ArmMatch,
    ) -> Vec<smir::Statement> {
        let Some(guard) = &arm.guard else {
            return self.lower_statements(&arm.body);
        };
        let mut out = self.lower_statements(&guard.setup);
        let cond = self.lower_expr(&guard.condition);
        self.drain_prelude(arm.span, &mut out);
        let then_body = self.lower_statements(&arm.body);
        let next = self.lower_arms(rest, fallback, fallback_span, condition);
        out.push(smir::Statement {
            kind: smir::StatementKind::If {
                cond,
                then_body,
                else_body: non_empty(next),
            },
            span: arm.span,
        });
        out
    }
}
