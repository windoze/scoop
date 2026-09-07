use std::collections::HashSet;

use super::*;

impl Validator<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn definition_while(
        &self,
        target: LoopId,
        condition_setup: &[Statement],
        cond: &Expr,
        body: &[Statement],
        available: HashSet<u32>,
        check_reads: bool,
        reachable: bool,
        known: &HashSet<u32>,
        owners: &mut DefinitionOwners,
        owner: DefinitionOwner,
    ) -> Check<DefinitionFlow> {
        let condition = self.definition_statements(
            condition_setup,
            DefinitionFlow::falling_through(available.clone()),
            check_reads,
            reachable,
            known,
            owners,
            owner,
        )?;
        if condition.falls_through {
            self.check_expression_reads(cond, &condition.available, check_reads, known)?;
        }
        let body_flow = self.definition_statements(
            body,
            DefinitionFlow::falling_through(condition.available.clone()),
            check_reads,
            reachable && condition.falls_through,
            known,
            owners,
            owner,
        )?;

        let break_outcome = AbruptOutcome::Break(target.into_raw());
        let continue_outcome = AbruptOutcome::Continue(target.into_raw());
        let mut abrupt = condition.abrupt;
        let setup_breaks = abrupt.remove(&break_outcome);
        abrupt.remove(&continue_outcome);
        if condition.falls_through {
            let mut body_abrupt = body_flow.abrupt;
            body_abrupt.remove(&break_outcome);
            body_abrupt.remove(&continue_outcome);
            abrupt.extend(body_abrupt);
        }

        Ok(DefinitionFlow {
            // Definitions created inside either loop region are scoped to the
            // loop. A consumed break therefore conservatively restores only
            // the incoming definition set.
            available,
            falls_through: reachable && (condition.falls_through || setup_breaks),
            abrupt,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn definition_when(
        &self,
        value: &When,
        available: HashSet<u32>,
        check_reads: bool,
        reachable: bool,
        known: &HashSet<u32>,
        owners: &mut DefinitionOwners,
        owner: DefinitionOwner,
    ) -> Check<DefinitionFlow> {
        self.check_expression_reads(&value.subject, &available, check_reads && reachable, known)?;
        let mut normal_paths = Vec::new();
        let mut guard_abrupt = HashSet::new();
        let mut next_reachable = reachable;
        for arm in &value.arms {
            self.check_pattern_reads(
                &arm.pattern,
                &available,
                check_reads && next_reachable,
                known,
            )?;
            let mut arm_available = available.clone();
            self.insert_pattern_definitions(&arm.pattern, &mut arm_available, owners, owner)?;
            let arm_reachable = next_reachable;
            let (body_reachable, can_try_next, body_available, abrupt) = match &arm.guard {
                Some(guard) => {
                    let guard_setup = self.definition_statements(
                        &guard.setup,
                        DefinitionFlow::falling_through(arm_available),
                        check_reads,
                        arm_reachable,
                        known,
                        owners,
                        owner,
                    )?;
                    if guard_setup.falls_through {
                        self.check_expression_reads(
                            &guard.condition,
                            &guard_setup.available,
                            check_reads,
                            known,
                        )?;
                    }
                    (
                        guard_setup.falls_through,
                        !pattern_is_irrefutable(&arm.pattern) || guard_setup.falls_through,
                        guard_setup.available,
                        guard_setup.abrupt,
                    )
                }
                None => (
                    arm_reachable,
                    !pattern_is_irrefutable(&arm.pattern),
                    arm_available,
                    HashSet::new(),
                ),
            };
            let body = self.definition_statements(
                &arm.body,
                DefinitionFlow::falling_through(body_available),
                check_reads,
                body_reachable,
                known,
                owners,
                owner,
            )?;
            guard_abrupt.extend(abrupt);
            if body_reachable {
                normal_paths.push(body);
            }
            next_reachable &= can_try_next;
        }
        if let WhenFallback::Else(body) = &value.fallback {
            let fallback = self.definition_statements(
                body,
                DefinitionFlow::falling_through(available.clone()),
                check_reads,
                next_reachable,
                known,
                owners,
                owner,
            )?;
            if next_reachable {
                normal_paths.push(fallback);
            }
        }
        let mut merged = Self::merge_normal_definitions(available, normal_paths);
        merged.abrupt.extend(guard_abrupt);
        Ok(merged)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn definition_try(
        &self,
        value: &Try,
        available: HashSet<u32>,
        check_reads: bool,
        reachable: bool,
        known: &HashSet<u32>,
        owners: &mut DefinitionOwners,
        owner: DefinitionOwner,
    ) -> Check<DefinitionFlow> {
        let body = self.definition_statements(
            &value.body,
            DefinitionFlow::falling_through(available.clone()),
            check_reads,
            reachable,
            known,
            owners,
            owner,
        )?;
        let mut paths = vec![body];
        for catch in &value.catches {
            let mut catch_available = available.clone();
            self.insert_definition(catch.local, &mut catch_available, owners, owner)?;
            paths.push(self.definition_statements(
                &catch.body,
                DefinitionFlow::falling_through(catch_available),
                check_reads,
                reachable,
                known,
                owners,
                owner,
            )?);
        }
        let mut merged = Self::merge_normal_definitions(available.clone(), paths);
        if let Some(finally) = &value.finally_body {
            let incoming_reachable = merged.falls_through || !merged.abrupt.is_empty();
            let finally = self.definition_statements(
                finally,
                DefinitionFlow::falling_through(available),
                check_reads,
                incoming_reachable,
                known,
                owners,
                owner,
            )?;
            if incoming_reachable {
                if finally.falls_through {
                    merged.abrupt.extend(finally.abrupt);
                } else {
                    merged.falls_through = false;
                    merged.abrupt = finally.abrupt;
                }
            }
        }
        Ok(merged)
    }
}
