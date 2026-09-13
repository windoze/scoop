//! Typed resolution of external relocation candidates against core bridges.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use scoop_identity::ConeIdentity;
use scoop_lir::{LirTargetProfile, StrongExternalLirBridgeSurfaceV1, StrongExternalLirBridgeV1};

use super::{
    StrongRelocationBindingV1, StrongRelocationResolutionV1,
    VerifiedCurrentConeStrongRelocationClosureV1,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreStrongRequirementUseV1 {
    use_site: StrongRelocationBindingV1,
    bridge: StrongExternalLirBridgeV1,
}

impl CoreStrongRequirementUseV1 {
    pub const fn use_site(&self) -> &StrongRelocationBindingV1 {
        &self.use_site
    }

    pub const fn bridge(&self) -> &StrongExternalLirBridgeV1 {
        &self.bridge
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedCoreStrongRequirementClosureV1 {
    strong_closure: VerifiedCurrentConeStrongRelocationClosureV1,
    external_bridges: StrongExternalLirBridgeSurfaceV1,
    core_requirements: Vec<CoreStrongRequirementUseV1>,
    remaining_external_candidates: Vec<StrongRelocationBindingV1>,
}

impl VerifiedCoreStrongRequirementClosureV1 {
    pub const fn producer(&self) -> ConeIdentity {
        self.strong_closure.producer()
    }

    pub const fn strong_closure(&self) -> &VerifiedCurrentConeStrongRelocationClosureV1 {
        &self.strong_closure
    }

    pub const fn external_bridges(&self) -> &StrongExternalLirBridgeSurfaceV1 {
        &self.external_bridges
    }

    pub fn core_requirements(&self) -> &[CoreStrongRequirementUseV1] {
        &self.core_requirements
    }

    pub fn remaining_external_candidates(&self) -> &[StrongRelocationBindingV1] {
        &self.remaining_external_candidates
    }
}

pub fn verify_core_strong_requirements_v1(
    target: LirTargetProfile,
    strong_closure: VerifiedCurrentConeStrongRelocationClosureV1,
    external_bridges: StrongExternalLirBridgeSurfaceV1,
) -> Result<VerifiedCoreStrongRequirementClosureV1, CoreStrongRequirementValidationError> {
    if strong_closure.producer() != external_bridges.producer() {
        return Err(CoreStrongRequirementValidationError::ProducerMismatch {
            object: strong_closure.producer(),
            bridge: external_bridges.producer(),
        });
    }
    let normalization = target.contract().native_symbol_normalization();
    let mut bridges = BTreeMap::<Vec<u8>, &StrongExternalLirBridgeV1>::new();
    for bridge in external_bridges.bridges() {
        let request = expected_symbol(bridge);
        let name = normalization
            .compiler_generated_object_symbol(request.symbol().as_str())
            .into_bytes();
        if bridges.insert(name.clone(), bridge).is_some() {
            return Err(
                CoreStrongRequirementValidationError::DuplicateNormalizedBridgeSymbol { name },
            );
        }
    }

    let mut used = BTreeSet::new();
    let mut core_requirements = Vec::new();
    let mut remaining_external_candidates = Vec::new();
    for binding in strong_closure.bindings() {
        let StrongRelocationResolutionV1::ExternalCandidate { .. } = binding.resolution() else {
            continue;
        };
        if let Some(bridge) = bridges.get(binding.symbol()) {
            used.insert(binding.symbol().to_vec());
            core_requirements.push(CoreStrongRequirementUseV1 {
                use_site: binding.clone(),
                bridge: (*bridge).clone(),
            });
        } else {
            remaining_external_candidates.push(binding.clone());
        }
    }
    if let Some(name) = bridges.keys().find(|name| !used.contains(*name)) {
        return Err(CoreStrongRequirementValidationError::UnusedExternalBridge {
            name: name.clone(),
        });
    }

    Ok(VerifiedCoreStrongRequirementClosureV1 {
        strong_closure,
        external_bridges,
        core_requirements,
        remaining_external_candidates,
    })
}

fn expected_symbol(bridge: &StrongExternalLirBridgeV1) -> scoop_identity::PersistentSymbolRequest {
    match bridge {
        StrongExternalLirBridgeV1::Callable(bridge) => bridge.expected_symbol(),
        StrongExternalLirBridgeV1::TypeDescriptor(bridge) => bridge.expected_symbol(),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoreStrongRequirementValidationError {
    ProducerMismatch {
        object: ConeIdentity,
        bridge: ConeIdentity,
    },
    DuplicateNormalizedBridgeSymbol {
        name: Vec<u8>,
    },
    UnusedExternalBridge {
        name: Vec<u8>,
    },
}

impl fmt::Display for CoreStrongRequirementValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid core strong requirement closure: {self:?}"
        )
    }
}

impl std::error::Error for CoreStrongRequirementValidationError {}

#[cfg(test)]
mod tests;
