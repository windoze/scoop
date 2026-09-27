//! Match every decoded digest input to the same typed member and digest plans.

use super::resources::allocate;
use super::*;
use scoop_wire::WirePath;

impl DecodedLinkIdentityClosureSectionV1 {
    pub fn replay_digest_patch_inputs(
        &self,
        member_plan: &PlannedLinkObjectMemberSetV1,
        digest_plan: &DigestFinalizationPlanV1,
    ) -> Result<Vec<ProvisionalDigestPatchSiteV1>, LinkDigestPatchInputValidationError> {
        let path = WirePath::root().field(3);

        let scoop_members = member_plan
            .scoop_lir_members()
            .iter()
            .map(|member| member.member_id())
            .collect::<BTreeSet<_>>();
        let mut expected = BTreeMap::new();

        for node in digest_plan.nodes() {
            for patch in node.patch_intents() {
                let intent = patch.id();
                let definition = patch.key().target_definition();

                let member = member_plan.member_for_definition(definition).ok_or(
                    LinkDigestPatchInputValidationError::MissingTargetMember { intent, definition },
                )?;
                if !scoop_members.contains(&member) {
                    return Err(LinkDigestPatchInputValidationError::NonScoopTargetMember {
                        intent,
                        member,
                    });
                }
                if expected
                    .insert(*intent.as_array(), (intent, member))
                    .is_some()
                {
                    return Err(
                        LinkDigestPatchInputValidationError::DuplicateExpectedPatchIntent(intent),
                    );
                }
            }
        }

        let mut sites = allocate(self.patch_sites.len(), &path)?;

        let mut seen = BTreeSet::new();
        let mut previous = None;
        for (index, decoded) in self.patch_sites.iter().enumerate() {
            let (intent, member) = expected.get(decoded.intent.as_array()).copied().ok_or(
                LinkDigestPatchInputValidationError::UnknownPatchIntent(*decoded.intent.as_array()),
            )?;
            if let Some(previous) = previous {
                if previous >= intent {
                    return Err(if previous == intent {
                        LinkDigestPatchInputValidationError::DuplicatePatchIntent(intent)
                    } else {
                        LinkDigestPatchInputValidationError::NonCanonicalPatchSiteOrder { index }
                    });
                }
            }
            if !decoded.member.matches(member.as_array()) {
                return Err(LinkDigestPatchInputValidationError::PatchMemberMismatch {
                    intent,
                    expected: member,
                });
            }
            previous = Some(intent);
            seen.insert(intent);
            sites.push(ProvisionalDigestPatchSiteV1::new(
                intent,
                member,
                decoded.checked_offset,
                32,
            ));
        }

        if let Some((intent, _)) = expected.values().find(|(intent, _)| !seen.contains(intent)) {
            return Err(LinkDigestPatchInputValidationError::MissingPatchIntent(
                *intent,
            ));
        }

        Ok(sites)
    }
}

impl From<WireError> for LinkDigestPatchInputValidationError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
