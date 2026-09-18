//! Ordinary MIR paired with the exact trusted-core selection that it uses.

use std::collections::HashSet;
use std::fmt;

use crate::{
    Callee, ImportedCoreCallableUseId, ImportedDependencyMirCallableId, MirValidationError, Module,
    SelectedDependencyMirSet, SelectedImportedMirSet, StatementKind,
};

/// Closed ordinary MIR product whose imported-core references are branded by
/// and resolved through one exact selected set.
///
/// Owning the sidecar prevents later stages from pairing an already-lowered
/// MIR graph with an equal-looking selection projected from another artifact.
pub struct OrdinaryMirOutput<'a> {
    module: Module,
    imported_core: SelectedImportedMirSet<'a>,
    imported_dependencies: SelectedDependencyMirSet,
}

impl<'a> OrdinaryMirOutput<'a> {
    pub fn try_new(
        module: Module,
        imported_core: SelectedImportedMirSet<'a>,
        imported_dependencies: SelectedDependencyMirSet,
    ) -> Result<Self, OrdinaryMirOutputError> {
        if module.cone == scoop_identity::ConeIdentity::CORE {
            return Err(OrdinaryMirOutputError::CurrentConeIsCore);
        }
        module
            .validate()
            .map_err(OrdinaryMirOutputError::InvalidModule)?;
        if imported_dependencies.consumer() != module.cone {
            return Err(OrdinaryMirOutputError::ImportedDependencyConsumerMismatch {
                module: module.cone,
                selected: imported_dependencies.consumer(),
            });
        }
        if module.meta.imported_core_callables.len() != imported_core.len() {
            return Err(OrdinaryMirOutputError::SelectionCountMismatch {
                module: module.meta.imported_core_callables.len(),
                selected: imported_core.len(),
            });
        }

        let mut resolved = HashSet::with_capacity(module.meta.imported_core_callables.len());
        for (id, callable) in module.meta.imported_core_callables.iter() {
            let Some(selected) = imported_core.resolve_callable(callable.reference()) else {
                return Err(OrdinaryMirOutputError::ForeignImportedCallable {
                    index: id.into_raw().into_u32(),
                });
            };
            if !resolved.insert(selected.kind()) {
                return Err(OrdinaryMirOutputError::DuplicateImportedCallable {
                    index: id.into_raw().into_u32(),
                });
            }
        }

        let referenced = referenced_imported_callables(&module);
        for (id, _) in module.meta.imported_core_callables.iter() {
            if !referenced.contains(&id) {
                return Err(OrdinaryMirOutputError::UnreferencedImportedCallable {
                    index: id.into_raw().into_u32(),
                });
            }
        }

        validate_imported_dependencies(&module, &imported_dependencies)?;

        Ok(Self {
            module,
            imported_core,
            imported_dependencies,
        })
    }

    pub const fn module(&self) -> &Module {
        &self.module
    }

    pub const fn imported_core(&self) -> &SelectedImportedMirSet<'a> {
        &self.imported_core
    }

    pub const fn imported_dependencies(&self) -> &SelectedDependencyMirSet {
        &self.imported_dependencies
    }

    pub fn into_parts(self) -> (Module, SelectedImportedMirSet<'a>, SelectedDependencyMirSet) {
        (self.module, self.imported_core, self.imported_dependencies)
    }
}

fn validate_imported_dependencies(
    module: &Module,
    selected: &SelectedDependencyMirSet,
) -> Result<(), OrdinaryMirOutputError> {
    if module.meta.imported_dependency_callables.len() != selected.len() {
        return Err(
            OrdinaryMirOutputError::ImportedDependencySelectionCountMismatch {
                module: module.meta.imported_dependency_callables.len(),
                selected: selected.len(),
            },
        );
    }
    let mut declarations = HashSet::with_capacity(selected.len());
    for (id, callable) in module.meta.imported_dependency_callables.iter() {
        let Some(selected) = selected.resolve_callable(callable.reference()) else {
            return Err(OrdinaryMirOutputError::ForeignImportedDependencyCallable {
                index: id.into_raw().into_u32(),
            });
        };
        if !declarations.insert((selected.provider(), selected.declaration())) {
            return Err(
                OrdinaryMirOutputError::DuplicateImportedDependencyCallable {
                    index: id.into_raw().into_u32(),
                },
            );
        }
    }

    let referenced = referenced_dependency_callables(module);
    for (id, _) in module.meta.imported_dependency_callables.iter() {
        if !referenced.contains(&id) {
            return Err(
                OrdinaryMirOutputError::UnreferencedImportedDependencyCallable {
                    index: id.into_raw().into_u32(),
                },
            );
        }
    }
    Ok(())
}

fn referenced_imported_callables(module: &Module) -> HashSet<ImportedCoreCallableUseId> {
    let mut referenced = HashSet::new();
    for (_, function) in module.functions.iter() {
        for (_, block) in function.body.blocks.iter() {
            for statement in &block.statements {
                let StatementKind::Call(effect) = &statement.kind else {
                    continue;
                };
                let call = match effect {
                    crate::CallEffect::Unit(call) | crate::CallEffect::Value { call, .. } => call,
                };
                if let Callee::CoreExternal(callable) = call.target.callee {
                    referenced.insert(callable);
                }
            }
        }
    }
    referenced
}

fn referenced_dependency_callables(module: &Module) -> HashSet<ImportedDependencyMirCallableId> {
    let mut referenced = HashSet::new();
    for (_, function) in module.functions.iter() {
        for (_, block) in function.body.blocks.iter() {
            for statement in &block.statements {
                let StatementKind::Call(effect) = &statement.kind else {
                    continue;
                };
                let call = match effect {
                    crate::CallEffect::Unit(call) | crate::CallEffect::Value { call, .. } => call,
                };
                if let Callee::DependencyStrong(callable) = call.target.callee {
                    referenced.insert(callable);
                }
            }
        }
    }
    referenced
}

#[derive(Debug)]
pub enum OrdinaryMirOutputError {
    CurrentConeIsCore,
    InvalidModule(MirValidationError),
    ImportedDependencyConsumerMismatch {
        module: scoop_identity::ConeIdentity,
        selected: scoop_identity::ConeIdentity,
    },
    SelectionCountMismatch {
        module: usize,
        selected: usize,
    },
    ForeignImportedCallable {
        index: u32,
    },
    DuplicateImportedCallable {
        index: u32,
    },
    UnreferencedImportedCallable {
        index: u32,
    },
    ImportedDependencySelectionCountMismatch {
        module: usize,
        selected: usize,
    },
    ForeignImportedDependencyCallable {
        index: u32,
    },
    DuplicateImportedDependencyCallable {
        index: u32,
    },
    UnreferencedImportedDependencyCallable {
        index: u32,
    },
}

impl fmt::Display for OrdinaryMirOutputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot seal ordinary MIR output: {self:?}")
    }
}

impl std::error::Error for OrdinaryMirOutputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidModule(error) => Some(error),
            Self::CurrentConeIsCore
            | Self::ImportedDependencyConsumerMismatch { .. }
            | Self::SelectionCountMismatch { .. }
            | Self::ForeignImportedCallable { .. }
            | Self::DuplicateImportedCallable { .. }
            | Self::UnreferencedImportedCallable { .. }
            | Self::ImportedDependencySelectionCountMismatch { .. }
            | Self::ForeignImportedDependencyCallable { .. }
            | Self::DuplicateImportedDependencyCallable { .. }
            | Self::UnreferencedImportedDependencyCallable { .. } => None,
        }
    }
}
