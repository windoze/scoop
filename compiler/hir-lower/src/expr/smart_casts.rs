use super::*;
use std::collections::BTreeMap;

mod conditions;
mod facts;
mod joins;
mod properties;
mod reads;

use facts::TypeFacts;
pub(crate) use facts::{BooleanFacts, SmartCastFacts};

impl Lowerer {
    /// A branch receives an independent state, including snapshots and writes.
    pub(crate) fn with_smart_casts<T>(
        &mut self,
        facts: SmartCastFacts,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let saved = std::mem::replace(&mut self.smart_casts, facts);
        let result = f(self);
        self.smart_casts = saved;
        result
    }

    pub(crate) fn checked_condition_facts(
        &mut self,
        condition: &hir::Expr,
        outcome: bool,
    ) -> SmartCastFacts {
        self.refine_flow_condition(self.smart_casts.clone(), condition, outcome)
            .unwrap_or_else(|| self.smart_casts.clone())
    }
}
