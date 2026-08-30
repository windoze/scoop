//! Block-level lexical scopes for local `val` / `var` declarations
//! (milestone2 DESIGN.md 2.2).

use std::collections::HashMap;

use scoop_hir::LocalId;

/// A stack of scopes, innermost last. Function bodies, nested blocks
/// and if/while bodies each push a scope; a local is visible from its
/// declaration to the end of its scope, and shadows outer locals with
/// the same name.
#[derive(Clone)]
pub(crate) struct Scopes {
    stack: Vec<HashMap<String, LocalId>>,
}

impl Scopes {
    pub(crate) fn new() -> Self {
        Scopes { stack: Vec::new() }
    }

    pub(crate) fn push(&mut self) {
        self.stack.push(HashMap::new());
    }

    pub(crate) fn pop(&mut self) {
        self.stack.pop();
    }

    /// Whether `name` is already declared in the innermost scope
    /// (redeclaration in the same scope is a diagnostic; shadowing an
    /// outer scope is not).
    pub(crate) fn is_declared_here(&self, name: &str) -> bool {
        self.stack
            .last()
            .is_some_and(|scope| scope.contains_key(name))
    }

    pub(crate) fn declare(&mut self, name: String, id: LocalId) {
        self.stack
            .last_mut()
            .expect("declarations only happen inside a scope")
            .insert(name, id);
    }

    /// Resolve `name`, innermost scope first (so inner locals shadow
    /// outer ones).
    pub(crate) fn lookup(&self, name: &str) -> Option<LocalId> {
        self.stack
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
    }

    /// The bindings visible at the current source position. Inner scopes
    /// replace outer bindings with the same name. Capture analysis consumes
    /// this snapshot before entering a nested callable body.
    pub(crate) fn visible(&self) -> HashMap<String, LocalId> {
        let mut visible = HashMap::new();
        for scope in &self.stack {
            visible.extend(scope.iter().map(|(name, local)| (name.clone(), *local)));
        }
        visible
    }
}
