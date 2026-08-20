use super::*;

pub(super) struct ArmPrimary<C> {
    pub condition: C,
    pub paths: Option<(SmartCastFacts, SmartCastFacts)>,
    pub coverage: Vec<hir::Pattern>,
    pub completes: bool,
    pub irrefutable: bool,
}

impl Lowerer {
    pub(super) fn prepare_when_head<C>(
        &mut self,
        arm: &ast::WhenArm,
        reachable: &mut bool,
        prepare: impl FnOnce(&mut Self) -> Option<ArmPrimary<C>>,
    ) -> Option<ArmHead<C>> {
        let incoming = self.smart_casts.clone();
        let result = prepare(self)
            .and_then(|primary| self.finish_when_head(arm, primary, reachable, incoming.clone()));
        if result.is_none() {
            self.smart_casts = incoming;
        }
        result
    }

    fn finish_when_head<C>(
        &mut self,
        arm: &ast::WhenArm,
        primary: ArmPrimary<C>,
        reachable: &mut bool,
        incoming: SmartCastFacts,
    ) -> Option<ArmHead<C>> {
        let primary_completes = primary.completes;
        let irrefutable = primary.irrefutable;
        let (primary_true, primary_false) = primary
            .paths
            .unwrap_or_else(|| (incoming.clone(), incoming));
        self.smart_casts = primary_true;
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
        let next = if let Some(guard) = &guard {
            let guard_false = self.checked_condition_facts(&guard.condition, false);
            self.smart_casts = self.checked_condition_facts(&guard.condition, true);
            if irrefutable {
                guard_false
            } else {
                self.common_when_facts(&primary_false, &guard_false)
            }
        } else {
            primary_false
        };
        let head = ArmHead {
            condition: primary.condition,
            guard,
            scope: self.scopes.clone(),
            facts: self.smart_casts.clone(),
            coverage: primary.coverage,
            requires_expected: self.value_block_requires_expected(&arm.body),
            body_reachable: *reachable && primary_completes && guard_completes,
        };
        *reachable = *reachable
            && primary_completes
            && (!irrefutable || (arm.guard.is_some() && guard_completes));
        self.smart_casts = next;
        Some(head)
    }

    pub(super) fn when_predicate_completes(&self, predicate: &hir::WhenGuard) -> bool {
        self.statements_control_outcomes(&predicate.setup)
            .can_fall_through()
            && self.expression_can_complete(&predicate.condition)
    }

    fn common_when_facts(&mut self, lhs: &SmartCastFacts, rhs: &SmartCastFacts) -> SmartCastFacts {
        self.common_smart_casts(lhs.clone(), rhs.clone())
    }
}
