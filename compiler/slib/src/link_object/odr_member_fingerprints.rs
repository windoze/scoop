//! Shared member ABIs, independent of the producing implementation.

use scoop_identity::{OdrGroupId, OdrMemberId, OdrMemberRole};
use scoop_lir::{RegistrationDefinitionOwner, RegistrationTableV1};
use scoop_wire::{Digest256, HashError, domain_separated_cbor_hash};

use super::OdrAbiFingerprintV1;

mod encode;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistrationAbiV1 {
    Strong,
    Odr(OdrMemberAbiV1),
}

impl RegistrationAbiV1 {
    pub(super) fn from_owner(
        owner: RegistrationDefinitionOwner,
        table: RegistrationTableV1,
    ) -> Result<Self, HashError> {
        match owner {
            RegistrationDefinitionOwner::Strong => Ok(Self::Strong),
            RegistrationDefinitionOwner::Odr { group, member } => {
                let abi = domain_separated_cbor_hash(
                    "scoop-odr-member-abi-v1",
                    &encode::RegistrationAbi {
                        group,
                        member,
                        table,
                    },
                )?;
                Ok(Self::Odr(OdrMemberAbiV1::new(
                    group,
                    member,
                    OdrMemberRole::RegistrationRecord,
                    abi,
                )))
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableDefinitionAbiV1 {
    Strong,
    Odr(OdrMemberAbiV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OdrMemberAbiV1 {
    group: OdrGroupId,
    member: OdrMemberId,
    role: OdrMemberRole,
    abi: OdrAbiFingerprintV1,
}

impl OdrMemberAbiV1 {
    fn new(group: OdrGroupId, member: OdrMemberId, role: OdrMemberRole, abi: Digest256) -> Self {
        Self {
            group,
            member,
            role,
            abi: OdrAbiFingerprintV1::from_array(*abi.as_array()),
        }
    }

    pub const fn group(self) -> OdrGroupId {
        self.group
    }
    pub const fn member(self) -> OdrMemberId {
        self.member
    }
    pub const fn role(self) -> OdrMemberRole {
        self.role
    }
    pub const fn abi(self) -> OdrAbiFingerprintV1 {
        self.abi
    }
}

pub(super) fn callable_body(
    canonical: scoop_lir::CanonicalCallableAbiV1,
) -> CallableDefinitionAbiV1 {
    match canonical.owner() {
        scoop_lir::CanonicalCallableAbiOwnerV1::Strong => CallableDefinitionAbiV1::Strong,
        scoop_lir::CanonicalCallableAbiOwnerV1::Odr {
            group,
            member,
            role,
            abi,
        } => CallableDefinitionAbiV1::Odr(OdrMemberAbiV1::new(group, member, role, abi)),
    }
}

pub(super) fn shape_definition(canonical: scoop_lir::CanonicalShapeAbiV1) -> OdrMemberAbiV1 {
    OdrMemberAbiV1::new(
        canonical.group(),
        canonical.member(),
        canonical.role(),
        canonical.abi(),
    )
}
