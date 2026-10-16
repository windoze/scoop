use std::collections::HashMap;

use scoop_identity::{InitializationCallableRole, PersistentInitializationUnitId};

use super::*;

/// One local unit with both exact generated roles bound to emitted bodies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StrongInitializationUnitMaterializationRoot {
    unit: InitializationUnitId,
    identity: PersistentInitializationUnitId,
    initializer: CallableMaterializationRoot,
    ensure: CallableMaterializationRoot,
}
impl StrongInitializationUnitMaterializationRoot {
    pub const fn unit(self) -> InitializationUnitId {
        self.unit
    }
    pub const fn identity(self) -> PersistentInitializationUnitId {
        self.identity
    }
    pub const fn initializer(self) -> CallableMaterializationRoot {
        self.initializer
    }
    pub const fn ensure(self) -> CallableMaterializationRoot {
        self.ensure
    }
}

#[derive(Debug)]
pub enum StrongInitializationUnitError {
    MissingBody {
        unit: PersistentInitializationUnitId,
        role: InitializationCallableRole,
        function: FunctionId,
    },
}
impl std::fmt::Display for StrongInitializationUnitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cannot bind MIR initialization unit: {self:?}")
    }
}
impl std::error::Error for StrongInitializationUnitError {}

pub(super) fn bind(
    module: &Module,
    callable_roots: &[CallableMaterializationRoot],
) -> Result<Vec<StrongInitializationUnitMaterializationRoot>, StrongInitializationUnitError> {
    let roots: HashMap<_, _> = callable_roots
        .iter()
        .map(|root| (root.function(), *root))
        .collect();
    module
        .initialization_units
        .iter()
        .map(|(unit, definition)| {
            let identity = definition.identity.id();
            let lookup = |function, role| {
                roots
                    .get(&function)
                    .copied()
                    .ok_or(StrongInitializationUnitError::MissingBody {
                        unit: identity,
                        role,
                        function,
                    })
            };
            Ok(StrongInitializationUnitMaterializationRoot {
                unit,
                identity,
                initializer: lookup(
                    definition.initializer,
                    InitializationCallableRole::Initializer,
                )?,
                ensure: lookup(definition.ensure, InitializationCallableRole::Ensure)?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests;
