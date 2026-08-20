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
    DuplicateUnit {
        unit: PersistentInitializationUnitId,
    },
    Hash(scoop_wire::HashError),
    Odr(scoop_identity::OdrMemberIdentityError),
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
        actual: Box<CallableSignatureSubject>,
    },
    Signature {
        unit: PersistentInitializationUnitId,
        role: InitializationCallableRole,
        function: FunctionId,
    },
}
impl std::fmt::Display for StrongInitializationUnitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid MIR initialization unit: {self:?}")
    }
}
impl std::error::Error for StrongInitializationUnitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Identity { source, .. } => Some(source),
            Self::Hash(source) => Some(source),
            Self::Odr(source) => Some(source),
            _ => None,
        }
    }
}

pub(super) fn validate(
    module: &Module,
    callable_roots: &[CallableMaterializationRoot],
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
        let initializer = role(
            module,
            &roots,
            identity,
            unit.identity.key(),
            unit.initializer,
            InitializationCallableRole::Initializer,
        )?;
        let ensure = role(
            module,
            &roots,
            identity,
            unit.identity.key(),
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
    roots: &HashMap<FunctionId, CallableMaterializationRoot>,
    unit: PersistentInitializationUnitId,
    key: &InitializationUnitKey,
    function: FunctionId,
    role: InitializationCallableRole,
) -> Result<CallableMaterializationRoot, StrongInitializationUnitError> {
    let declaration = match key {
        InitializationUnitKey::GenericDelegatedExtensionApplication { property, .. } => {
            PersistentInitializationUnitId::from_key(&InitializationUnitKey::ExtensionProperty(
                *property,
            ))
            .map_err(StrongInitializationUnitError::Hash)?
        }
        InitializationUnitKey::GenericCompanionApplication { companion, .. } => {
            PersistentInitializationUnitId::from_key(
                &InitializationUnitKey::GenericCompanionTemplate(*companion),
            )
            .map_err(StrongInitializationUnitError::Hash)?
        }
        _ => unit,
    };
    let expected = PersistentGeneratedCallableId::from_key(&GeneratedCallableKey::Initialization {
        unit: declaration,
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
    let expected_subject = if let Some(key) = key.specialization_key() {
        let group = scoop_identity::OdrGroupId::from_key(&key)
            .map_err(StrongInitializationUnitError::Hash)?;
        let member = scoop_identity::OdrMemberKey::new(
            group,
            scoop_identity::OdrMemberRole::CallableBody,
            scoop_identity::OdrMemberDiscriminator::GeneratedCallable(expected),
        )
        .and_then(|key| scoop_identity::CallableOdrMemberId::from_key(&key))
        .map_err(StrongInitializationUnitError::Odr)?;
        CallableSignatureSubject::Odr(member)
    } else {
        CallableSignatureSubject::Strong(CallableOwner::Generated(expected))
    };
    if root.subject() != expected_subject {
        return Err(StrongInitializationUnitError::WrongRole {
            unit,
            role,
            function,
            expected,
            actual: Box::new(root.subject()),
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
    let signature = module.meta.callable_signatures.get(root.subject());
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
