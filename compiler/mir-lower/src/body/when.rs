use super::*;

impl BodyLowerer<'_> {
    pub(super) fn lower_when(
        &mut self,
        when: &hir::When,
        span: Span,
        out: &mut Vec<smir::Statement>,
    ) {
        let subject = when.subject.as_ref().map(|value| {
            let ty = self.lower_type(value.ty);
            let init = self.lower_expr(value);
            self.drain_prelude(span, out);
            let local = self.new_hidden("when", ty.clone(), false);
            out.push(smir::Statement {
                kind: smir::StatementKind::ValDecl { local, init },
                span,
            });
            (local, ty)
        });
        out.extend(self.lower_arms(&when.arms, subject.as_ref(), &when.fallback, span));
    }

    /// Each false condition or guard enters the remaining decision sequence.
    /// Exhaustiveness affects only its final edge, never an arm's runtime test.
    fn lower_arms(
        &mut self,
        arms: &[hir::WhenArm],
        subject: Option<&(mir::LocalId, mir::Type)>,
        fallback: &hir::WhenFallback,
        fallback_span: Span,
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
        match &arm.condition {
            hir::WhenCondition::Always => {
                self.lower_matched_arm(arm, rest, subject, fallback, fallback_span)
            }
            hir::WhenCondition::Predicate(condition) => {
                let mut out = self.lower_statements(&condition.setup);
                let cond = self.lower_expr(&condition.condition);
                self.drain_prelude(arm.span, &mut out);
                let then_body = self.lower_matched_arm(arm, rest, subject, fallback, fallback_span);
                let next = self.lower_arms(rest, subject, fallback, fallback_span);
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
            hir::WhenCondition::Case(pattern) => {
                let (subject_local, subject_ty) = subject.expect("a case has a subject");
                let mut path = Vec::new();
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
                    &mut path,
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
                let mut then_body: Vec<_> = bindings
                    .into_iter()
                    .map(|(local, init)| smir::Statement {
                        kind: smir::StatementKind::ValDecl { local, init },
                        span: arm.span,
                    })
                    .collect();
                then_body.extend(self.lower_matched_arm(
                    arm,
                    rest,
                    subject,
                    fallback,
                    fallback_span,
                ));
                if steps.is_empty() {
                    return then_body;
                }
                let else_body = self.lower_arms(rest, subject, fallback, fallback_span);
                vec![smir::Statement {
                    kind: smir::StatementKind::PatternDecision(smir::PatternDecision {
                        steps,
                        then_body,
                        else_body,
                    }),
                    span: arm.span,
                }]
            }
        }
    }

    fn lower_matched_arm(
        &mut self,
        arm: &hir::WhenArm,
        rest: &[hir::WhenArm],
        subject: Option<&(mir::LocalId, mir::Type)>,
        fallback: &hir::WhenFallback,
        fallback_span: Span,
    ) -> Vec<smir::Statement> {
        let Some(guard) = &arm.guard else {
            return self.lower_statements(&arm.body);
        };
        let mut out = self.lower_statements(&guard.setup);
        let cond = self.lower_expr(&guard.condition);
        self.drain_prelude(arm.span, &mut out);
        let then_body = self.lower_statements(&arm.body);
        let next = self.lower_arms(rest, subject, fallback, fallback_span);
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
