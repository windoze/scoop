//! Canonical member-aware owners for all verified external strong definitions.

use std::collections::BTreeSet;
use std::fmt;

use scoop_identity::{
    ConeIdentity, GeneratedBridgeAtomId, ObjectDefinitionAtomId, ObjectDefinitionPlanKey,
    StrongDefinitionEntity, StrongDefinitionEntityKind, StrongDefinitionRole,
};

use super::{
    PlannedStrongObjectSymbolRoleV1, VerifiedBoundaryRoleV1,
    VerifiedCurrentConeStrongRelocationClosureV1,
};
use crate::SlibMemberId;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct StrongDefinitionOwnerV1 {
    entity: StrongDefinitionEntity,
    role: StrongDefinitionRole,
}

impl StrongDefinitionOwnerV1 {
    pub fn new(
        entity: StrongDefinitionEntity,
        role: StrongDefinitionRole,
    ) -> Result<Self, StrongDefinitionOwnerValidationError> {
        ObjectDefinitionPlanKey::strong(ConeIdentity::CORE, entity, role).map_err(|_| {
            StrongDefinitionOwnerValidationError::EntityRoleMismatch { entity, role }
        })?;
        if matches!(
            entity.kind(),
            StrongDefinitionEntityKind::GeneratedBridgeAtom(_)
                | StrongDefinitionEntityKind::ConeImage(_)
        ) {
            return Err(StrongDefinitionOwnerValidationError::ReservedOwnerKind { entity, role });
        }
        Ok(Self { entity, role })
    }

    pub const fn entity(self) -> StrongDefinitionEntity {
        self.entity
    }

    pub const fn role(self) -> StrongDefinitionRole {
        self.role
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrongDefinitionOwnerValidationError {
    EntityRoleMismatch {
        entity: StrongDefinitionEntity,
        role: StrongDefinitionRole,
    },
    ReservedOwnerKind {
        entity: StrongDefinitionEntity,
        role: StrongDefinitionRole,
    },
}

impl fmt::Display for StrongDefinitionOwnerValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid strong definition owner: {self:?}")
    }
}

impl std::error::Error for StrongDefinitionOwnerValidationError {}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LinkDefinitionOwnerV1 {
    StrongDefinition(StrongDefinitionOwnerV1),
    GeneratedBridge(GeneratedBridgeAtomId),
    ConeImage(ConeIdentity),
    VerifierBoundary {
        atom: ObjectDefinitionAtomId,
        boundary: VerifiedBoundaryRoleV1,
    },
}

impl LinkDefinitionOwnerV1 {
    pub fn from_strong_primary(
        entity: StrongDefinitionEntity,
        role: StrongDefinitionRole,
    ) -> Result<Self, StrongDefinitionOwnerValidationError> {
        match (entity.kind(), role) {
            (
                StrongDefinitionEntityKind::GeneratedBridgeAtom(atom),
                StrongDefinitionRole::GeneratedBridge,
            ) => Ok(Self::GeneratedBridge(atom)),
            (
                StrongDefinitionEntityKind::ConeImage(cone),
                StrongDefinitionRole::ImageDescriptor,
            ) => Ok(Self::ConeImage(cone)),
            _ => StrongDefinitionOwnerV1::new(entity, role).map(Self::StrongDefinition),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefinedLinkSymbolOwnerV1 {
    member: SlibMemberId,
    symbol: Vec<u8>,
    owner: LinkDefinitionOwnerV1,
}

impl DefinedLinkSymbolOwnerV1 {
    pub const fn member(&self) -> SlibMemberId {
        self.member
    }

    pub fn symbol(&self) -> &[u8] {
        &self.symbol
    }

    pub const fn owner(&self) -> LinkDefinitionOwnerV1 {
        self.owner
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalDefinedLinkSymbolOwnerSetV1 {
    producer: ConeIdentity,
    owners: Vec<DefinedLinkSymbolOwnerV1>,
}

impl CanonicalDefinedLinkSymbolOwnerSetV1 {
    pub fn from_verified_strong_closure(
        closure: &VerifiedCurrentConeStrongRelocationClosureV1,
    ) -> Result<Self, DefinedLinkSymbolOwnerBuildError> {
        let mut primary_owners = BTreeSet::new();
        let mut owners = Vec::new();
        for member in closure.members() {
            for symbol in member.definitions().symbols() {
                let owner = match symbol.role() {
                    PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                        owner,
                        definition_role,
                        ..
                    } => {
                        let link_owner = primary_owner(owner, definition_role)?;
                        if !primary_owners.insert(link_owner) {
                            return Err(DefinedLinkSymbolOwnerBuildError::DuplicatePrimaryOwner(
                                link_owner,
                            ));
                        }
                        link_owner
                    }
                    PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart { atom, .. } => {
                        LinkDefinitionOwnerV1::VerifierBoundary {
                            atom,
                            boundary: VerifiedBoundaryRoleV1::Start,
                        }
                    }
                    PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd { atom, .. } => {
                        LinkDefinitionOwnerV1::VerifierBoundary {
                            atom,
                            boundary: VerifiedBoundaryRoleV1::End,
                        }
                    }
                };
                owners.push(DefinedLinkSymbolOwnerV1 {
                    member: member.member(),
                    symbol: symbol.macho_name().to_vec(),
                    owner,
                });
            }
        }
        owners.sort_unstable_by(|left, right| {
            (&left.symbol, left.member, left.owner).cmp(&(&right.symbol, right.member, right.owner))
        });
        if let Some(pair) = owners
            .windows(2)
            .find(|pair| pair[0].symbol == pair[1].symbol)
        {
            return Err(DefinedLinkSymbolOwnerBuildError::DuplicateSymbol(
                pair[0].symbol.clone(),
            ));
        }
        Ok(Self {
            producer: closure.producer(),
            owners,
        })
    }

    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub fn owners(&self) -> &[DefinedLinkSymbolOwnerV1] {
        &self.owners
    }
}

fn primary_owner(
    entity: StrongDefinitionEntity,
    role: StrongDefinitionRole,
) -> Result<LinkDefinitionOwnerV1, DefinedLinkSymbolOwnerBuildError> {
    LinkDefinitionOwnerV1::from_strong_primary(entity, role)
        .map_err(|_| DefinedLinkSymbolOwnerBuildError::EntityRoleMismatch { entity, role })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DefinedLinkSymbolOwnerBuildError {
    EntityRoleMismatch {
        entity: StrongDefinitionEntity,
        role: StrongDefinitionRole,
    },
    DuplicatePrimaryOwner(LinkDefinitionOwnerV1),
    DuplicateSymbol(Vec<u8>),
}

impl fmt::Display for DefinedLinkSymbolOwnerBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid defined link-symbol owner set: {self:?}")
    }
}

impl std::error::Error for DefinedLinkSymbolOwnerBuildError {}

#[cfg(test)]
mod tests;
