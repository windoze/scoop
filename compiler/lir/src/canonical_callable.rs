//! Canonical content leaves for the final LIR function bodies.

use std::fmt;

use scoop_identity::{
    CallableBodyKeyKind, OdrGroupId, OdrMemberId, OdrMemberRole, PersistentCallableBodyId,
    RuntimeIdentityRecord,
};
use scoop_wire::{Digest256, HashError, domain_separated_cbor_hash};

use crate::{ConeLirFoundation, Function, Module};

mod encode;
mod wire;

#[cfg(test)]
pub(crate) mod tests;
pub use wire::DecodedCanonicalCallableLirDefinitionsV1;

/// A content hash, distinct from every persistent entity identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalCallableLirDefinitionV1 {
    body: PersistentCallableBodyId,
    fingerprint: Digest256,
    owner: CanonicalCallableDefinitionOwnerV1,
}

/// ODR members carry their actual member role and complete ABI content.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalCallableDefinitionOwnerV1 {
    Strong,
    Odr {
        group: OdrGroupId,
        member: OdrMemberId,
        role: OdrMemberRole,
        abi: Digest256,
    },
}

impl CanonicalCallableLirDefinitionV1 {
    pub const fn new(
        body: PersistentCallableBodyId,
        fingerprint: Digest256,
        owner: CanonicalCallableDefinitionOwnerV1,
    ) -> Self {
        Self {
            body,
            fingerprint,
            owner,
        }
    }

    pub const fn body(self) -> PersistentCallableBodyId {
        self.body
    }

    pub const fn fingerprint(self) -> Digest256 {
        self.fingerprint
    }

    pub const fn owner(self) -> CanonicalCallableDefinitionOwnerV1 {
        self.owner
    }
}

/// The exact set of LIR Function leaves, including lowered root and
/// initialization gateways as well as ordinary Strong and ODR bodies.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalCallableLirDefinitionsV1 {
    definitions: Vec<CanonicalCallableLirDefinitionV1>,
}

impl CanonicalCallableLirDefinitionsV1 {
    pub fn from_module(
        module: &Module,
        foundation: &ConeLirFoundation,
    ) -> Result<Self, CanonicalCallableLirError> {
        let definitions = module
            .functions
            .iter()
            .map(|function| {
                let owner = match body_odr_member(function.callable_body.identity_record()) {
                    None => CanonicalCallableDefinitionOwnerV1::Strong,
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
                        .map_err(CanonicalCallableLirError::Hash)?;
                        CanonicalCallableDefinitionOwnerV1::Odr {
                            group,
                            member,
                            role,
                            abi,
                        }
                    }
                };
                Ok(CanonicalCallableLirDefinitionV1::new(
                    function.callable_body.id(),
                    canonical_callable_lir_fingerprint(module, function)
                        .map_err(CanonicalCallableLirError::Hash)?,
                    owner,
                ))
            })
            .collect::<Result<Vec<_>, CanonicalCallableLirError>>()?;
        Self::new(definitions, foundation)
    }

    pub fn new(
        mut definitions: Vec<CanonicalCallableLirDefinitionV1>,
        foundation: &ConeLirFoundation,
    ) -> Result<Self, CanonicalCallableLirError> {
        definitions.sort_by_key(|definition| definition.body);
        let mut records = foundation.callable_bodies().iter().collect::<Vec<_>>();
        // Foundation records are topological: a root gateway follows main
        // even when its body ID sorts first. Content leaves are ID-ordered.
        records.sort_unstable_by_key(|record| record.id());
        let expected = records.iter().map(|record| record.id()).collect::<Vec<_>>();
        let actual = definitions
            .iter()
            .map(|definition| definition.body)
            .collect::<Vec<_>>();
        if actual != expected {
            return Err(CanonicalCallableLirError::BodySet { expected, actual });
        }
        for (record, definition) in records.into_iter().zip(&definitions) {
            let matches = match (body_odr_member(record), definition.owner) {
                (None, CanonicalCallableDefinitionOwnerV1::Strong) => true,
                (
                    Some(expected),
                    CanonicalCallableDefinitionOwnerV1::Odr {
                        group,
                        member,
                        role,
                        ..
                    },
                ) => expected == (group, member, role),
                _ => false,
            };
            if !matches {
                return Err(CanonicalCallableLirError::DefinitionOwner { body: record.id() });
            }
        }
        Ok(Self { definitions })
    }

    pub fn definitions(&self) -> &[CanonicalCallableLirDefinitionV1] {
        &self.definitions
    }

    pub fn get(&self, body: PersistentCallableBodyId) -> Option<&CanonicalCallableLirDefinitionV1> {
        self.definitions
            .binary_search_by_key(&body, |definition| definition.body)
            .ok()
            .map(|index| &self.definitions[index])
    }
}

fn body_odr_member(
    record: &RuntimeIdentityRecord<PersistentCallableBodyId>,
) -> Option<(OdrGroupId, OdrMemberId, OdrMemberRole)> {
    match record.key().kind() {
        CallableBodyKeyKind::Odr(member) => Some((member.group(), member.member(), member.role())),
        CallableBodyKeyKind::Strong(_)
        | CallableBodyKeyKind::RootGateway { .. }
        | CallableBodyKeyKind::InitializationStartupGateway(_)
        | CallableBodyKeyKind::ReleaseHook { .. } => None,
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

/// Computes the content of one actual function without referring to its
/// producer, diagnostic names, local arena numbering or object placement.
pub fn canonical_callable_lir_fingerprint(
    module: &Module,
    function: &Function,
) -> Result<Digest256, HashError> {
    domain_separated_cbor_hash(
        "scoop-lir-definition-v1",
        &encode::CallableProjection::new(module, function)?,
    )
}

#[derive(Debug)]
pub enum CanonicalCallableLirError {
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

impl fmt::Display for CanonicalCallableLirError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid canonical LIR callable definitions: {self:?}")
    }
}
impl std::error::Error for CanonicalCallableLirError {}
