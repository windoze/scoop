use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{
    DefinitionAtomRole, DigestNodeId, DigestPatchIntentId, DigestSemanticFieldRole,
    ObjectDefinitionAtomId, ObjectDefinitionPlanId,
};
use scoop_lir::{ConeLirFoundation, DigestFinalizationPlanV1};

use super::{
    DigestPatchSiteValidationError, ProvisionalDigestPatchSiteV1,
    VerifiedBuiltinObjectStrongRelocationSetV1,
};
use crate::SlibMemberId;

#[derive(Clone, Copy)]
pub(super) struct ExpectedPatchSite {
    pub(super) intent: DigestPatchIntentId,
    pub(super) source: DigestNodeId,
    pub(super) semantic_field_role: DigestSemanticFieldRole,
    pub(super) member: SlibMemberId,
    pub(super) definition: ObjectDefinitionPlanId,
    pub(super) atom: ObjectDefinitionAtomId,
    pub(super) atom_role: DefinitionAtomRole,
}

pub(super) fn derive_expected_sites(
    builtins: &VerifiedBuiltinObjectStrongRelocationSetV1,
    foundation: &ConeLirFoundation,
    digest_plan: &DigestFinalizationPlanV1,
) -> Result<BTreeMap<DigestPatchIntentId, ExpectedPatchSite>, DigestPatchSiteValidationError> {
    let scoop_members = builtins
        .member_plan()
        .scoop_lir_members()
        .iter()
        .map(|member| member.member_id())
        .collect::<BTreeSet<_>>();
    let mut expected = BTreeMap::new();
    for node in digest_plan.nodes() {
        for patch in node.patch_intents() {
            let key = patch.key();
            let (definition, atom) = foundation
                .resolve_definition_atom(key.target_definition(), key.atom_role())
                .map_err(|kind| DigestPatchSiteValidationError::InvalidPatchTarget {
                    intent: patch.id(),
                    kind,
                })?;
            let member = builtins
                .member_plan()
                .member_for_definition(definition)
                .ok_or(DigestPatchSiteValidationError::MissingTargetMember {
                    intent: patch.id(),
                    definition,
                })?;
            if !scoop_members.contains(&member) {
                return Err(DigestPatchSiteValidationError::NonScoopTargetMember {
                    intent: patch.id(),
                    member,
                });
            }
            let site = ExpectedPatchSite {
                intent: patch.id(),
                source: node.id(),
                semantic_field_role: key.semantic_field_role(),
                member,
                definition,
                atom,
                atom_role: key.atom_role(),
            };
            if expected.insert(patch.id(), site).is_some() {
                return Err(DigestPatchSiteValidationError::DuplicateExpectedIntent(
                    patch.id(),
                ));
            }
        }
    }
    Ok(expected)
}

pub(super) fn validate_provisional_order_and_coverage(
    expected: &BTreeMap<DigestPatchIntentId, ExpectedPatchSite>,
    actual: &[ProvisionalDigestPatchSiteV1],
) -> Result<(), DigestPatchSiteValidationError> {
    for (index, pair) in actual.windows(2).enumerate() {
        if pair[0].intent >= pair[1].intent {
            return Err(if pair[0].intent == pair[1].intent {
                DigestPatchSiteValidationError::DuplicatePatchSite(pair[0].intent)
            } else {
                DigestPatchSiteValidationError::NonCanonicalPatchSiteOrder { index: index + 1 }
            });
        }
    }
    let actual = actual
        .iter()
        .map(|site| site.intent)
        .collect::<BTreeSet<_>>();
    let expected = expected.keys().copied().collect::<BTreeSet<_>>();
    if let Some(intent) = actual.difference(&expected).next() {
        return Err(DigestPatchSiteValidationError::UnexpectedPatchIntent(
            *intent,
        ));
    }
    if let Some(intent) = expected.difference(&actual).next() {
        return Err(DigestPatchSiteValidationError::MissingPatchIntent(*intent));
    }
    Ok(())
}
