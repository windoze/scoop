//! Structural contracts of the two generated, no-argument startup gateways.
//! General ABI, safepoint identity and root-plan validation remain at the same
//! complete LIR boundary; this module checks only the gateway's closed control flow.

use scoop_identity::{
    DecodedCallableBodyKey, DecodedCallableBodyKeyKind, DefinitionOwner, StorageRole,
};
use scoop_wire::decode_runtime;

use crate::*;

mod body;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GatewayValidationError {
    pub body: PersistentCallableBodyId,
    pub reason: &'static str,
}

impl std::fmt::Display for GatewayValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid startup gateway {}: {}", self.body, self.reason)
    }
}
impl std::error::Error for GatewayValidationError {}

#[derive(Clone, Copy)]
enum Gateway {
    Root {
        entry: LocalFunctionRef,
        failure_root: GlobalId,
    },
    Initialization {
        ensure: ManagedLocalFunctionRef,
    },
}
impl Gateway {
    fn entry(self) -> LocalFunctionRef {
        match self {
            Self::Root { entry, .. } => entry,
            Self::Initialization { ensure } => LocalFunctionRef::Managed(ensure),
        }
    }
}

type Result<T> = std::result::Result<T, GatewayValidationError>;

fn error(function: &Function, reason: &'static str) -> GatewayValidationError {
    GatewayValidationError {
        body: function.callable_body.id(),
        reason,
    }
}

pub fn validate_startup_gateways(module: &Module) -> Result<()> {
    for function in &module.functions {
        let key = decode_runtime::<DecodedCallableBodyKey>(
            function.callable_body.identity_record().key_bytes(),
        )
        .map_err(|_| error(function, "invalid callable body key"))?;
        let target = match key.kind() {
            DecodedCallableBodyKeyKind::RootGateway { root_cone, main } => {
                let LirOutput::Executable { entry } = module.output else {
                    return Err(error(function, "library contains a root gateway"));
                };
                if root_cone.as_array() != module.cone.as_array()
                    || entry_function(module, function, entry)?
                        .callable_body
                        .id()
                        .as_array()
                        != main.as_array()
                {
                    return Err(error(
                        function,
                        "root gateway owner or main differs from the executable",
                    ));
                }
                let roots = module
                    .globals
                    .iter()
                    .filter_map(|(id, global)| {
                        let GlobalInit::Storage { identity, .. } = &global.init else {
                            return None;
                        };
                        let key = identity.identity_record().key();
                        match key.owner() {
                            DefinitionOwner::RootEntry {
                                root_cone: owner,
                                main: target,
                            } if key.role() == StorageRole::RootEntryFailureRoot
                                && owner == module.cone
                                && target.body().as_array() == main.as_array() =>
                            {
                                Some(id)
                            }
                            _ => None,
                        }
                    })
                    .collect::<Vec<_>>();
                let [failure_root] = roots.as_slice() else {
                    return Err(error(
                        function,
                        "root gateway needs one dedicated failure storage",
                    ));
                };
                Gateway::Root {
                    entry,
                    failure_root: *failure_root,
                }
            }
            DecodedCallableBodyKeyKind::InitializationStartupGateway(unit) => {
                let unit = module
                    .initialization_units
                    .values()
                    .find(|candidate| candidate.identity.id().as_array() == unit.as_array())
                    .ok_or_else(|| {
                        error(
                            function,
                            "startup gateway names an absent initialization unit",
                        )
                    })?;
                if unit.schedule != InitializationSchedule::EagerStartup {
                    return Err(error(
                        function,
                        "lazy initialization unit has a startup gateway",
                    ));
                }
                Gateway::Initialization {
                    ensure: unit.ensure,
                }
            }
            _ => continue,
        };
        let entry = entry_function(module, function, target.entry())?;
        if !entry.signature.arguments().is_empty()
            || !matches!(entry.signature.result(), AbiReturn::UnitVoid)
        {
            return Err(error(
                function,
                "gateway target must return Unit without arguments",
            ));
        }
        body::validate(function, target)?;
    }
    Ok(())
}

fn entry_function<'a>(
    module: &'a Module,
    gateway: &Function,
    entry: LocalFunctionRef,
) -> Result<&'a Function> {
    let (reference, effect) = match entry {
        LocalFunctionRef::Managed(reference) => (reference.declaration(), GcEffect::Managed),
        LocalFunctionRef::NoGc(reference) => (reference.declaration(), GcEffect::NoGc),
    };
    let function = module
        .functions
        .get(reference.into_u32() as usize)
        .ok_or_else(|| error(gateway, "gateway target function is absent"))?;
    if function.gc_effect != effect {
        return Err(error(
            gateway,
            "gateway target effect differs from its typed reference",
        ));
    }
    Ok(function)
}

#[cfg(test)]
mod tests;
