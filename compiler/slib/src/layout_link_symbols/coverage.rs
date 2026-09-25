//! Bind every saved coverage projection to the same finalized object set.

use super::*;

pub(super) fn replay(
    finalized: VerifiedEntryPatchSetV2,
    ordinary: &VerifiedCrossConeStrongRequirementClosureV1,
    shape: &VerifiedExternalShapeRequirementClosureV1<'_>,
    input: &ReplayInputs<'_, '_>,
    meter: &mut BudgetMeter,
) -> Result<VerifiedCodeLinkObjectMemberSetV2, LayoutLinkSymbolUseError> {
    resources::final_directory(input.manifest.members(), meter)?;
    let finalized = verify_code_link_object_members_v2(finalized, input.manifest.members())?;
    input
        .link
        .link_identity_closure_wire()
        .replay_final_object_projections(&finalized, meter)?;
    resources::ordinary_coverage(ordinary, finalized.projection(), meter)?;
    let ordinary = CrossConeLinkClosureSectionV1::from_verified_requirements(ordinary, &finalized)?;
    input
        .link
        .cross_cone_link_closure_wire()
        .replay_object_coverage_against(ordinary.object_coverage(), meter)?;
    let shape =
        CrossConeLayoutLinkClosureSectionV1::from_verified_requirements(shape, &finalized, meter)?;
    input
        .link
        .layout_link_closure_wire()
        .replay_object_coverage_against(shape.object_coverage(), meter)?;
    Ok(finalized)
}
