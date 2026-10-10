use super::conditions::ArmPrimary;
use super::*;

impl Lowerer {
    pub(super) fn subject_primary(
        &mut self,
        arm: &ast::WhenArm,
        subject: &Subject,
    ) -> Option<ArmPrimary<hir::WhenCondition>> {
        match &arm.condition {
            ast::WhenArmCondition::Case(pattern) => {
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
                Some(ArmPrimary {
                    irrefutable: crate::patterns::is_irrefutable(&pattern),
                    condition: hir::WhenCondition::Case(pattern.clone()),
                    source: None,
                    coverage: vec![pattern],
                    completes: true,
                })
            }
            ast::WhenArmCondition::Else => Some(ArmPrimary {
                condition: hir::WhenCondition::Always,
                source: None,
                coverage: Vec::new(),
                completes: true,
                irrefutable: true,
            }),
            ast::WhenArmCondition::Conditions(conditions) => {
                let source = combine_conditions(
                    conditions
                        .iter()
                        .map(|condition| subject_condition(condition, subject)),
                );
                let predicate = self.lower_when_predicate(&source)?;
                Some(ArmPrimary {
                    completes: self.when_predicate_completes(&predicate),
                    irrefutable: false,
                    condition: hir::WhenCondition::Predicate(predicate),
                    source: Some(source),
                    coverage: self.ordinary_when_coverage(conditions, subject.value.ty),
                })
            }
        }
    }

    pub(super) fn conditional_primary(
        &mut self,
        arm: &ast::WhenArm,
    ) -> Option<ArmPrimary<hir::WhenPredicate>> {
        match &arm.condition {
            ast::WhenArmCondition::Case(_) => {
                self.error(arm.span, "case requires a when subject".into());
                None
            }
            ast::WhenArmCondition::Else => Some(ArmPrimary {
                condition: hir::WhenPredicate::Always,
                source: None,
                coverage: Vec::new(),
                completes: true,
                irrefutable: true,
            }),
            ast::WhenArmCondition::Conditions(conditions) => {
                let sources = conditions
                    .iter()
                    .map(|condition| match condition {
                        ast::WhenCondition::Expression(value) => Some(value.clone()),
                        ast::WhenCondition::Is { span, .. }
                        | ast::WhenCondition::In { span, .. } => {
                            self.error(*span, "when is/in conditions require a subject".into());
                            None
                        }
                    })
                    .collect::<Option<Vec<_>>>()?;
                let source = combine_conditions(sources.into_iter());
                let predicate = self.lower_when_predicate(&source)?;
                Some(ArmPrimary {
                    completes: self.when_predicate_completes(&predicate),
                    irrefutable: false,
                    condition: hir::WhenPredicate::Predicate(predicate),
                    source: Some(source),
                    coverage: Vec::new(),
                })
            }
        }
    }

    fn lower_when_predicate(&mut self, source: &ast::Expr) -> Option<hir::WhenGuard> {
        let mut setup = Vec::new();
        let condition = self.lower_condition(source, "when", &mut setup)?;
        Some(hir::WhenGuard { setup, condition })
    }
}

fn combine_conditions(conditions: impl Iterator<Item = ast::Expr>) -> ast::Expr {
    conditions
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
        .expect("ordinary conditions are nonempty")
}

fn subject_condition(condition: &ast::WhenCondition, subject: &Subject) -> ast::Expr {
    match condition {
        ast::WhenCondition::Expression(value) => ast::Expr::Binary {
            op: ast::BinOp::Eq,
            lhs: Box::new(subject.reference.clone()),
            rhs: Box::new(value.clone()),
            span: value.span(),
        },
        ast::WhenCondition::Is { ty, negated, span } => ast::Expr::Is {
            operand: Box::new(subject.reference.clone()),
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
            lhs: Box::new(subject.reference.clone()),
            rhs: Box::new(collection.clone()),
            span: *span,
        },
    }
}
