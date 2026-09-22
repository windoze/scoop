//! Typed resolution of external relocation candidates against core bridges.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use scoop_identity::{
    CallableBodyKey, ConeIdentity, LinkageClass, PersistentCallableBodyId, PersistentExactTypeId,
    PersistentSymbolError, PersistentSymbolKey, PersistentSymbolRequest,
    StrongCallableDefinitionOwner, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{LirTargetProfile, StrongExternalLirBridgeSurfaceV1, StrongExternalLirBridgeV1};

use super::{
    CanonicalDefinedLinkSymbolOwnerSetV1, CanonicalUndefinedRelocationUseV1, LinkDefinitionOwnerV1,
    StrongDefinitionOwnerV1, StrongRelocationBindingV1, StrongRelocationResolutionV1,
    VerifiedCurrentConeStrongRelocationClosureV1,
};
use crate::SlibMemberId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreStrongRequirementUseV1 {
    use_site: CanonicalUndefinedRelocationUseV1,
    core_member: SlibMemberId,
    owner: StrongDefinitionOwnerV1,
    bridge: StrongExternalLirBridgeV1,
}

impl CoreStrongRequirementUseV1 {
    pub const fn use_site(&self) -> &CanonicalUndefinedRelocationUseV1 {
        &self.use_site
    }

    pub const fn core_member(&self) -> SlibMemberId {
        self.core_member
    }

    pub const fn owner(&self) -> StrongDefinitionOwnerV1 {
        self.owner
    }

    pub const fn bridge(&self) -> &StrongExternalLirBridgeV1 {
        &self.bridge
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedCoreStrongRequirementClosureV1 {
    target: LirTargetProfile,
    strong_closure: VerifiedCurrentConeStrongRelocationClosureV1,
    external_bridges: StrongExternalLirBridgeSurfaceV1,
    core_owners: CanonicalDefinedLinkSymbolOwnerSetV1,
    core_requirements: Vec<CoreStrongRequirementUseV1>,
    remaining_external_candidates: Vec<StrongRelocationBindingV1>,
}

impl VerifiedCoreStrongRequirementClosureV1 {
    pub const fn target(&self) -> LirTargetProfile {
        self.target
    }

    pub const fn producer(&self) -> ConeIdentity {
        self.strong_closure.producer()
    }

    pub const fn strong_closure(&self) -> &VerifiedCurrentConeStrongRelocationClosureV1 {
        &self.strong_closure
    }

    pub const fn external_bridges(&self) -> &StrongExternalLirBridgeSurfaceV1 {
        &self.external_bridges
    }

    pub const fn core_owners(&self) -> &CanonicalDefinedLinkSymbolOwnerSetV1 {
        &self.core_owners
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
    core_owners: CanonicalDefinedLinkSymbolOwnerSetV1,
) -> Result<VerifiedCoreStrongRequirementClosureV1, CoreStrongRequirementValidationError> {
    if strong_closure.producer() != external_bridges.producer() {
        return Err(CoreStrongRequirementValidationError::ProducerMismatch {
            object: strong_closure.producer(),
            bridge: external_bridges.producer(),
        });
    }
    if core_owners.producer() != ConeIdentity::CORE {
        return Err(
            CoreStrongRequirementValidationError::CoreOwnerProducerMismatch {
                actual: core_owners.producer(),
            },
        );
    }
    let normalization = target.contract().native_symbol_normalization();
    let mut bridges = BTreeMap::<
        Vec<u8>,
        (
            &StrongExternalLirBridgeV1,
            SlibMemberId,
            StrongDefinitionOwnerV1,
        ),
    >::new();
    let mut support_bridges = BTreeMap::<
        Vec<u8>,
        (
            &StrongExternalLirBridgeV1,
            SlibMemberId,
            StrongDefinitionOwnerV1,
        ),
    >::new();
    for bridge in external_bridges.bridges() {
        let request = expected_symbol(bridge);
        let name = normalization
            .compiler_generated_object_symbol(request.symbol().as_str())
            .into_bytes();
        let expected_owner = expected_owner(bridge)?;
        let Some(core_owner) = core_owners
            .owners()
            .binary_search_by(|owner| owner.symbol().cmp(&name))
            .ok()
            .map(|index| &core_owners.owners()[index])
        else {
            return Err(CoreStrongRequirementValidationError::MissingCoreOwner {
                name,
                owner: expected_owner,
            });
        };
        if core_owner.owner() != LinkDefinitionOwnerV1::StrongDefinition(expected_owner) {
            return Err(CoreStrongRequirementValidationError::CoreOwnerMismatch {
                name,
                expected: expected_owner,
                actual: core_owner.owner(),
            });
        }
        if bridges
            .insert(name.clone(), (bridge, core_owner.member(), expected_owner))
            .is_some()
        {
            return Err(
                CoreStrongRequirementValidationError::DuplicateNormalizedBridgeSymbol { name },
            );
        }
        if let Some((request, support_owner)) = type_registration_support(bridge)? {
            let support_name = normalization
                .compiler_generated_object_symbol(request.symbol().as_str())
                .into_bytes();
            let Some(core_owner) = core_owners
                .owners()
                .binary_search_by(|owner| owner.symbol().cmp(&support_name))
                .ok()
                .map(|index| &core_owners.owners()[index])
            else {
                return Err(CoreStrongRequirementValidationError::MissingCoreOwner {
                    name: support_name,
                    owner: support_owner,
                });
            };
            if core_owner.owner() != LinkDefinitionOwnerV1::StrongDefinition(support_owner) {
                return Err(CoreStrongRequirementValidationError::CoreOwnerMismatch {
                    name: support_name,
                    expected: support_owner,
                    actual: core_owner.owner(),
                });
            }
            if bridges.contains_key(&support_name)
                || support_bridges
                    .insert(
                        support_name.clone(),
                        (bridge, core_owner.member(), support_owner),
                    )
                    .is_some()
            {
                return Err(
                    CoreStrongRequirementValidationError::DuplicateNormalizedBridgeSymbol {
                        name: support_name,
                    },
                );
            }
        }
    }

    let mut used = BTreeSet::new();
    let mut core_requirements = Vec::new();
    let mut remaining_external_candidates = Vec::new();
    for binding in strong_closure.bindings() {
        let StrongRelocationResolutionV1::ExternalCandidate { .. } = binding.resolution() else {
            continue;
        };
        if let Some((bridge, core_member, owner)) = bridges.get(binding.symbol()) {
            used.insert(binding.symbol().to_vec());
            core_requirements.push(CoreStrongRequirementUseV1 {
                use_site: CanonicalUndefinedRelocationUseV1::from(binding),
                core_member: *core_member,
                owner: *owner,
                bridge: (*bridge).clone(),
            });
        } else if let Some((bridge, core_member, owner)) = support_bridges.get(binding.symbol()) {
            core_requirements.push(CoreStrongRequirementUseV1 {
                use_site: CanonicalUndefinedRelocationUseV1::from(binding),
                core_member: *core_member,
                owner: *owner,
                bridge: (*bridge).clone(),
            });
        } else {
            remaining_external_candidates.push(binding.clone());
        }
    }
    if let Some(name) = bridges.iter().find_map(|(name, (bridge, _, _))| {
        matches!(bridge, StrongExternalLirBridgeV1::Callable(_))
            .then(|| name)
            .filter(|name| !used.contains(*name))
    }) {
        return Err(
            CoreStrongRequirementValidationError::UnusedExternalCallableBridge {
                name: name.clone(),
            },
        );
    }

    Ok(VerifiedCoreStrongRequirementClosureV1 {
        target,
        strong_closure,
        external_bridges,
        core_owners,
        core_requirements,
        remaining_external_candidates,
    })
}

fn type_registration_support(
    bridge: &StrongExternalLirBridgeV1,
) -> Result<
    Option<(PersistentSymbolRequest, StrongDefinitionOwnerV1)>,
    CoreStrongRequirementValidationError,
> {
    let StrongExternalLirBridgeV1::TypeDescriptor(bridge) = bridge else {
        return Ok(None);
    };
    let target = bridge.target();
    let request =
        PersistentSymbolRequest::new(
            PersistentSymbolKey::TypeRegistration(target),
            LinkageClass::ConeStrong,
        )
        .map_err(|source| {
            CoreStrongRequirementValidationError::InvalidTypeRegistrationSymbol { target, source }
        })?;
    let owner = StrongDefinitionOwnerV1::new(
        StrongDefinitionEntity::exact_type(target),
        StrongDefinitionRole::TypeRegistration,
    )
    .map_err(
        |_| CoreStrongRequirementValidationError::InvalidExpectedOwner {
            entity: StrongDefinitionEntity::exact_type(target),
            role: StrongDefinitionRole::TypeRegistration,
        },
    )?;
    Ok(Some((request, owner)))
}

fn expected_symbol(bridge: &StrongExternalLirBridgeV1) -> scoop_identity::PersistentSymbolRequest {
    match bridge {
        StrongExternalLirBridgeV1::Callable(bridge) => bridge.bridge().expected_symbol(),
        StrongExternalLirBridgeV1::TypeDescriptor(bridge) => bridge.expected_symbol(),
    }
}

fn expected_owner(
    bridge: &StrongExternalLirBridgeV1,
) -> Result<StrongDefinitionOwnerV1, CoreStrongRequirementValidationError> {
    let (entity, role) = match bridge {
        StrongExternalLirBridgeV1::Callable(bridge) => {
            let target = bridge.bridge().target();
            let body = PersistentCallableBodyId::from_key(&CallableBodyKey::strong(target))
                .map_err(
                    |_| CoreStrongRequirementValidationError::InvalidCallableOwner { target },
                )?;
            (
                StrongDefinitionEntity::callable_body(body),
                StrongDefinitionRole::CallableBody,
            )
        }
        StrongExternalLirBridgeV1::TypeDescriptor(bridge) => (
            StrongDefinitionEntity::exact_type(bridge.target()),
            StrongDefinitionRole::TypeDescriptor,
        ),
    };
    StrongDefinitionOwnerV1::new(entity, role)
        .map_err(|_| CoreStrongRequirementValidationError::InvalidExpectedOwner { entity, role })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoreStrongRequirementValidationError {
    ProducerMismatch {
        object: ConeIdentity,
        bridge: ConeIdentity,
    },
    CoreOwnerProducerMismatch {
        actual: ConeIdentity,
    },
    DuplicateNormalizedBridgeSymbol {
        name: Vec<u8>,
    },
    MissingCoreOwner {
        name: Vec<u8>,
        owner: StrongDefinitionOwnerV1,
    },
    CoreOwnerMismatch {
        name: Vec<u8>,
        expected: StrongDefinitionOwnerV1,
        actual: LinkDefinitionOwnerV1,
    },
    InvalidCallableOwner {
        target: StrongCallableDefinitionOwner,
    },
    InvalidExpectedOwner {
        entity: StrongDefinitionEntity,
        role: StrongDefinitionRole,
    },
    InvalidTypeRegistrationSymbol {
        target: PersistentExactTypeId,
        source: PersistentSymbolError,
    },
    UnusedExternalCallableBridge {
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

impl std::error::Error for CoreStrongRequirementValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidTypeRegistrationSymbol { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
