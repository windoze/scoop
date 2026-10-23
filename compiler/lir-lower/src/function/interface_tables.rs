//! Cache immutable table pointers without retaining another managed object.

use std::collections::{HashMap, HashSet};

use super::{lir, mir};

pub(super) struct TableCache {
    enabled: bool,
    addressed: HashSet<mir::LocalId>,
    entries: HashMap<mir::LocalId, HashMap<mir::InterfaceId, lir::Value>>,
}

impl TableCache {
    pub fn new(optimization: lir::OptimizationMode, addressed: HashSet<mir::LocalId>) -> Self {
        Self {
            enabled: optimization == lir::OptimizationMode::Release,
            addressed,
            entries: HashMap::new(),
        }
    }

    fn source(&self, expression: &mir::Expr) -> Option<mir::LocalId> {
        if self.enabled
            && matches!(
                expression.ty,
                mir::Type::Any
                    | mir::Type::Class(_)
                    | mir::Type::Interface(_)
                    | mir::Type::String
                    | mir::Type::Function(_)
            )
            && let mir::ExprKind::Local(local) = expression.kind
            && !self.addressed.contains(&local)
        {
            Some(local)
        } else {
            None
        }
    }

    pub fn get(&self, source: &mir::Expr, interface: mir::InterfaceId) -> Option<lir::Value> {
        self.entries
            .get(&self.source(source)?)?
            .get(&interface)
            .copied()
    }

    pub fn insert(&mut self, source: &mir::Expr, interface: mir::InterfaceId, table: lir::Value) {
        if let Some(local) = self.source(source) {
            self.entries
                .entry(local)
                .or_default()
                .insert(interface, table);
        }
    }

    pub fn written(&mut self, statement: &mir::Statement) {
        let local = match statement.kind {
            mir::StatementKind::ValDecl { local, .. }
            | mir::StatementKind::Assign { local, .. }
            | mir::StatementKind::Call(mir::CallEffect::Value {
                destination: local, ..
            }) => local,
            _ => return,
        };
        self.entries.remove(&local);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}
