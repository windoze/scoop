//! The manifest directory of physical ODR definitions emitted by one Cone.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use scoop_identity::{ObjectDefinitionPlanOwner, OdrGroupId, OdrMemberId, OdrMemberRole};
use scoop_wire::{Encoder, WireEncode};

use super::{
    CallableDefinitionAbiV1, OdrAbiFingerprintV1, OdrMemberAbiV1, PlannedStrongObjectSymbolRoleV1,
    RegistrationAbiV1, VerifiedStrongRegistrationPatchSetV1,
};

mod wire;
pub use wire::{DecodedCanonicalOdrMemberDirectoryV1, OdrMemberDirectoryValidationError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OdrMemberDirectoryEntryV1 {
    member: OdrMemberId,
    role: OdrMemberRole,
    abi: OdrAbiFingerprintV1,
}

impl OdrMemberDirectoryEntryV1 {
    pub const fn member(self) -> OdrMemberId {
        self.member
    }

    pub const fn role(self) -> OdrMemberRole {
        self.role
    }

    pub const fn abi(self) -> OdrAbiFingerprintV1 {
        self.abi
    }

    fn from_fingerprint(value: OdrMemberAbiV1) -> Self {
        Self {
            member: value.member(),
            role: value.role(),
            abi: value.abi(),
        }
    }
}

impl WireEncode for OdrMemberDirectoryEntryV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.member.encode(encoder)?;
        encoder.field(2)?;
        self.role.encode(encoder)?;
        encoder.field(3)?;
        self.abi.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OdrMemberDirectoryGroupV1 {
    group: OdrGroupId,
    members: Vec<OdrMemberDirectoryEntryV1>,
}

impl OdrMemberDirectoryGroupV1 {
    pub const fn group(&self) -> OdrGroupId {
        self.group
    }

    pub fn members(&self) -> &[OdrMemberDirectoryEntryV1] {
        &self.members
    }
}

impl WireEncode for OdrMemberDirectoryGroupV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.group.encode(encoder)?;
        encoder.field(2)?;
        encode_array(encoder, &self.members)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalOdrMemberDirectoryV1 {
    groups: Vec<OdrMemberDirectoryGroupV1>,
}

impl CanonicalOdrMemberDirectoryV1 {
    /// Join existing content fingerprints with the already verified primary
    /// definitions. No object parsing or fingerprint computation is repeated.
    pub fn from_patch_set<D, C, I>(
        patch_set: &VerifiedStrongRegistrationPatchSetV1<D, C, I>,
    ) -> Result<Self, OdrMemberDirectoryProjectionError>
    where
        D: scoop_lir::StrongDescriptorReference,
        C: Clone,
    {
        let closure = patch_set
            .callables()
            .body_objects()
            .registrations()
            .patch_sites()
            .builtins()
            .strong_relocations();
        let physical = closure
            .members()
            .iter()
            .flat_map(|member| member.definitions().symbols())
            .filter_map(|symbol| match (symbol.role(), symbol.definition_owner()) {
                (
                    PlannedStrongObjectSymbolRoleV1::PrimaryDefinition { .. },
                    ObjectDefinitionPlanOwner::Odr { member },
                ) => Some(member),
                _ => None,
            })
            .collect();
        let callables = patch_set
            .callables()
            .fingerprints()
            .iter()
            .flat_map(|value| {
                let body = match value.definition() {
                    CallableDefinitionAbiV1::Strong => None,
                    CallableDefinitionAbiV1::Odr(fingerprint) => Some(fingerprint),
                };
                [body, odr_registration(value.registration())]
                    .into_iter()
                    .flatten()
            });
        let safepoints = patch_set
            .safepoints()
            .fingerprints()
            .iter()
            .filter_map(|value| odr_registration(value.registration()));
        let types = patch_set
            .types()
            .fingerprints()
            .iter()
            .filter_map(|value| odr_registration(value.registration()));
        let immortals = patch_set
            .immortal_objects()
            .fingerprints()
            .iter()
            .flat_map(|value| {
                let object = match value.definition() {
                    super::ImmortalObjectDefinitionAbiV1::Strong => None,
                    super::ImmortalObjectDefinitionAbiV1::Odr(value) => Some(value),
                };
                [object, odr_registration(value.registration())]
                    .into_iter()
                    .flatten()
            });
        let shapes = patch_set.types().shapes().iter().copied();
        let storages = patch_set
            .static_storages()
            .fingerprints()
            .iter()
            .filter_map(|value| odr_registration(value.registration()))
            .chain(
                patch_set
                    .static_storages()
                    .odr_definitions()
                    .iter()
                    .copied(),
            );
        let initializations = patch_set
            .initializations()
            .fingerprints()
            .iter()
            .filter_map(|value| odr_registration(value.registration()))
            .chain(
                patch_set
                    .initializations()
                    .odr_definitions()
                    .iter()
                    .copied(),
            );
        Self::from_members(
            physical,
            callables
                .chain(safepoints)
                .chain(types)
                .chain(immortals)
                .chain(shapes)
                .chain(storages)
                .chain(initializations)
                .map(|value| {
                    (
                        value.group(),
                        OdrMemberDirectoryEntryV1::from_fingerprint(value),
                    )
                }),
        )
    }

    pub fn groups(&self) -> &[OdrMemberDirectoryGroupV1] {
        &self.groups
    }

    fn from_members(
        mut physical: BTreeSet<OdrMemberId>,
        entries: impl IntoIterator<Item = (OdrGroupId, OdrMemberDirectoryEntryV1)>,
    ) -> Result<Self, OdrMemberDirectoryProjectionError> {
        let mut members = BTreeMap::new();
        for (group, entry) in entries {
            if !physical_role(entry.role) {
                return Err(OdrMemberDirectoryProjectionError::InvalidPhysicalRole {
                    member: entry.member,
                    role: entry.role,
                });
            }
            if members.insert(entry.member, (group, entry)).is_some() {
                return Err(OdrMemberDirectoryProjectionError::DuplicateMember(
                    entry.member,
                ));
            }
            if !physical.remove(&entry.member) {
                return Err(OdrMemberDirectoryProjectionError::UnexpectedMember(
                    entry.member,
                ));
            }
        }
        if let Some(member) = physical.first() {
            return Err(OdrMemberDirectoryProjectionError::MissingMemberFingerprint(
                *member,
            ));
        }
        let mut groups = BTreeMap::<_, Vec<_>>::new();
        for (group, entry) in members.into_values() {
            groups.entry(group).or_default().push(entry);
        }
        Ok(Self {
            groups: groups
                .into_iter()
                .map(|(group, members)| OdrMemberDirectoryGroupV1 { group, members })
                .collect(),
        })
    }
}

impl WireEncode for CanonicalOdrMemberDirectoryV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_array(encoder, &self.groups)
    }
}

fn odr_registration(value: RegistrationAbiV1) -> Option<OdrMemberAbiV1> {
    match value {
        RegistrationAbiV1::Strong => None,
        RegistrationAbiV1::Odr(value) => Some(value),
    }
}

fn physical_role(role: OdrMemberRole) -> bool {
    role != OdrMemberRole::GeneratedNominal
}

fn encode_array(
    encoder: &mut Encoder,
    values: &[impl WireEncode],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OdrMemberDirectoryProjectionError {
    DuplicateMember(OdrMemberId),
    UnexpectedMember(OdrMemberId),
    MissingMemberFingerprint(OdrMemberId),
    InvalidPhysicalRole {
        member: OdrMemberId,
        role: OdrMemberRole,
    },
}

impl fmt::Display for OdrMemberDirectoryProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid physical ODR member directory: {self:?}")
    }
}

impl std::error::Error for OdrMemberDirectoryProjectionError {}

#[cfg(test)]
mod tests;
