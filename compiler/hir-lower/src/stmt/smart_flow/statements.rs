use super::*;

impl SmartFlow<'_> {
    pub(super) fn statement(
        &mut self,
        facts: SmartCastFacts,
        statement: &hir::Statement,
    ) -> SmartCastExits {
        match &statement.kind {
            hir::StatementKind::Expr(value) => self.expression(facts, value),
            hir::StatementKind::ValDecl { pattern, init } => {
                let mut exits = self.expression(facts, init);
                if let Some(facts) = exits.get_mut(&Outcome::Fallthrough) {
                    self.record_pattern(facts, pattern, init);
                }
                exits
            }
            hir::StatementKind::Assign { target, value } => {
                let mut exits = Self::normal(facts);
                let operands: Vec<&hir::Expr> = match target {
                    hir::AssignTarget::Index { array, index } => vec![array, index],
                    hir::AssignTarget::Field { receiver, .. } => vec![receiver],
                    _ => Vec::new(),
                };
                for operand in operands.into_iter().chain([value]) {
                    if let Some(facts) = exits.remove(&Outcome::Fallthrough) {
                        let next = self.expression(facts, operand);
                        self.merge(&mut exits, next);
                    }
                }
                if let hir::AssignTarget::Local(local) = target
                    && let Some(facts) = exits.get_mut(&Outcome::Fallthrough)
                {
                    // Source assignments discard the old value's facts. A
                    // compiler result local may retain a Boolean implication.
                    self.state.record_flow_value(facts, *local, value);
                }
                exits
            }
            hir::StatementKind::Return { value } => {
                let exits = self.expression(facts, value);
                self.retarget(exits, Outcome::Return)
            }
            hir::StatementKind::Throw(value) => {
                let exits = self.expression(facts, value);
                self.retarget(exits, Outcome::Throw)
            }
            hir::StatementKind::Break { target } => {
                BTreeMap::from([(Outcome::Break(*target), facts)])
            }
            hir::StatementKind::Continue { target } => {
                BTreeMap::from([(Outcome::Continue(*target), facts)])
            }
            hir::StatementKind::If {
                cond,
                then_body,
                else_body,
            } => {
                let mut exits = self.expression(facts, cond);
                if let Some(facts) = exits.remove(&Outcome::Fallthrough) {
                    for (outcome, body) in [
                        (true, then_body.as_slice()),
                        (false, else_body.as_deref().unwrap_or_default()),
                    ] {
                        if let Some(branch) =
                            self.state
                                .refine_flow_condition(facts.clone(), cond, outcome)
                        {
                            let next = self.statements(branch, body);
                            self.merge(&mut exits, next);
                        }
                    }
                }
                exits
            }
            hir::StatementKind::When(when) => self.when(facts, when),
            hir::StatementKind::Try(region) => self.try_region(facts, region),
            hir::StatementKind::While {
                loop_kind,
                target,
                condition_setup,
                cond,
                body,
            } => self.loop_region(facts, *loop_kind, *target, condition_setup, cond, body),
            hir::StatementKind::ContextScope { value, body } => {
                let mut exits = self.expression(facts, value);
                if let Some(facts) = exits.remove(&Outcome::Fallthrough) {
                    let next = self.statements(facts, body);
                    self.merge(&mut exits, next);
                }
                exits
            }
            hir::StatementKind::InitializationEnsure(_)
            | hir::StatementKind::GenericDelegateEnsure(_) => BTreeMap::from([
                (Outcome::Fallthrough, facts.clone()),
                (Outcome::Throw, facts),
            ]),
            hir::StatementKind::LocalFunction(_) => Self::normal(facts),
        }
    }

    fn record_pattern(
        &mut self,
        facts: &mut SmartCastFacts,
        pattern: &hir::Pattern,
        value: &hir::Expr,
    ) {
        if let hir::Pattern::Binding { local } = pattern {
            self.state.record_flow_value(facts, *local, value);
        }
    }
}
