//! Shared callable ABIs bound to their existing physical body identities.

use std::fmt;

use scoop_identity::{
    CallableBodyKeyKind, OdrGroupId, OdrMemberId, OdrMemberRole, PersistentCallableBodyId,
    RuntimeIdentityRecord,
};
use scoop_wire::{Digest256, HashError, domain_separated_cbor_hash};

use crate::{ConeLirFoundation, Module};

mod encode;
mod wire;

#[cfg(test)]
pub(crate) mod tests;
pub use wire::DecodedCanonicalCallableAbisV1;

/// The shared ABI and typed ownership of an emitted callable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalCallableAbiV1 {
    body: PersistentCallableBodyId,
    owner: CanonicalCallableAbiOwnerV1,
}

/// ODR members carry their actual member role and complete ABI content.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalCallableAbiOwnerV1 {
    Strong,
    Odr {
        group: OdrGroupId,
        member: OdrMemberId,
        role: OdrMemberRole,
        abi: Digest256,
    },
}

impl CanonicalCallableAbiV1 {
    pub const fn new(body: PersistentCallableBodyId, owner: CanonicalCallableAbiOwnerV1) -> Self {
        Self { body, owner }
    }

    pub const fn body(self) -> PersistentCallableBodyId {
        self.body
    }

    pub const fn owner(self) -> CanonicalCallableAbiOwnerV1 {
        self.owner
    }
}

/// The complete callable ABI table, including lowered root and
/// initialization gateways as well as ordinary Strong and ODR bodies.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalCallableAbisV1 {
    definitions: Vec<CanonicalCallableAbiV1>,
}

impl CanonicalCallableAbisV1 {
    pub fn from_module(
        module: &Module,
        foundation: &ConeLirFoundation,
    ) -> Result<Self, CanonicalCallableAbiError> {
        let definitions = module
            .callable_bodies()
            .map(|function| {
                let owner =
                    match body_odr_member(function.callable_body.identity_record(), foundation) {
                        None => CanonicalCallableAbiOwnerV1::Strong,
                        Some((group, member, role)) => {
                            let abi = domain_separated_cbor_hash(
                                "scoop-odr-member-abi-v1",
                                &encode::CallableAbiProjection {
                                    module,
                                    function,
                                    group,
                                    member,
                                    role,
                                },
                            )
                            .map_err(CanonicalCallableAbiError::Hash)?;
                            CanonicalCallableAbiOwnerV1::Odr {
                                group,
                                member,
                                role,
                                abi,
                            }
                        }
                    };
                Ok(CanonicalCallableAbiV1::new(
                    function.callable_body.id(),
                    owner,
                ))
            })
            .collect::<Result<Vec<_>, CanonicalCallableAbiError>>()?;
        Self::new(definitions, foundation)
    }

    pub fn new(
        mut definitions: Vec<CanonicalCallableAbiV1>,
        foundation: &ConeLirFoundation,
    ) -> Result<Self, CanonicalCallableAbiError> {
        definitions.sort_by_key(|definition| definition.body);
        let mut records = foundation.callable_bodies().iter().collect::<Vec<_>>();
        // Foundation records are topological: a root gateway follows main
        // even when its body ID sorts first. ABI records are ID-ordered.
        records.sort_unstable_by_key(|record| record.id());
        let expected = records.iter().map(|record| record.id()).collect::<Vec<_>>();
        let actual = definitions
            .iter()
            .map(|definition| definition.body)
            .collect::<Vec<_>>();
        if actual != expected {
            return Err(CanonicalCallableAbiError::BodySet { expected, actual });
        }
        for (record, definition) in records.into_iter().zip(&definitions) {
            let matches = match (body_odr_member(record, foundation), definition.owner) {
                (None, CanonicalCallableAbiOwnerV1::Strong) => true,
                (
                    Some(expected),
                    CanonicalCallableAbiOwnerV1::Odr {
                        group,
                        member,
                        role,
                        ..
                    },
                ) => expected == (group, member, role),
                _ => false,
            };
            if !matches {
                return Err(CanonicalCallableAbiError::DefinitionOwner { body: record.id() });
            }
        }
        Ok(Self { definitions })
    }

    pub fn definitions(&self) -> &[CanonicalCallableAbiV1] {
        &self.definitions
    }

    pub fn get(&self, body: PersistentCallableBodyId) -> Option<&CanonicalCallableAbiV1> {
        self.definitions
            .binary_search_by_key(&body, |definition| definition.body)
            .ok()
            .map(|index| &self.definitions[index])
    }
}

fn body_odr_member(
    record: &RuntimeIdentityRecord<PersistentCallableBodyId>,
    foundation: &ConeLirFoundation,
) -> Option<(OdrGroupId, OdrMemberId, OdrMemberRole)> {
    match record.key().kind() {
        CallableBodyKeyKind::Odr(member) => Some((member.group(), member.member(), member.role())),
        CallableBodyKeyKind::ReleaseHook { .. } => {
            let definition = foundation.definition_for(
                scoop_identity::StrongDefinitionEntity::callable_body(record.id()),
                scoop_identity::StrongDefinitionRole::CallableBody,
            )?;
            let scoop_identity::ObjectDefinitionPlanOwner::Odr { member } =
                definition.key().owner()
            else {
                return None;
            };
            let member = foundation.odr_member(member)?;
            Some((member.key().group(), member.id(), member.key().role()))
        }
        CallableBodyKeyKind::Strong(_)
        | CallableBodyKeyKind::RootGateway { .. }
        | CallableBodyKeyKind::InitializationStartupGateway(_) => None,
    }
}

#[cfg(test)]
fn function_bodies(
    foundation: &ConeLirFoundation,
) -> impl Iterator<Item = PersistentCallableBodyId> + '_ {
    foundation
        .callable_bodies()
        .iter()
        .map(|record| record.id())
}

#[derive(Debug)]
pub enum CanonicalCallableAbiError {
    Hash(HashError),
    BodySet {
        expected: Vec<PersistentCallableBodyId>,
        actual: Vec<PersistentCallableBodyId>,
    },
    UnknownBody([u8; 32]),
    UnsortedBodies,
    DefinitionOwner {
        body: PersistentCallableBodyId,
    },
}

impl fmt::Display for CanonicalCallableAbiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid canonical callable ABIs: {self:?}")
    }
}
impl std::error::Error for CanonicalCallableAbiError {}
