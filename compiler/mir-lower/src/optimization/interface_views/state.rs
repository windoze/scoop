//! Reference sources name current local values, never mutable storage addresses.

use std::collections::{HashMap, HashSet};

use la_arena::Arena;

use super::{mir, pure_local};

#[derive(Default)]
pub(super) struct Available {
    aliases: HashMap<mir::LocalId, mir::LocalId>,
    tracked: HashSet<mir::LocalId>,
}

impl Available {
    pub fn source(
        &self,
        expression: &mir::Expr,
        addressed: &HashSet<mir::LocalId>,
    ) -> Option<mir::LocalId> {
        let local = pure_local(expression)?;
        if addressed.contains(&local) {
            return None;
        }
        Some(self.aliases.get(&local).copied().unwrap_or(local))
    }

    pub fn statement(
        &mut self,
        statement: &mir::Statement,
        locals: &Arena<mir::Local>,
        addressed: &HashSet<mir::LocalId>,
    ) {
        let (local, source) = match &statement.kind {
            mir::StatementKind::ValDecl { local, init }
            | mir::StatementKind::Assign { local, value: init } => {
                let source = self.source(init, addressed).filter(|source| {
                    locals[*local].ty == mir::Type::Any || locals[*source].ty == locals[*local].ty
                });
                (*local, source)
            }
            mir::StatementKind::Call(mir::CallEffect::Value { destination, .. }) => {
                (*destination, None)
            }
            _ => return,
        };
        // Roots are flattened when assigned. Forget old snapshots' association
        // with this storage rather than redirecting them to its new value.
        // Most MIR definitions introduce fresh temporaries. They cannot
        // invalidate prior views and need no scan of the available maps.
        if self.tracked.contains(&local) {
            self.aliases
                .retain(|alias, root| *alias != local && *root != local);
        }
        if addressed.contains(&local) {
            return;
        }
        let root = source.unwrap_or(local);
        if root != local {
            self.aliases.insert(local, root);
            self.tracked.extend([local, root]);
        }
    }
}
