//! Block-level lexical scopes for local `val` / `var` declarations
//! (milestone2 DESIGN.md 2.2).

use std::collections::HashMap;

use scoop_hir::{LocalFunctionId, LocalId};

/// A stack of scopes, innermost last. Function bodies, nested blocks
/// and if/while bodies each push a scope; a local is visible from its
/// declaration to the end of its scope, and shadows outer locals with
/// the same name.
#[derive(Debug, Clone)]
pub(crate) struct Scopes {
    stack: Vec<imbl::HashMap<String, LocalId>>,
    aliases: Vec<imbl::HashMap<String, crate::aliases::SourceTypeAliasId>>,
    nominals: Vec<imbl::HashMap<String, crate::NominalTarget>>,
    imports: Vec<std::sync::Arc<crate::imports::FrozenFileImports>>,
}

/// Lexical candidate layers for block-local named functions. Each block is a
/// separate layer; declarations are inserted as they are encountered, so a
/// name is visible to its own body and subsequent statements but never
/// before its declaration.
#[derive(Clone)]
pub(crate) struct LocalFunctionScopes {
    stack: Vec<imbl::HashMap<String, Vec<LocalFunctionId>>>,
}

impl LocalFunctionScopes {
    pub(crate) fn new() -> Self {
        Self { stack: Vec::new() }
    }

    pub(crate) fn push(&mut self) {
        self.stack.push(imbl::HashMap::new());
    }

    pub(crate) fn pop(&mut self) {
        self.stack.pop();
    }

    pub(crate) fn declare(&mut self, name: String, id: LocalFunctionId) {
        self.stack
            .last_mut()
            .expect("local functions are declared inside a scope")
            .entry(name)
            .or_default()
            .push(id);
    }

    pub(crate) fn current(&self, name: &str) -> Vec<LocalFunctionId> {
        self.stack
            .last()
            .and_then(|scope| scope.get(name))
            .cloned()
            .unwrap_or_default()
    }

    /// Return the nearest candidate layer containing `name` as one whole
    /// overload set.
    pub(crate) fn lookup(&self, name: &str) -> Vec<LocalFunctionId> {
        self.stack
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
            .unwrap_or_default()
    }
}

impl Scopes {
    pub(crate) fn new() -> Self {
        Scopes {
            stack: Vec::new(),
            aliases: Vec::new(),
            nominals: Vec::new(),
            imports: Vec::new(),
        }
    }

    pub(crate) fn push(&mut self) {
        self.stack.push(imbl::HashMap::new());
        self.aliases.push(imbl::HashMap::new());
        self.nominals.push(imbl::HashMap::new());
        self.imports.push(Default::default());
    }

    pub(crate) fn pop(&mut self) {
        self.stack.pop();
        self.aliases.pop();
        self.nominals.pop();
        self.imports.pop();
    }

    pub(crate) fn imports(&self) -> &[std::sync::Arc<crate::imports::FrozenFileImports>] {
        &self.imports
    }

    pub(crate) fn add_imports(&mut self, imports: crate::imports::FrozenFileImports) {
        let current = std::sync::Arc::make_mut(
            self.imports
                .last_mut()
                .expect("a block import has a lexical scope"),
        );
        current.exact.extend(imports.exact);
        current.stars.extend(imports.stars);
        current.unavailable.extend(imports.unavailable);
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

impl Scopes {
    pub(crate) fn nested_callable(&self) -> Self {
        Self {
            stack: vec![imbl::HashMap::new(); self.stack.len()],
            aliases: self.aliases.clone(),
            nominals: self.nominals.clone(),
            imports: self.imports.clone(),
        }
    }

    pub(crate) fn type_alias(&self, name: &str) -> Option<crate::aliases::SourceTypeAliasId> {
        match self.type_target(name)? {
            crate::namespace::TopLevelTypeTarget::Alias(id) => Some(id),
            _ => None,
        }
    }

    pub(crate) fn type_target(&self, name: &str) -> Option<crate::namespace::TopLevelTypeTarget> {
        self.aliases
            .iter()
            .zip(&self.nominals)
            .rev()
            .find_map(|(aliases, nominals)| {
                aliases
                    .get(name)
                    .copied()
                    .map(crate::namespace::TopLevelTypeTarget::Alias)
                    .or_else(|| {
                        nominals
                            .get(name)
                            .copied()
                            .map(crate::namespace::TopLevelTypeTarget::Nominal)
                    })
            })
    }

    pub(crate) fn type_declared_here(&self, name: &str) -> bool {
        self.aliases
            .last()
            .is_some_and(|scope| scope.contains_key(name))
            || self
                .nominals
                .last()
                .is_some_and(|scope| scope.contains_key(name))
    }

    pub(crate) fn contains_nominal(&self, owner: crate::Owner) -> bool {
        self.nominals
            .iter()
            .any(|scope| scope.values().any(|target| target.owner() == owner))
    }

    pub(crate) fn declare_nominal(&mut self, name: String, target: crate::NominalTarget) {
        self.nominals
            .last_mut()
            .expect("local types have a lexical scope")
            .insert(name, target);
    }

    pub(crate) fn declare_type_alias(
        &mut self,
        name: String,
        alias: crate::aliases::SourceTypeAliasId,
    ) -> Option<crate::aliases::SourceTypeAliasId> {
        self.aliases
            .last_mut()
            .expect("a local alias has a lexical scope")
            .insert(name, alias)
    }
}
