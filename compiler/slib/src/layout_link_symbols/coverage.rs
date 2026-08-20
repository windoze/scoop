//! Bind every saved coverage projection to the same finalized object set.

use super::*;

pub(super) fn replay(
    finalized: VerifiedEntryPatchSetV2,
    ordinary: &VerifiedCrossConeStrongRequirementClosureV1,
    shape: &VerifiedExternalShapeRequirementClosureV1<'_>,
    support: &crate::LirLinkSupportSectionV1,
    input: &ReplayInputs<'_>,
) -> Result<
    (
        VerifiedCodeLinkObjectMemberSetV2,
        CanonicalKnownLinkExtensionCodeContributionSetV1,
    ),
    LayoutLinkSymbolUseError,
> {
    let finalized = verify_code_link_object_members_v2(finalized, input.manifest.members())?;
    input
        .link
        .link_identity_closure_wire()
        .replay_final_object_projections(&finalized)?;

    let ordinary = CrossConeLinkClosureSectionV1::from_verified_requirements(ordinary, &finalized)?;
    input
        .link
        .cross_cone_link_closure_wire()
        .replay_object_coverage_against(ordinary.object_coverage())?;
    let shape = CrossConeLayoutLinkClosureSectionV1::from_verified_requirements(shape, &finalized)?;
    input
        .link
        .layout_link_closure_wire()
        .replay_object_coverage_against(shape.object_coverage())?;

    let contributions =
        CanonicalKnownLinkExtensionCodeContributionSetV1::from_cross_cone_layout_semantic_imports(
            ordinary.semantic_imports(),
            shape.semantic_imports(),
        )
        .and_then(|contributions| contributions.with_link_support(support))
        .map_err(LayoutCodeFingerprintError::LinkContributionEncoding)?;
    Ok((finalized, contributions))
}
