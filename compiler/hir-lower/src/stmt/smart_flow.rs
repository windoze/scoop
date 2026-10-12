//! Flow-sensitive type facts for the structured HIR being constructed.
use super::*;
use crate::expr::SmartCastFacts;
use std::collections::BTreeMap;

mod expressions;
mod loops;
mod operands;
mod regions;
mod statements;

type Outcome = flow::ControlOutcome<hir::LoopId>;
pub(crate) type SmartCastExits = BTreeMap<Outcome, SmartCastFacts>;

struct SmartFlow<'a> {
    state: &'a mut Lowerer,
    check_reads: bool,
}

impl Lowerer {
    pub(crate) fn smart_cast_exits(
        &mut self,
        incoming: SmartCastFacts,
        statements: &[hir::Statement],
    ) -> SmartCastExits {
        SmartFlow {
            state: self,
            check_reads: false,
        }
        .statements(incoming, statements)
    }

    pub(crate) fn advance_smart_casts(
        &mut self,
        incoming: SmartCastFacts,
        statements: &[hir::Statement],
        value: Option<&hir::Expr>,
    ) {
        let mut flow = SmartFlow {
            state: self,
            check_reads: true,
        };
        let mut exits = flow.statements(incoming.clone(), statements);
        if let Some(value) = value
            && let Some(facts) = exits.remove(&Outcome::Fallthrough)
        {
            let next = flow.expression(facts, value);
            flow.merge(&mut exits, next);
        }
        self.smart_casts = exits.remove(&Outcome::Fallthrough).unwrap_or(incoming);
    }

    pub(crate) fn joined_exit_facts(&mut self, exits: &SmartCastExits) -> Option<SmartCastFacts> {
        let mut facts = exits.values().cloned();
        let mut result = facts.next()?;
        for facts in facts {
            result = self.common_smart_casts(result, facts);
        }
        Some(result)
    }

    pub(crate) fn catch_smart_cast_facts(
        &mut self,
        incoming: SmartCastFacts,
        body: &[hir::Statement],
        value: Option<&hir::Expr>,
    ) -> SmartCastFacts {
        let mut flow = SmartFlow {
            state: self,
            check_reads: false,
        };
        let mut exits = flow.statements(incoming.clone(), body);
        if let Some(value) = value
            && let Some(facts) = exits.remove(&Outcome::Fallthrough)
        {
            let next = flow.expression(facts, value);
            flow.merge(&mut exits, next);
        }
        exits.remove(&Outcome::Throw).unwrap_or(incoming)
    }

    pub(crate) fn finally_smart_cast_facts<'a>(
        &mut self,
        incoming: SmartCastFacts,
        body: &[hir::Statement],
        catches: impl Iterator<Item = &'a [hir::Statement]>,
    ) -> SmartCastFacts {
        let mut exits = self.smart_cast_exits(incoming.clone(), body);
        if let Some(thrown) = exits.get(&Outcome::Throw).cloned() {
            for body in catches {
                let next = self.smart_cast_exits(thrown.clone(), body);
                SmartFlow {
                    state: self,
                    check_reads: false,
                }
                .merge(&mut exits, next);
            }
        }
        self.joined_exit_facts(&exits).unwrap_or(incoming)
    }
}

impl SmartFlow<'_> {
    fn normal(facts: SmartCastFacts) -> SmartCastExits {
        BTreeMap::from([(Outcome::Fallthrough, facts)])
    }

    fn merge(&mut self, target: &mut SmartCastExits, source: SmartCastExits) {
        for (outcome, facts) in source {
            self.insert(target, outcome, facts);
        }
    }

    fn insert(&mut self, exits: &mut SmartCastExits, outcome: Outcome, facts: SmartCastFacts) {
        let facts = if let Some(previous) = exits.remove(&outcome) {
            self.state.common_smart_casts(previous, facts)
        } else {
            facts
        };
        exits.insert(outcome, facts);
    }

    fn statements(
        &mut self,
        incoming: SmartCastFacts,
        statements: &[hir::Statement],
    ) -> SmartCastExits {
        let mut exits = Self::normal(incoming);
        for statement in statements {
            let Some(facts) = exits.remove(&Outcome::Fallthrough) else {
                break;
            };
            let next = self.statement(facts, statement);
            self.merge(&mut exits, next);
        }
        exits
    }

    fn retarget(&mut self, mut exits: SmartCastExits, target: Outcome) -> SmartCastExits {
        if let Some(facts) = exits.remove(&Outcome::Fallthrough) {
            self.insert(&mut exits, target, facts);
        }
        exits
    }
}
