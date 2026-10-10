use super::*;

impl Lowerer {
    pub(super) fn prepare_when_head(
        &mut self,
        arm: &ast::WhenArm,
        subject: Option<&Subject>,
        reachable: &mut bool,
    ) -> Option<ArmHead> {
        let incoming = self.smart_casts.clone();
        let result = self.prepare_when_head_inner(arm, subject, reachable);
        if result.is_none() {
            self.smart_casts = incoming;
        }
        result
    }

    fn prepare_when_head_inner(
        &mut self,
        arm: &ast::WhenArm,
        subject: Option<&Subject>,
        reachable: &mut bool,
    ) -> Option<ArmHead> {
        let incoming = self.smart_casts.clone();
        let (condition, source, coverage) = match &arm.condition {
            ast::WhenArmCondition::Case(pattern) => {
                let subject = subject.expect("the parser requires a case subject");
                if !matches!(
                    self.types[subject.value.ty],
                    Type::Enum(_)
                        | Type::Tuple(_)
                        | Type::Struct(_)
                        | Type::Integer(_)
                        | Type::String
                        | Type::Boolean
                        | Type::Unit
                ) {
                    let found = self.type_name(subject.value.ty);
                    self.error(subject.value.span, format!("case subject must be an enum, tuple, struct or scalar value, found {found}"));
                    return None;
                }
                let pattern = self.lower_pattern(
                    pattern,
                    subject.value.ty,
                    PatternCtx {
                        mutable: false,
                        in_when: true,
                    },
                )?;
                (
                    hir::WhenCondition::Case(pattern.clone()),
                    None,
                    vec![pattern],
                )
            }
            ast::WhenArmCondition::Else => (hir::WhenCondition::Always, None, Vec::new()),
            ast::WhenArmCondition::Conditions(conditions) => {
                let source = conditions
                    .iter()
                    .map(|condition| condition_expression(condition, subject))
                    .reduce(|lhs, rhs| {
                        let span = ast::Span {
                            start: lhs.span().start,
                            end: rhs.span().end,
                        };
                        ast::Expr::Binary {
                            op: ast::BinOp::Or,
                            lhs: Box::new(lhs),
                            rhs: Box::new(rhs),
                            span,
                        }
                    })
                    .expect("ordinary conditions are nonempty");
                let mut setup = Vec::new();
                let condition = self.lower_condition(&source, "when", &mut setup)?;
                let coverage = subject
                    .map(|subject| self.ordinary_when_coverage(conditions, subject.value.ty))
                    .unwrap_or_default();
                (
                    hir::WhenCondition::Predicate(hir::WhenGuard { setup, condition }),
                    Some(source),
                    coverage,
                )
            }
        };
        let primary_completes = match &condition {
            hir::WhenCondition::Predicate(condition) => self.when_predicate_completes(condition),
            _ => true,
        };
        let irrefutable = match &condition {
            hir::WhenCondition::Case(pattern) => crate::patterns::is_irrefutable(pattern),
            hir::WhenCondition::Always => true,
            hir::WhenCondition::Predicate(_) => false,
        };
        let primary_false = source
            .as_ref()
            .map(|source| self.when_path_facts(&incoming, source, false))
            .unwrap_or_else(|| incoming.clone());
        if let Some(source) = &source {
            self.smart_casts = self.when_path_facts(&incoming, source, true);
        }
        let primary_true = self.smart_casts.clone();
        let guard = arm.guard.as_ref().map(|source| {
            let mut setup = Vec::new();
            let condition = self.lower_expr(source, &mut setup, None)?;
            if !self.is_subtype(condition.ty, self.boolean) {
                let found = self.type_name(condition.ty);
                self.error(
                    source.span(),
                    format!("when guard must be Boolean, found {found}"),
                );
                return None;
            }
            Some(hir::WhenGuard { setup, condition })
        });
        let guard = match guard {
            Some(guard) => Some(guard?),
            None => None,
        };
        let guard_completes = guard
            .as_ref()
            .is_none_or(|guard| self.when_predicate_completes(guard));
        let next = if let Some(source) = &arm.guard {
            let guard_false = self.when_path_facts(&primary_true, source, false);
            self.smart_casts = self.when_path_facts(&primary_true, source, true);
            if irrefutable {
                guard_false
            } else {
                self.common_when_facts(&primary_false, &guard_false)
            }
        } else {
            primary_false
        };
        let head = ArmHead {
            condition,
            guard,
            scope: self.scopes.clone(),
            facts: self.smart_casts.clone(),
            coverage,
            requires_expected: self.value_block_requires_expected(&arm.body),
            body_reachable: *reachable && primary_completes && guard_completes,
        };
        *reachable = *reachable
            && primary_completes
            && (!irrefutable || (arm.guard.is_some() && guard_completes));
        self.smart_casts = next;
        Some(head)
    }

    fn when_predicate_completes(&self, predicate: &hir::WhenGuard) -> bool {
        self.statements_control_outcomes(&predicate.setup)
            .can_fall_through()
            && self.expression_can_complete(&predicate.condition)
    }

    fn when_path_facts(
        &mut self,
        incoming: &SmartCastFacts,
        source: &ast::Expr,
        outcome: bool,
    ) -> SmartCastFacts {
        let added = self.resolve_smart_casts(source, outcome);
        self.join_smart_casts(incoming.clone(), added)
    }

    fn common_when_facts(&mut self, lhs: &SmartCastFacts, rhs: &SmartCastFacts) -> SmartCastFacts {
        self.common_smart_casts(lhs.clone(), rhs.clone())
    }
}

fn condition_expression(condition: &ast::WhenCondition, subject: Option<&Subject>) -> ast::Expr {
    match condition {
        ast::WhenCondition::Expression(value) => match subject {
            Some(subject) => ast::Expr::Binary {
                op: ast::BinOp::Eq,
                lhs: Box::new(subject.reference.clone()),
                rhs: Box::new(value.clone()),
                span: value.span(),
            },
            None => value.clone(),
        },
        ast::WhenCondition::Is { ty, negated, span } => ast::Expr::Is {
            operand: Box::new(subject.expect("is has a subject").reference.clone()),
            ty: ty.clone(),
            negated: *negated,
            span: *span,
        },
        ast::WhenCondition::In {
            collection,
            negated,
            span,
        } => ast::Expr::Binary {
            op: if *negated {
                ast::BinOp::NotContains
            } else {
                ast::BinOp::Contains
            },
            lhs: Box::new(subject.expect("in has a subject").reference.clone()),
            rhs: Box::new(collection.clone()),
            span: *span,
        },
    }
}
