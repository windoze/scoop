//! Cache immutable table pointers without retaining another managed object.

use std::collections::{HashMap, HashSet};

use super::{lir, mir};

pub(super) enum CachedTable {
    Slots(lir::Value),
    CheckedEntry(lir::LocalId),
}

pub(super) struct TableCache {
    enabled: bool,
    addressed: HashSet<mir::LocalId>,
    entries: HashMap<mir::LocalId, HashMap<mir::InterfaceId, lir::Value>>,
    checked: HashMap<(mir::LocalId, mir::InterfaceId), lir::LocalId>,
}

impl TableCache {
    pub fn new(optimization: lir::OptimizationMode, addressed: HashSet<mir::LocalId>) -> Self {
        Self {
            enabled: optimization == lir::OptimizationMode::Release,
            addressed,
            entries: HashMap::new(),
            checked: HashMap::new(),
        }
    }

    fn source(&self, expression: &mir::Expr) -> Option<mir::LocalId> {
        if self.enabled
            && let Some(local) = source_local(expression)
            && !self.addressed.contains(&local)
        {
            Some(local)
        } else {
            None
        }
    }

    pub fn get(&self, source: &mir::Expr, interface: mir::InterfaceId) -> Option<CachedTable> {
        let local = self.source(source)?;
        self.entries
            .get(&local)
            .and_then(|tables| tables.get(&interface))
            .copied()
            .map(CachedTable::Slots)
            .or_else(|| {
                self.checked
                    .get(&(local, interface))
                    .copied()
                    .map(CachedTable::CheckedEntry)
            })
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
        if let Some(local) = written_local(statement) {
            self.entries.remove(&local);
            self.checked.retain(|(source, _), _| *source != local);
        }
    }

    pub fn enter_mir(&mut self, checked: HashMap<(mir::LocalId, mir::InterfaceId), lir::LocalId>) {
        self.checked = checked;
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub(super) fn source_local(expression: &mir::Expr) -> Option<mir::LocalId> {
    if matches!(
        expression.ty,
        mir::Type::Any
            | mir::Type::Class(_)
            | mir::Type::Interface(_)
            | mir::Type::String
            | mir::Type::Function(_)
    ) && let mir::ExprKind::Local(local) = expression.kind
    {
        Some(local)
    } else {
        None
    }
}

pub(super) fn written_local(statement: &mir::Statement) -> Option<mir::LocalId> {
    match statement.kind {
        mir::StatementKind::ValDecl { local, .. }
        | mir::StatementKind::Assign { local, .. }
        | mir::StatementKind::Call(mir::CallEffect::Value {
            destination: local, ..
        }) => Some(local),
        _ => None,
    }
}
