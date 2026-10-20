//! Shared reverse lookups that preserve the canonical member and body order.

use std::collections::BTreeMap;

use scoop_identity::{
    OdrMemberId, PersistentCallableBodyId, StrongDefinitionEntity, StrongDefinitionRole,
};

use super::super::definition_subject::member_subject;
use super::{CanonicalLirFoundation, ConeLirFoundation};

#[derive(Debug, Eq, PartialEq)]
pub(super) struct DefinitionLookup {
    bodies: BTreeMap<PersistentCallableBodyId, usize>,
    members: BTreeMap<(StrongDefinitionEntity, StrongDefinitionRole), OdrMemberId>,
}

impl DefinitionLookup {
    pub(super) fn new(foundation: &CanonicalLirFoundation) -> Self {
        let mut bodies = BTreeMap::new();
        for (index, body) in foundation.callable_bodies.iter().enumerate() {
            bodies.entry(body.id()).or_insert(index);
        }
        let mut members = BTreeMap::new();
        for member in &foundation.odr_members {
            if let Some(subject) = member_subject(member.key()) {
                members.entry(subject).or_insert(member.id());
            }
        }
        Self { bodies, members }
    }
}

impl ConeLirFoundation {
    pub(in crate::foundation) fn callable_body_position(
        &self,
        body: PersistentCallableBodyId,
    ) -> Option<usize> {
        self.definition_lookup.bodies.get(&body).copied()
    }

    pub(in crate::foundation) fn odr_member_for_subject(
        &self,
        entity: StrongDefinitionEntity,
        role: StrongDefinitionRole,
    ) -> Option<OdrMemberId> {
        self.definition_lookup.members.get(&(entity, role)).copied()
    }
}
