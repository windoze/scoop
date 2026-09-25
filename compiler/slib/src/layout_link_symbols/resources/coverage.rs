//! Prepay the shared directory verifier and ordinary coverage projection.

use super::*;

pub(in super::super) fn final_directory(
    directory: &[crate::SlibMemberRecord],
    meter: &mut BudgetMeter,
) -> Result<(), WireError> {
    // The verifier indexes actual/expected members and their typed identities.
    // Neither the finalized object bytes nor the archive are copied here.
    let count = directory.len() as u64;
    table::<crate::SlibMemberId>(count.saturating_mul(8), meter)?;
    table::<VerifiedCodeLinkObjectMemberV1>(count.saturating_mul(2), meter)?;
    for record in directory {
        if matches!(record.role(), crate::SlibMemberRole::LinkObject { .. }) {
            copy_plan(record.stable_key(), meter)?;
            copy_plan(record.role(), meter)?;
        }
    }
    Ok(())
}

pub(in super::super) fn ordinary_coverage(
    ordinary: &VerifiedCrossConeStrongRequirementClosureV1,
    objects: &CodeLinkObjectMemberSetV1,
    meter: &mut BudgetMeter,
) -> Result<(), WireError> {
    copy_plan(ordinary.semantic_imports(), meter)?;
    copy_plan(objects, meter)?;
    let count = ordinary.requirements().len() as u64;
    slots::<CrossConeUndefinedRequirementV1>(count, meter)?;
    meter.charge_work(
        count.saturating_mul(log(objects.members().len() as u64)),
        &WirePath::root(),
    )?;
    for requirement in ordinary.requirements() {
        // The common builder copies each complete use and hashes it alongside
        // the final directory; strings retain their actual encoded lengths.
        copy_plan(requirement, meter)?;
    }
    Ok(())
}
