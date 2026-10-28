use super::*;
use scoop_identity::PersistentLocalValueId;

/// A lexical block in the already structured source body. The parent belongs
/// to the same callable; inlining is carried separately by DebugLocation.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DebugScope {
    pub parent: Option<Arc<DebugScope>>,
    pub file: Arc<DebugFile>,
    pub line: u32,
    pub column: u32,
}

/// Immutable source facts shared by every IR stage and inline copy.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DebugDeclaration {
    pub identity: PersistentLocalValueId,
    pub name: Arc<str>,
}

/// The declaration stays shared while its lexical/inline location can change.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum DebugVariable {
    #[default]
    Generated,
    Source {
        declaration: Arc<DebugDeclaration>,
        location: Arc<DebugLocation>,
    },
}

impl DebugRecords {
    pub fn scope(
        &mut self,
        parent: Option<Arc<DebugScope>>,
        origin: &DebugOrigin,
    ) -> Option<Arc<DebugScope>> {
        let DebugOrigin::Source(location) = origin else {
            return parent;
        };
        let scope = DebugScope {
            parent,
            file: location.file.clone(),
            line: location.line,
            column: location.column,
        };
        Some(match self.scopes.get(&scope) {
            Some(scope) => scope.clone(),
            None => {
                let scope = Arc::new(scope);
                self.scopes.insert(scope.clone());
                scope
            }
        })
    }

    pub fn scoped(&mut self, origin: &DebugOrigin, scope: Option<Arc<DebugScope>>) -> DebugOrigin {
        match origin {
            DebugOrigin::Generated => DebugOrigin::Generated,
            DebugOrigin::Source(location) => {
                let mut location = (**location).clone();
                location.scope = scope;
                self.location(location)
            }
        }
    }

    pub fn variable(
        &mut self,
        identity: PersistentLocalValueId,
        name: &str,
        origin: DebugOrigin,
    ) -> DebugVariable {
        let DebugOrigin::Source(location) = origin else {
            return DebugVariable::Generated;
        };
        let declaration = DebugDeclaration {
            identity,
            name: name.into(),
        };
        let declaration = match self.declarations.get(&declaration) {
            Some(declaration) => declaration.clone(),
            None => {
                let declaration = Arc::new(declaration);
                self.declarations.insert(declaration.clone());
                declaration
            }
        };
        DebugVariable::Source {
            declaration,
            location,
        }
    }

    pub fn scope_variable(&mut self, variable: &mut DebugVariable, scope: Option<Arc<DebugScope>>) {
        if let DebugVariable::Source { location, .. } = variable {
            let DebugOrigin::Source(scoped) =
                self.scoped(&DebugOrigin::Source(location.clone()), scope)
            else {
                unreachable!("a scoped source location remains a source location")
            };
            *location = scoped;
        }
    }

    pub fn inline_variable(&mut self, variable: &mut DebugVariable, site: &Arc<DebugInlineSite>) {
        if let DebugVariable::Source { location, .. } = variable {
            let DebugOrigin::Source(inlined) =
                self.inlined(&DebugOrigin::Source(location.clone()), site)
            else {
                unreachable!("an inlined source location remains a source location")
            };
            *location = inlined;
        }
    }
}
