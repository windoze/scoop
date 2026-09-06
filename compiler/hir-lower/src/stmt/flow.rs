use std::collections::BTreeSet;

use super::*;

/// One way that control can leave a structured statement region.
///
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ControlOutcome<Target> {
    Fallthrough,
    Return,
    Throw,
    Break(Target),
    Continue(Target),
}

/// The compositional set of control outcomes for one structured region.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ControlOutcomes<Target> {
    values: BTreeSet<ControlOutcome<Target>>,
}

impl<Target: Ord> ControlOutcomes<Target> {
    fn empty() -> Self {
        Self {
            values: BTreeSet::new(),
        }
    }

    fn singleton(outcome: ControlOutcome<Target>) -> Self {
        std::iter::once(outcome).collect()
    }

    fn fallthrough() -> Self {
        Self::singleton(ControlOutcome::Fallthrough)
    }

    fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub(crate) fn can_fall_through(&self) -> bool {
        self.values.contains(&ControlOutcome::Fallthrough)
    }

    fn union(mut self, other: Self) -> Self {
        self.values.extend(other.values);
        self
    }

    /// Consume the jumps owned by `target`, returning whether at least one
    /// break makes the loop's exit reachable. Continue is a header edge and
    /// therefore contributes no escaping outcome.
    fn consume_loop_jumps(&mut self, target: Target) -> bool
    where
        Target: Copy,
    {
        let exits = self.values.remove(&ControlOutcome::Break(target));
        self.values.remove(&ControlOutcome::Continue(target));
        exits
    }

    /// Sequence two regions. Only an incoming fallthrough reaches `next`;
    /// every already-abrupt outcome is preserved unchanged.
    fn then(mut self, next: Self) -> Self {
        if self.values.remove(&ControlOutcome::Fallthrough) {
            self.values.extend(next.values);
        }
        self
    }

    /// Run `finally` once for every reachable incoming outcome. A normally
    /// completing finally restores the incoming outcome; its abrupt outcomes
    /// replace it. An unreachable incoming region cannot manufacture a
    /// finally outcome.
    fn apply_finally(self, mut finally: Self) -> Self {
        if self.is_empty() {
            return Self::empty();
        }
        let resumes_incoming = finally.values.remove(&ControlOutcome::Fallthrough);
        let mut result = if resumes_incoming {
            self
        } else {
            Self::empty()
        };
        result.values.extend(finally.values);
        result
    }
}

impl<Target: Ord> FromIterator<ControlOutcome<Target>> for ControlOutcomes<Target> {
    fn from_iter<T: IntoIterator<Item = ControlOutcome<Target>>>(iter: T) -> Self {
        Self {
            values: iter.into_iter().collect(),
        }
    }
}

pub(crate) type HirControlOutcomes = ControlOutcomes<hir::LoopId>;

/// Analyze a statement list. Once no fallthrough remains, later statements
/// are unreachable and cannot add outcomes back to the list.
pub(crate) fn statements_control_outcomes(statements: &[hir::Statement]) -> HirControlOutcomes {
    let mut outcomes = HirControlOutcomes::fallthrough();
    for statement in statements {
        if !outcomes.can_fall_through() {
            break;
        }
        outcomes = outcomes.then(statement_control_outcomes(statement));
    }
    outcomes
}

fn statement_control_outcomes(statement: &hir::Statement) -> HirControlOutcomes {
    match &statement.kind {
        hir::StatementKind::Return { .. } => HirControlOutcomes::singleton(ControlOutcome::Return),
        hir::StatementKind::Throw(_) => HirControlOutcomes::singleton(ControlOutcome::Throw),
        hir::StatementKind::Break { target } => {
            HirControlOutcomes::singleton(ControlOutcome::Break(*target))
        }
        hir::StatementKind::Continue { target } => {
            HirControlOutcomes::singleton(ControlOutcome::Continue(*target))
        }
        hir::StatementKind::If {
            then_body,
            else_body,
            ..
        } => statements_control_outcomes(then_body).union(
            else_body
                .as_deref()
                .map_or_else(HirControlOutcomes::fallthrough, statements_control_outcomes),
        ),
        hir::StatementKind::When(when) => when_control_outcomes(when),
        hir::StatementKind::Try(try_) => {
            let mut incoming = statements_control_outcomes(&try_.body);
            for catch in &try_.catches {
                incoming = incoming.union(statements_control_outcomes(&catch.body));
            }
            match try_.finally_body.as_deref() {
                Some(body) => incoming.apply_finally(statements_control_outcomes(body)),
                None => incoming,
            }
        }
        hir::StatementKind::While {
            target,
            condition_setup,
            body,
            ..
        } => {
            let mut setup = statements_control_outcomes(condition_setup);
            let reaches_condition = setup.values.remove(&ControlOutcome::Fallthrough);
            let setup_breaks = setup.consume_loop_jumps(*target);

            let mut result = setup;
            if setup_breaks {
                result.values.insert(ControlOutcome::Fallthrough);
            }

            if reaches_condition {
                // A completed condition may be false before the first
                // iteration, so the loop conservatively falls through. Body
                // fallthrough and continue are backedges; a matching break is
                // another path to the same exit.
                result.values.insert(ControlOutcome::Fallthrough);
                let mut body_outcomes = statements_control_outcomes(body);
                body_outcomes.values.remove(&ControlOutcome::Fallthrough);
                body_outcomes.consume_loop_jumps(*target);
                result = result.union(body_outcomes);
            }
            result
        }
        hir::StatementKind::Expr(_)
        | hir::StatementKind::InitializationEnsure(_)
        | hir::StatementKind::LocalFunction(_)
        | hir::StatementKind::ValDecl { .. }
        | hir::StatementKind::Assign { .. } => HirControlOutcomes::fallthrough(),
    }
}

/// Fold ordered `when` arms from the fallback edge. This mirrors MIR's
/// first-match lowering: an unguarded irrefutable pattern cuts off later arms,
/// while a guard's setup must complete before either the body or next arm is
/// reachable.
fn when_control_outcomes(when: &hir::When) -> HirControlOutcomes {
    let mut next = match &when.fallback {
        hir::WhenFallback::Else(body) => statements_control_outcomes(body),
        hir::WhenFallback::Impossible(_) => HirControlOutcomes::empty(),
    };

    for arm in when.arms.iter().rev() {
        let body = statements_control_outcomes(&arm.body);
        let matched = match &arm.guard {
            Some(guard) => statements_control_outcomes(&guard.setup).then(body.union(next.clone())),
            None => body,
        };
        next = if crate::patterns::is_irrefutable(&arm.pattern) {
            matched
        } else {
            matched.union(next)
        };
    }
    next
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
    struct TestLoop(u8);

    fn outcomes<const N: usize>(
        values: [ControlOutcome<TestLoop>; N],
    ) -> ControlOutcomes<TestLoop> {
        values.into_iter().collect()
    }

    fn span() -> Span {
        Span::new(0, 0)
    }

    fn expression() -> hir::Expr {
        hir::Expr {
            kind: hir::ExprKind::UnitLiteral,
            ty: hir::TypeId::from_raw(0_u32.into()),
            span: span(),
            origin: hir::ExpressionOrigin::Definition(hir::DefinitionOrigin {
                provider: hir::IntrinsicProviderId::from_raw(0),
                file: 0,
                span: span(),
                context: hir::SourceContextId::from_raw(0_u32.into()),
            }),
        }
    }

    fn statement(kind: hir::StatementKind) -> hir::Statement {
        hir::Statement { kind, span: span() }
    }

    fn return_statement() -> hir::Statement {
        statement(hir::StatementKind::Return { value: None })
    }

    fn throw_statement() -> hir::Statement {
        statement(hir::StatementKind::Throw(expression()))
    }

    fn assert_hir_outcomes(
        statements: &[hir::Statement],
        expected: impl IntoIterator<Item = ControlOutcome<hir::LoopId>>,
    ) {
        assert_eq!(
            statements_control_outcomes(statements),
            expected.into_iter().collect()
        );
    }

    #[test]
    fn sequence_only_advances_fallthrough() {
        assert_hir_outcomes(
            &[return_statement(), throw_statement()],
            [ControlOutcome::Return],
        );

        let partial_return = statement(hir::StatementKind::If {
            cond: expression(),
            then_body: vec![return_statement()],
            else_body: None,
        });
        assert_hir_outcomes(
            &[partial_return, throw_statement()],
            [ControlOutcome::Return, ControlOutcome::Throw],
        );
    }

    #[test]
    fn branch_outcomes_form_a_set() {
        let branch = statement(hir::StatementKind::If {
            cond: expression(),
            then_body: vec![return_statement()],
            else_body: Some(vec![throw_statement()]),
        });
        assert_hir_outcomes(&[branch], [ControlOutcome::Return, ControlOutcome::Throw]);
    }

    #[test]
    fn while_setup_must_complete_before_body_or_false_exit() {
        let target = hir::LoopId::from_raw(0);
        let abrupt_setup = statement(hir::StatementKind::While {
            target,
            condition_setup: vec![return_statement()],
            cond: expression(),
            body: vec![throw_statement()],
        });
        assert_hir_outcomes(&[abrupt_setup], [ControlOutcome::Return]);

        let reachable_condition = statement(hir::StatementKind::While {
            target,
            condition_setup: Vec::new(),
            cond: expression(),
            body: vec![throw_statement()],
        });
        assert_hir_outcomes(
            &[reachable_condition],
            [ControlOutcome::Fallthrough, ControlOutcome::Throw],
        );

        let mixed_setup = statement(hir::StatementKind::While {
            target,
            condition_setup: vec![statement(hir::StatementKind::If {
                cond: expression(),
                then_body: vec![return_statement()],
                else_body: None,
            })],
            cond: expression(),
            body: vec![throw_statement()],
        });
        assert_hir_outcomes(
            &[mixed_setup],
            [
                ControlOutcome::Fallthrough,
                ControlOutcome::Return,
                ControlOutcome::Throw,
            ],
        );
    }

    #[test]
    fn loop_targets_are_consumed_from_while_setup_and_body() {
        let target = hir::LoopId::from_raw(0);
        let setup_break = statement(hir::StatementKind::While {
            target,
            condition_setup: vec![statement(hir::StatementKind::Break { target })],
            cond: expression(),
            body: vec![throw_statement()],
        });
        assert_hir_outcomes(&[setup_break], [ControlOutcome::Fallthrough]);

        let setup_continue = statement(hir::StatementKind::While {
            target,
            condition_setup: vec![statement(hir::StatementKind::Continue { target })],
            cond: expression(),
            body: vec![throw_statement()],
        });
        assert_hir_outcomes(&[setup_continue], []);

        let body_jumps = statement(hir::StatementKind::While {
            target,
            condition_setup: Vec::new(),
            cond: expression(),
            body: vec![statement(hir::StatementKind::If {
                cond: expression(),
                then_body: vec![statement(hir::StatementKind::Break { target })],
                else_body: Some(vec![statement(hir::StatementKind::Continue { target })]),
            })],
        });
        assert_hir_outcomes(&[body_jumps], [ControlOutcome::Fallthrough]);
    }

    #[test]
    fn inner_loop_target_break_in_finally_preserves_the_pending_outer_jump() {
        let outer = hir::LoopId::from_raw(0);
        let inner = hir::LoopId::from_raw(1);
        let transfer = statement(hir::StatementKind::Try(hir::Try {
            body: vec![statement(hir::StatementKind::Break { target: outer })],
            catches: Vec::new(),
            finally_body: Some(vec![statement(hir::StatementKind::While {
                target: inner,
                condition_setup: Vec::new(),
                cond: expression(),
                body: vec![statement(hir::StatementKind::Break { target: inner })],
            })]),
        }));

        assert_hir_outcomes(&[transfer], [ControlOutcome::Break(outer)]);
    }

    #[test]
    fn when_stops_after_an_unguarded_irrefutable_arm() {
        let when = statement(hir::StatementKind::When(hir::When {
            subject: expression(),
            arms: vec![
                hir::WhenArm {
                    pattern: hir::Pattern::Wildcard,
                    guard: None,
                    body: vec![return_statement()],
                    span: span(),
                },
                hir::WhenArm {
                    pattern: hir::Pattern::Wildcard,
                    guard: None,
                    body: vec![throw_statement()],
                    span: span(),
                },
            ],
            fallback: hir::WhenFallback::Else(Vec::new()),
        }));
        assert_hir_outcomes(&[when], [ControlOutcome::Return]);
    }

    #[test]
    fn refutable_when_arm_unions_its_body_with_the_fallback() {
        let when = statement(hir::StatementKind::When(hir::When {
            subject: expression(),
            arms: vec![hir::WhenArm {
                pattern: hir::Pattern::Variant {
                    application: hir::EnumApplicationId::from_raw(0_u32.into()),
                    variant: 0,
                    fields: Vec::new(),
                },
                guard: None,
                body: vec![return_statement()],
                span: span(),
            }],
            fallback: hir::WhenFallback::Else(vec![throw_statement()]),
        }));
        assert_hir_outcomes(&[when], [ControlOutcome::Return, ControlOutcome::Throw]);
    }

    #[test]
    fn when_guard_setup_controls_body_and_next_arm_reachability() {
        let abrupt_guard = statement(hir::StatementKind::When(hir::When {
            subject: expression(),
            arms: vec![hir::WhenArm {
                pattern: hir::Pattern::Wildcard,
                guard: Some(hir::WhenGuard {
                    setup: vec![return_statement()],
                    condition: expression(),
                }),
                body: vec![throw_statement()],
                span: span(),
            }],
            fallback: hir::WhenFallback::Else(Vec::new()),
        }));
        assert_hir_outcomes(&[abrupt_guard], [ControlOutcome::Return]);

        let completing_guard = statement(hir::StatementKind::When(hir::When {
            subject: expression(),
            arms: vec![hir::WhenArm {
                pattern: hir::Pattern::Wildcard,
                guard: Some(hir::WhenGuard {
                    setup: Vec::new(),
                    condition: expression(),
                }),
                body: vec![return_statement()],
                span: span(),
            }],
            fallback: hir::WhenFallback::Else(vec![throw_statement()]),
        }));
        assert_hir_outcomes(
            &[completing_guard],
            [ControlOutcome::Return, ControlOutcome::Throw],
        );

        let mixed_guard = statement(hir::StatementKind::When(hir::When {
            subject: expression(),
            arms: vec![hir::WhenArm {
                pattern: hir::Pattern::Wildcard,
                guard: Some(hir::WhenGuard {
                    setup: vec![statement(hir::StatementKind::If {
                        cond: expression(),
                        then_body: vec![return_statement()],
                        else_body: None,
                    })],
                    condition: expression(),
                }),
                body: vec![throw_statement()],
                span: span(),
            }],
            fallback: hir::WhenFallback::Else(Vec::new()),
        }));
        assert_hir_outcomes(
            &[mixed_guard],
            [
                ControlOutcome::Fallthrough,
                ControlOutcome::Return,
                ControlOutcome::Throw,
            ],
        );
    }

    #[test]
    fn try_merges_catches_before_applying_finally() {
        let try_ = statement(hir::StatementKind::Try(hir::Try {
            body: vec![return_statement()],
            catches: vec![hir::CatchClause {
                local: hir::LocalId::from_raw(0_u32.into()),
                ty: hir::TypeId::from_raw(0_u32.into()),
                body: vec![throw_statement()],
                span: span(),
            }],
            finally_body: Some(Vec::new()),
        }));
        assert_hir_outcomes(&[try_], [ControlOutcome::Return, ControlOutcome::Throw]);

        let overriding_finally = statement(hir::StatementKind::Try(hir::Try {
            body: vec![throw_statement()],
            catches: Vec::new(),
            finally_body: Some(vec![return_statement()]),
        }));
        assert_hir_outcomes(&[overriding_finally], [ControlOutcome::Return]);
    }

    #[test]
    fn finally_restores_or_overrides_each_incoming_outcome() {
        let incoming = outcomes([
            ControlOutcome::Fallthrough,
            ControlOutcome::Return,
            ControlOutcome::Break(TestLoop(1)),
        ]);
        let mixed_finally = outcomes([
            ControlOutcome::Fallthrough,
            ControlOutcome::Throw,
            ControlOutcome::Continue(TestLoop(2)),
        ]);
        assert_eq!(
            incoming.clone().apply_finally(mixed_finally),
            outcomes([
                ControlOutcome::Fallthrough,
                ControlOutcome::Return,
                ControlOutcome::Throw,
                ControlOutcome::Break(TestLoop(1)),
                ControlOutcome::Continue(TestLoop(2)),
            ])
        );
        assert_eq!(
            incoming.apply_finally(outcomes([ControlOutcome::Return])),
            outcomes([ControlOutcome::Return])
        );
    }

    #[test]
    fn unreachable_incoming_does_not_manufacture_finally_outcomes() {
        assert_eq!(
            ControlOutcomes::<TestLoop>::empty().apply_finally(outcomes([ControlOutcome::Throw])),
            ControlOutcomes::empty()
        );
        assert_eq!(
            ControlOutcomes::<TestLoop>::empty().then(outcomes([ControlOutcome::Return])),
            ControlOutcomes::empty()
        );
    }

    #[test]
    fn typed_jumps_survive_sequence_until_their_loop_consumes_them() {
        let incoming = outcomes([
            ControlOutcome::Fallthrough,
            ControlOutcome::Break(TestLoop(1)),
            ControlOutcome::Continue(TestLoop(2)),
        ]);
        assert_eq!(
            incoming.then(outcomes([ControlOutcome::Return])),
            outcomes([
                ControlOutcome::Return,
                ControlOutcome::Break(TestLoop(1)),
                ControlOutcome::Continue(TestLoop(2)),
            ])
        );
    }
}
