//! Symbol and owner queries shared by every dependency strong subject.

use std::collections::BTreeMap;

use scoop_identity::{
    CallableBodyKey, ConeIdentity, LinkageClass, PersistentCallableBodyId, PersistentSymbolKey,
    PersistentSymbolRequest, StrongCallableDefinitionOwner, StrongDefinitionEntity,
    StrongDefinitionRole,
};
use scoop_lir::StrongExternalLirBridgeV1;

use super::CrossConeStrongRequirementValidationError;
use crate::SlibMemberId;
use crate::link_object::{
    CanonicalDefinedLinkSymbolOwnerSetV1, LinkDefinitionOwnerV1, StrongDefinitionOwnerV1,
};

pub(super) fn dependency_index(
    consumer: ConeIdentity,
    owners: &[CanonicalDefinedLinkSymbolOwnerSetV1],
) -> Result<
    BTreeMap<ConeIdentity, &CanonicalDefinedLinkSymbolOwnerSetV1>,
    CrossConeStrongRequirementValidationError,
> {
    let mut index = BTreeMap::new();
    for owner_set in owners {
        let provider = owner_set.producer();
        if provider == consumer {
            return Err(CrossConeStrongRequirementValidationError::SelfDependency { provider });
        }
        if index.insert(provider, owner_set).is_some() {
            return Err(CrossConeStrongRequirementValidationError::DuplicateProvider { provider });
        }
    }
    Ok(index)
}

pub(super) fn resolve_owner(
    dependencies: &BTreeMap<ConeIdentity, &CanonicalDefinedLinkSymbolOwnerSetV1>,
    provider: ConeIdentity,
    name: &[u8],
    expected: StrongDefinitionOwnerV1,
) -> Result<SlibMemberId, CrossConeStrongRequirementValidationError> {
    let owners = dependencies
        .get(&provider)
        .ok_or(CrossConeStrongRequirementValidationError::MissingProvider { provider })?;
    let owner = owners
        .owners()
        .binary_search_by(|owner| owner.symbol().cmp(name))
        .ok()
        .map(|index| &owners.owners()[index])
        .ok_or_else(
            || CrossConeStrongRequirementValidationError::MissingDependencyOwner {
                provider,
                name: name.to_vec(),
                owner: expected,
            },
        )?;
    if owner.owner() != LinkDefinitionOwnerV1::StrongDefinition(expected) {
        return Err(
            CrossConeStrongRequirementValidationError::DependencyOwnerMismatch {
                provider,
                name: name.to_vec(),
                expected,
                actual: Box::new(owner.owner()),
            },
        );
    }
    Ok(owner.member())
}

pub(super) fn type_registration_support(
    bridge: &StrongExternalLirBridgeV1,
) -> Result<
    Option<(PersistentSymbolRequest, StrongDefinitionOwnerV1)>,
    CrossConeStrongRequirementValidationError,
> {
    let StrongExternalLirBridgeV1::TypeDescriptor(bridge) = bridge else {
        return Ok(None);
    };
    let target = bridge.target();
    let request = PersistentSymbolRequest::new(
        PersistentSymbolKey::TypeRegistration(target),
        LinkageClass::ConeStrong,
    )
    .map_err(|source| {
        CrossConeStrongRequirementValidationError::InvalidTypeRegistrationSymbol { target, source }
    })?;
    let owner = StrongDefinitionOwnerV1::new(
        StrongDefinitionEntity::exact_type(target),
        StrongDefinitionRole::TypeRegistration,
    )
    .map_err(
        |_| CrossConeStrongRequirementValidationError::InvalidExpectedOwner {
            entity: StrongDefinitionEntity::exact_type(target),
            role: StrongDefinitionRole::TypeRegistration,
        },
    )?;
    Ok(Some((request, owner)))
}

pub(super) fn expected_symbol(
    bridge: &StrongExternalLirBridgeV1,
) -> scoop_identity::PersistentSymbolRequest {
    match bridge {
        StrongExternalLirBridgeV1::Callable(bridge) => bridge.bridge().expected_symbol(),
        StrongExternalLirBridgeV1::TypeDescriptor(bridge) => bridge.expected_symbol(),
    }
}

pub(super) fn expected_owner(
    bridge: &StrongExternalLirBridgeV1,
) -> Result<StrongDefinitionOwnerV1, CrossConeStrongRequirementValidationError> {
    let (entity, role) = match bridge {
        StrongExternalLirBridgeV1::Callable(bridge) => {
            return callable_owner(bridge.bridge().target());
        }
        StrongExternalLirBridgeV1::TypeDescriptor(bridge) => (
            StrongDefinitionEntity::exact_type(bridge.target()),
            StrongDefinitionRole::TypeDescriptor,
        ),
    };
    StrongDefinitionOwnerV1::new(entity, role).map_err(|_| {
        CrossConeStrongRequirementValidationError::InvalidExpectedOwner { entity, role }
    })
}

pub(super) fn callable_owner(
    target: StrongCallableDefinitionOwner,
) -> Result<StrongDefinitionOwnerV1, CrossConeStrongRequirementValidationError> {
    let body = PersistentCallableBodyId::from_key(&CallableBodyKey::strong(target))
        .map_err(|_| CrossConeStrongRequirementValidationError::InvalidCallableOwner { target })?;
    let entity = StrongDefinitionEntity::callable_body(body);
    let role = StrongDefinitionRole::CallableBody;
    StrongDefinitionOwnerV1::new(entity, role).map_err(|_| {
        CrossConeStrongRequirementValidationError::InvalidExpectedOwner { entity, role }
    })
}
