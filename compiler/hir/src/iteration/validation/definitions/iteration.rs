use std::collections::HashSet;

use super::*;

impl Validator<'_> {
    pub(super) fn validate_for_definition_schedule(
        &self,
        plan: &ForIterationPlan,
        incoming: &HashSet<u32>,
        reachable: bool,
        known: &HashSet<u32>,
        owners: &mut DefinitionOwners,
    ) -> Check<DefinitionFlow> {
        let owner = DefinitionOwner::Plan(owners.new_plan());
        for temporary in [
            plan.source(),
            plan.conformance().source(),
            plan.conformance().iterator(),
            plan.next().result(),
            plan.next().element(),
        ] {
            self.record_owned_definition(temporary.local, owners, owner)?;
        }
        for action in &plan.binding().actions {
            let local = match action {
                IrrefutableBindingAction::Project { result, .. }
                | IrrefutableBindingAction::Component { result, .. } => result.local,
                IrrefutableBindingAction::Bind { target, .. } => target.local,
            };
            self.record_owned_definition(local, owners, owner)?;
        }

        let source = self.definition_statements(
            plan.source_setup(),
            DefinitionFlow::falling_through(incoming.clone()),
            true,
            reachable,
            known,
            owners,
            owner,
        )?;
        if source.falls_through {
            self.check_expression_reads(plan.source_init(), &source.available, true, known)?;
        }
        let mut initialized = source.available.clone();
        self.mark_definition(plan.source().local, &mut initialized)?;

        let iterator = self.definition_statements(
            plan.iterator_setup(),
            DefinitionFlow::falling_through(initialized),
            true,
            source.falls_through,
            known,
            owners,
            owner,
        )?;
        if iterator.falls_through {
            self.check_expression_reads(plan.iterator_call(), &iterator.available, true, known)?;
        }
        let mut iteration = iterator.available.clone();
        self.mark_definition(plan.conformance().source().local, &mut iteration)?;
        self.mark_definition(plan.conformance().iterator().local, &mut iteration)?;
        self.mark_definition(plan.next().result().local, &mut iteration)?;
        self.mark_definition(plan.next().element().local, &mut iteration)?;

        let reaches_iteration = source.falls_through && iterator.falls_through;
        let mut successful_iteration = DefinitionFlow::falling_through(iteration);
        for action in &plan.binding().actions {
            match action {
                IrrefutableBindingAction::Project { result, .. } => {
                    self.mark_definition(result.local, &mut successful_iteration.available)?;
                }
                IrrefutableBindingAction::Component {
                    setup,
                    call,
                    result,
                    ..
                } => {
                    let action_reachable = reaches_iteration && successful_iteration.falls_through;
                    let setup = self.definition_statements(
                        setup,
                        DefinitionFlow::falling_through(successful_iteration.available),
                        true,
                        action_reachable,
                        known,
                        owners,
                        owner,
                    )?;
                    if setup.falls_through {
                        self.check_expression_reads(call, &setup.available, true, known)?;
                    }
                    if action_reachable {
                        successful_iteration.abrupt.extend(setup.abrupt);
                        successful_iteration.falls_through = setup.falls_through;
                    }
                    successful_iteration.available = setup.available;
                    self.mark_definition(result.local, &mut successful_iteration.available)?;
                }
                IrrefutableBindingAction::Bind { target, .. } => {
                    self.mark_definition(target.local, &mut successful_iteration.available)?;
                }
            }
        }
        let body_reachable = reaches_iteration && successful_iteration.falls_through;
        let body = self.definition_statements(
            plan.body(),
            DefinitionFlow::falling_through(successful_iteration.available),
            false,
            body_reachable,
            known,
            owners,
            DefinitionOwner::Ordinary,
        )?;
        if body_reachable {
            successful_iteration.abrupt.extend(body.abrupt);
            successful_iteration.falls_through = body.falls_through;
        }

        let mut abrupt = source.abrupt;
        if source.falls_through {
            abrupt.extend(iterator.abrupt);
        }
        if reaches_iteration {
            let mut iteration_abrupt = successful_iteration.abrupt;
            iteration_abrupt.remove(&AbruptOutcome::Break(plan.target().into_raw()));
            iteration_abrupt.remove(&AbruptOutcome::Continue(plan.target().into_raw()));
            abrupt.extend(iteration_abrupt);
        }
        Ok(DefinitionFlow {
            available: incoming.clone(),
            // A completed prefix may observe `None` before the first body
            // execution, independently of all body outcomes.
            falls_through: reaches_iteration,
            abrupt,
        })
    }
}
