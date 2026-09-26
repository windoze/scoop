use std::collections::HashSet;

use super::*;

mod control;
mod iteration;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum AbruptOutcome {
    Return,
    Throw,
    Break(u32),
    Continue(u32),
}

#[derive(Clone)]
struct DefinitionFlow {
    available: HashSet<u32>,
    falls_through: bool,
    abrupt: HashSet<AbruptOutcome>,
}

impl DefinitionFlow {
    fn falling_through(available: HashSet<u32>) -> Self {
        Self {
            available,
            falls_through: true,
            abrupt: HashSet::new(),
        }
    }

    fn abrupt(available: HashSet<u32>, outcome: AbruptOutcome) -> Self {
        Self {
            available,
            falls_through: false,
            abrupt: HashSet::from([outcome]),
        }
    }
}

#[derive(Clone, Copy)]
enum DefinitionOwner {
    Ordinary,
    Plan(usize),
}

#[derive(Default)]
struct DefinitionOwners {
    ordinary: HashSet<u32>,
    plans: Vec<HashSet<u32>>,
}

impl DefinitionOwners {
    fn new_plan(&mut self) -> usize {
        let owner = self.plans.len();
        self.plans.push(HashSet::new());
        owner
    }

    fn get_mut(&mut self, owner: DefinitionOwner) -> &mut HashSet<u32> {
        match owner {
            DefinitionOwner::Ordinary => &mut self.ordinary,
            DefinitionOwner::Plan(owner) => &mut self.plans[owner],
        }
    }
}

impl Validator<'_> {
    pub(super) fn validate_iteration_definition_schedules(
        &self,
        statements: &[Statement],
        predefined: impl IntoIterator<Item = LocalId>,
    ) -> Check {
        let known = self
            .locals
            .iter()
            .map(|(local, _)| local.into_raw().into_u32())
            .collect::<HashSet<_>>();
        let mut owners = DefinitionOwners::default();
        let mut available = HashSet::new();
        for local in predefined {
            self.insert_definition(
                local,
                &mut available,
                &mut owners,
                DefinitionOwner::Ordinary,
            )?;
        }
        self.definition_statements(
            statements,
            DefinitionFlow::falling_through(available),
            false,
            true,
            &known,
            &mut owners,
            DefinitionOwner::Ordinary,
        )?;
        self.validate_definition_owners(&owners)
    }

    #[allow(clippy::too_many_arguments)]
    fn definition_statements(
        &self,
        statements: &[Statement],
        mut flow: DefinitionFlow,
        check_reads: bool,
        reachable: bool,
        known: &HashSet<u32>,
        owners: &mut DefinitionOwners,
        owner: DefinitionOwner,
    ) -> Check<DefinitionFlow> {
        for statement in statements {
            let statement_reachable = reachable && flow.falls_through;
            let next = self.definition_statement(
                statement,
                std::mem::take(&mut flow.available),
                check_reads,
                statement_reachable,
                known,
                owners,
                owner,
            )?;
            if statement_reachable {
                flow.abrupt.extend(next.abrupt);
                flow.falls_through = next.falls_through;
            }
            // Unreachable syntax still contributes definition ownership, but
            // cannot restore a normal edge or make any read observable.
            flow.available = next.available;
        }
        if !reachable {
            flow.falls_through = false;
            flow.abrupt.clear();
        }
        Ok(flow)
    }

    #[allow(clippy::too_many_arguments)]
    fn definition_statement(
        &self,
        statement: &Statement,
        mut available: HashSet<u32>,
        check_reads: bool,
        reachable: bool,
        known: &HashSet<u32>,
        owners: &mut DefinitionOwners,
        owner: DefinitionOwner,
    ) -> Check<DefinitionFlow> {
        match &statement.kind {
            StatementKind::Expr(expression) => {
                self.check_expression_reads(
                    expression,
                    &available,
                    check_reads && reachable,
                    known,
                )?;
                Ok(DefinitionFlow::falling_through(available))
            }
            StatementKind::InitializationEnsure(_) | StatementKind::LocalFunction(_) => {
                Ok(DefinitionFlow::falling_through(available))
            }
            StatementKind::Return { value } => {
                if let Some(value) = value {
                    self.check_expression_reads(
                        value,
                        &available,
                        check_reads && reachable,
                        known,
                    )?;
                }
                Ok(DefinitionFlow::abrupt(available, AbruptOutcome::Return))
            }
            StatementKind::ValDecl { pattern, init } => {
                self.check_pattern_reads(pattern, &available, check_reads && reachable, known)?;
                self.check_expression_reads(init, &available, check_reads && reachable, known)?;
                self.insert_pattern_definitions(pattern, &mut available, owners, owner)?;
                Ok(DefinitionFlow::falling_through(available))
            }
            StatementKind::Assign { target, value } => {
                self.check_expression_reads(value, &available, check_reads && reachable, known)?;
                match target {
                    AssignTarget::Local(local) => {
                        let declaration = checked_arena(self.locals, *local).ok_or_else(|| {
                            invalid("iteration schedule assigns an invalid local")
                        })?;
                        let identity = local.into_raw().into_u32();
                        if !available.contains(&identity) {
                            if !declaration.mutable {
                                return fail(
                                    "iteration schedule initializes an immutable local by assignment",
                                );
                            }
                            self.record_owned_definition(*local, owners, owner)?;
                            available.insert(identity);
                        }
                    }
                    _ if check_reads
                        && reachable
                        && uses::assign_target_references_outside(
                            self.module,
                            target,
                            known,
                            &available,
                        ) =>
                    {
                        return fail("for source prefix references a future plan local");
                    }
                    _ => {}
                }
                Ok(DefinitionFlow::falling_through(available))
            }
            StatementKind::If {
                cond,
                then_body,
                else_body,
            } => {
                self.check_expression_reads(cond, &available, check_reads && reachable, known)?;
                let then_flow = self.definition_statements(
                    then_body,
                    DefinitionFlow::falling_through(available.clone()),
                    check_reads,
                    reachable,
                    known,
                    owners,
                    owner,
                )?;
                let else_flow = match else_body {
                    Some(else_body) => self.definition_statements(
                        else_body,
                        DefinitionFlow::falling_through(available.clone()),
                        check_reads,
                        reachable,
                        known,
                        owners,
                        owner,
                    )?,
                    None if reachable => DefinitionFlow::falling_through(available.clone()),
                    None => DefinitionFlow {
                        available: available.clone(),
                        falls_through: false,
                        abrupt: HashSet::new(),
                    },
                };
                Ok(Self::merge_normal_definitions(
                    available,
                    [then_flow, else_flow],
                ))
            }
            StatementKind::While {
                target,
                condition_setup,
                cond,
                body,
            } => self.definition_while(
                *target,
                condition_setup,
                cond,
                body,
                available,
                check_reads,
                reachable,
                known,
                owners,
                owner,
            ),
            StatementKind::For(plan) => {
                self.validate_for_definition_schedule(plan, &available, reachable, known, owners)
            }
            StatementKind::Break { target } => Ok(DefinitionFlow::abrupt(
                available,
                AbruptOutcome::Break(target.into_raw()),
            )),
            StatementKind::Continue { target } => Ok(DefinitionFlow::abrupt(
                available,
                AbruptOutcome::Continue(target.into_raw()),
            )),
            StatementKind::When(value) => self.definition_when(
                value,
                available,
                check_reads,
                reachable,
                known,
                owners,
                owner,
            ),
            StatementKind::Try(value) => self.definition_try(
                value,
                available,
                check_reads,
                reachable,
                known,
                owners,
                owner,
            ),
            StatementKind::Throw(value) => {
                self.check_expression_reads(value, &available, check_reads && reachable, known)?;
                Ok(DefinitionFlow::abrupt(available, AbruptOutcome::Throw))
            }
        }
    }

    fn check_expression_reads(
        &self,
        expression: &Expr,
        available: &HashSet<u32>,
        check_reads: bool,
        known: &HashSet<u32>,
    ) -> Check {
        if check_reads
            && uses::expression_references_outside(self.module, expression, known, available)
        {
            return fail("for source prefix references a future plan local");
        }
        Ok(())
    }

    fn check_pattern_reads(
        &self,
        pattern: &Pattern,
        available: &HashSet<u32>,
        check_reads: bool,
        known: &HashSet<u32>,
    ) -> Check {
        if check_reads && uses::pattern_references_outside(self.module, pattern, known, available) {
            return fail("for source prefix references a future plan local");
        }
        Ok(())
    }

    fn insert_pattern_definitions(
        &self,
        pattern: &Pattern,
        available: &mut HashSet<u32>,
        owners: &mut DefinitionOwners,
        owner: DefinitionOwner,
    ) -> Check {
        match pattern {
            Pattern::Binding { local } => self.insert_definition(*local, available, owners, owner),
            Pattern::Variant { fields, .. }
            | Pattern::ImportedVariant { fields, .. }
            | Pattern::Struct { fields, .. } => {
                for (_, pattern) in fields {
                    self.insert_pattern_definitions(pattern, available, owners, owner)?;
                }
                Ok(())
            }
            Pattern::Tuple(elements) => {
                for pattern in elements {
                    self.insert_pattern_definitions(pattern, available, owners, owner)?;
                }
                Ok(())
            }
            Pattern::Wildcard | Pattern::Literal { .. } => Ok(()),
        }
    }

    fn insert_definition(
        &self,
        local: LocalId,
        available: &mut HashSet<u32>,
        owners: &mut DefinitionOwners,
        owner: DefinitionOwner,
    ) -> Check {
        checked_arena(self.locals, local)
            .ok_or_else(|| invalid("iteration schedule defines an invalid local"))?;
        if !available.insert(local.into_raw().into_u32()) {
            return fail("iteration schedule reuses an already defined local");
        }
        self.record_owned_definition(local, owners, owner)
    }

    fn mark_definition(&self, local: LocalId, available: &mut HashSet<u32>) -> Check {
        checked_arena(self.locals, local)
            .ok_or_else(|| invalid("iteration schedule defines an invalid local"))?;
        available.insert(local.into_raw().into_u32());
        Ok(())
    }

    fn record_owned_definition(
        &self,
        local: LocalId,
        owners: &mut DefinitionOwners,
        owner: DefinitionOwner,
    ) -> Check {
        checked_arena(self.locals, local)
            .ok_or_else(|| invalid("iteration region defines an invalid local"))?;
        owners.get_mut(owner).insert(local.into_raw().into_u32());
        Ok(())
    }

    fn validate_definition_owners(&self, owners: &DefinitionOwners) -> Check {
        for (index, plan) in owners.plans.iter().enumerate() {
            if !plan.is_disjoint(&owners.ordinary)
                || owners.plans[..index]
                    .iter()
                    .any(|other| !plan.is_disjoint(other))
            {
                return fail("iteration plan reuses a local defined outside that plan");
            }
        }
        Ok(())
    }

    fn merge_normal_definitions(
        incoming: HashSet<u32>,
        flows: impl IntoIterator<Item = DefinitionFlow>,
    ) -> DefinitionFlow {
        let flows = flows.into_iter().collect::<Vec<_>>();
        let mut abrupt = HashSet::new();
        for flow in &flows {
            abrupt.extend(flow.abrupt.iter().copied());
        }
        let mut normal = flows.into_iter().filter(|flow| flow.falls_through);
        let Some(first) = normal.next() else {
            return DefinitionFlow {
                available: incoming,
                falls_through: false,
                abrupt,
            };
        };
        let mut available = first.available;
        for flow in normal {
            available.retain(|local| flow.available.contains(local));
        }
        DefinitionFlow {
            available,
            falls_through: true,
            abrupt,
        }
    }
}

fn pattern_is_irrefutable(pattern: &Pattern) -> bool {
    match pattern {
        Pattern::Binding { .. } | Pattern::Wildcard => true,
        Pattern::Tuple(elements) => elements.iter().all(pattern_is_irrefutable),
        Pattern::Struct { fields, .. } => fields
            .iter()
            .all(|(_, pattern)| pattern_is_irrefutable(pattern)),
        Pattern::Variant { .. } | Pattern::ImportedVariant { .. } | Pattern::Literal { .. } => {
            false
        }
    }
}
