//! Physical materialization and provisional-zero verification for digest slots.

use scoop_identity::{
    DefinitionAtomRole, DigestNodeId, DigestPatchIntentId, DigestSemanticFieldRole,
    ObjectDefinitionAtomId, ObjectDefinitionPlanId,
};
use scoop_lir::{ConeLirFoundation, StrongDigestFinalizationPlanV1};
use scoop_wire::{Encoder, WireEncode};

use super::{
    BuiltinObjectSectionRoleV1, ScoopLirObjectCandidateV1,
    VerifiedBuiltinObjectStrongRelocationSetV1,
};
use crate::SlibMemberId;

const DIGEST_SLOT_WIDTH: u8 = 32;

mod error;
pub use error::*;

mod expected;
use expected::{derive_expected_sites, validate_provisional_order_and_coverage};

mod physical;
use physical::{validate_disjoint_sites, validate_scoop_objects, verify_site};

mod final_object_normalization;
pub use final_object_normalization::*;

/// Untrusted producer materialization of one member-independent digest intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProvisionalDigestPatchSiteV1 {
    intent: DigestPatchIntentId,
    member: SlibMemberId,
    checked_offset: u64,
    width_bytes: u8,
}

impl ProvisionalDigestPatchSiteV1 {
    pub const fn new(
        intent: DigestPatchIntentId,
        member: SlibMemberId,
        checked_offset: u64,
        width_bytes: u8,
    ) -> Self {
        Self {
            intent,
            member,
            checked_offset,
            width_bytes,
        }
    }

    pub const fn intent(self) -> DigestPatchIntentId {
        self.intent
    }

    pub const fn member(self) -> SlibMemberId {
        self.member
    }

    pub const fn checked_offset(self) -> u64 {
        self.checked_offset
    }

    pub const fn width_bytes(self) -> u8 {
        self.width_bytes
    }
}

/// Canonical physical site proven to implement exactly one digest intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedMaterializedPatchSiteV1 {
    intent: DigestPatchIntentId,
    source: DigestNodeId,
    semantic_field_role: DigestSemanticFieldRole,
    member: SlibMemberId,
    definition: ObjectDefinitionPlanId,
    atom: ObjectDefinitionAtomId,
    atom_role: DefinitionAtomRole,
    section_role: BuiltinObjectSectionRoleV1,
    checked_offset: u64,
    offset_within_atom: u64,
}

impl VerifiedMaterializedPatchSiteV1 {
    pub const fn intent(self) -> DigestPatchIntentId {
        self.intent
    }

    pub const fn source(self) -> DigestNodeId {
        self.source
    }

    pub const fn semantic_field_role(self) -> DigestSemanticFieldRole {
        self.semantic_field_role
    }

    pub const fn member(self) -> SlibMemberId {
        self.member
    }

    pub const fn definition(self) -> ObjectDefinitionPlanId {
        self.definition
    }

    pub const fn atom(self) -> ObjectDefinitionAtomId {
        self.atom
    }

    pub const fn atom_role(self) -> DefinitionAtomRole {
        self.atom_role
    }

    pub const fn section_role(self) -> BuiltinObjectSectionRoleV1 {
        self.section_role
    }

    pub const fn checked_offset(self) -> u64 {
        self.checked_offset
    }

    pub const fn offset_within_atom(self) -> u64 {
        self.offset_within_atom
    }

    pub const fn width_bytes(self) -> u8 {
        DIGEST_SLOT_WIDTH
    }
}

impl WireEncode for VerifiedMaterializedPatchSiteV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.intent.encode(encoder)?;
        encoder.field(2)?;
        self.member.encode(encoder)?;
        encoder.field(3)?;
        encoder.unsigned(self.checked_offset)
    }
}

/// Proof that every digest intent has one in-atom, relocation-free, zero slot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedScoopLirDigestPatchSiteSetV1 {
    builtins: VerifiedBuiltinObjectStrongRelocationSetV1,
    digest_plan: StrongDigestFinalizationPlanV1,
    sites: Vec<VerifiedMaterializedPatchSiteV1>,
}

impl VerifiedScoopLirDigestPatchSiteSetV1 {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.builtins.producer()
    }

    pub const fn builtins(&self) -> &VerifiedBuiltinObjectStrongRelocationSetV1 {
        &self.builtins
    }

    pub const fn digest_plan(&self) -> &StrongDigestFinalizationPlanV1 {
        &self.digest_plan
    }

    pub fn sites(&self) -> &[VerifiedMaterializedPatchSiteV1] {
        &self.sites
    }
}

pub fn verify_scoop_lir_digest_patch_sites_v1(
    builtins: VerifiedBuiltinObjectStrongRelocationSetV1,
    foundation: &ConeLirFoundation,
    digest_plan: StrongDigestFinalizationPlanV1,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
    provisional_sites: &[ProvisionalDigestPatchSiteV1],
) -> Result<VerifiedScoopLirDigestPatchSiteSetV1, DigestPatchSiteValidationError> {
    if builtins.producer() != foundation.producer() {
        return Err(DigestPatchSiteValidationError::ProducerMismatch {
            object: builtins.producer(),
            foundation: foundation.producer(),
        });
    }
    digest_plan
        .validate_against(foundation)
        .map_err(DigestPatchSiteValidationError::DigestPlan)?;
    let objects = validate_scoop_objects(&builtins, scoop_objects)?;
    let expected = derive_expected_sites(&builtins, foundation, &digest_plan)?;
    validate_provisional_order_and_coverage(&expected, provisional_sites)?;

    let mut sites = Vec::with_capacity(provisional_sites.len());
    for (expected, provisional) in expected.values().zip(provisional_sites) {
        sites.push(verify_site(&builtins, &objects, expected, *provisional)?);
    }
    validate_disjoint_sites(&sites)?;

    Ok(VerifiedScoopLirDigestPatchSiteSetV1 {
        builtins,
        digest_plan,
        sites,
    })
}

#[cfg(test)]
mod tests;
