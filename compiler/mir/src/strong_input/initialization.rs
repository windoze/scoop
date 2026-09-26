use std::collections::{HashMap, HashSet};

use scoop_identity::{
    CallableOwner, Effect, GeneratedCallableKey, InitializationCallableRole, InitializationUnitKey,
    PersistentGeneratedCallableId, PersistentInitializationUnitId,
};

use super::*;

/// One local unit with both exact generated roles bound to emitted bodies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StrongInitializationUnitMaterializationRoot {
    unit: InitializationUnitId,
    identity: PersistentInitializationUnitId,
    initializer: StrongCallableMaterializationRoot,
    ensure: StrongCallableMaterializationRoot,
}
impl StrongInitializationUnitMaterializationRoot {
    pub const fn unit(self) -> InitializationUnitId {
        self.unit
    }
    pub const fn identity(self) -> PersistentInitializationUnitId {
        self.identity
    }
    pub const fn initializer(self) -> StrongCallableMaterializationRoot {
        self.initializer
    }
    pub const fn ensure(self) -> StrongCallableMaterializationRoot {
        self.ensure
    }
}

#[derive(Debug)]
pub enum StrongInitializationUnitError {
    DuplicateUnit {
        unit: PersistentInitializationUnitId,
    },
    GenericUnit {
        unit: PersistentInitializationUnitId,
    },
    Identity {
        unit: PersistentInitializationUnitId,
        role: InitializationCallableRole,
        source: scoop_identity::GeneratedCallableIdentityError,
    },
    MissingBody {
        unit: PersistentInitializationUnitId,
        role: InitializationCallableRole,
        function: FunctionId,
    },
    WrongRole {
        unit: PersistentInitializationUnitId,
        role: InitializationCallableRole,
        function: FunctionId,
        expected: PersistentGeneratedCallableId,
        actual: CallableOwner,
    },
    Signature {
        unit: PersistentInitializationUnitId,
        role: InitializationCallableRole,
        function: FunctionId,
    },
}
impl std::fmt::Display for StrongInitializationUnitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid Strong MIR initialization unit: {self:?}")
    }
}
impl std::error::Error for StrongInitializationUnitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Identity { source, .. } => Some(source),
            _ => None,
        }
    }
}

pub(super) fn validate(
    module: &Module,
    callable_roots: &[StrongCallableMaterializationRoot],
) -> Result<Vec<StrongInitializationUnitMaterializationRoot>, StrongInitializationUnitError> {
    let roots: HashMap<_, _> = callable_roots
        .iter()
        .map(|root| (root.function(), *root))
        .collect();
    let mut seen = HashSet::new();
    let mut units = Vec::with_capacity(module.initialization_units.len());
    for (id, unit) in module.initialization_units.iter() {
        let identity = unit.identity.id();
        if !seen.insert(identity) {
            return Err(StrongInitializationUnitError::DuplicateUnit { unit: identity });
        }
        if matches!(
            unit.identity.key(),
            InitializationUnitKey::GenericDelegatedExtensionApplication { .. }
        ) {
            return Err(StrongInitializationUnitError::GenericUnit { unit: identity });
        }
        let initializer = role(
            module,
            &roots,
            identity,
            unit.initializer,
            InitializationCallableRole::Initializer,
        )?;
        let ensure = role(
            module,
            &roots,
            identity,
            unit.ensure,
            InitializationCallableRole::Ensure,
        )?;
        units.push(StrongInitializationUnitMaterializationRoot {
            unit: id,
            identity,
            initializer,
            ensure,
        });
    }
    Ok(units)
}

fn role(
    module: &Module,
    roots: &HashMap<FunctionId, StrongCallableMaterializationRoot>,
    unit: PersistentInitializationUnitId,
    function: FunctionId,
    role: InitializationCallableRole,
) -> Result<StrongCallableMaterializationRoot, StrongInitializationUnitError> {
    let expected = PersistentGeneratedCallableId::from_key(&GeneratedCallableKey::Initialization {
        unit,
        role,
    })
    .map_err(|source| StrongInitializationUnitError::Identity { unit, role, source })?;
    let root = *roots
        .get(&function)
        .ok_or(StrongInitializationUnitError::MissingBody {
            unit,
            role,
            function,
        })?;
    if root.implementation() != CallableOwner::Generated(expected) {
        return Err(StrongInitializationUnitError::WrongRole {
            unit,
            role,
            function,
            expected,
            actual: root.implementation(),
        });
    }
    if function.into_raw().into_u32() as usize >= module.functions.len() {
        return Err(StrongInitializationUnitError::MissingBody {
            unit,
            role,
            function,
        });
    }
    let body = &module.functions[function];
    let signature = module
        .meta
        .callable_signatures
        .get(CallableSignatureSubject::Strong(root.implementation()));
    let unit_exact = module
        .meta
        .source_exact_types
        .get(&Type::Unit)
        .map(|record| record.identity_record().id());
    if body.gc_effect != crate::GcEffect::Managed
        || !body.params.is_empty()
        || body.return_ty != Type::Unit
        || !signature.is_some_and(|record| {
            let signature = record.signature();
            signature.effect() == Effect::Ordinary
                && !signature.receiver().is_present()
                && signature.parameters().is_empty()
                && Some(signature.result()) == unit_exact
        })
    {
        return Err(StrongInitializationUnitError::Signature {
            unit,
            role,
            function,
        });
    }
    Ok(root)
}

#[cfg(test)]
mod tests;
