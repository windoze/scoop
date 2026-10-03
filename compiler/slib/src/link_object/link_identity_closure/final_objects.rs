//! The producer and reader derive owners from the same final object proof.

use super::*;

pub(super) fn owners<D, C, I>(
    objects: &VerifiedCodeLinkObjectMemberSetV1<D, C, I>,
) -> Result<
    (VerifiedImageOwnerProjectionV1, VerifiedEntryOwnerBranchV1),
    LinkIdentityClosureBuildError,
>
where
    D: scoop_lir::StrongDescriptorReference,
    C: Clone,
{
    let final_objects = objects.final_objects();
    let image = final_objects.runtime_images().fingerprint().image();
    let primary = image.primary();
    let image_owner = VerifiedImageOwnerProjectionV1 {
        member: image.member(),
        definition: image.plan().definition_plan(),
        primary_atom: primary.atom(),
        primary_symbol_table_index: image.primary_symbol_table_index(),
        checked_offset: primary.checked_offset(),
        byte_size: primary.byte_size(),
    };
    let entry_owner = match (final_objects.entry().branch(), final_objects.entry().plan()) {
        (VerifiedEntryProductionBranchV1::Library, scoop_lir::EntryProductionPlanV1::Library) => {
            VerifiedEntryOwnerBranchV1::Library
        }
        (
            VerifiedEntryProductionBranchV1::Executable(entry),
            scoop_lir::EntryProductionPlanV1::Executable(plan),
        ) => VerifiedEntryOwnerBranchV1::Executable(VerifiedEntryOwnerProjectionV1 {
            member: entry.member(),
            definition: plan.root_descriptor_definition(),
            checked_offset: entry.checked_offset(),
        }),
        _ => return Err(LinkIdentityClosureBuildError::EntryBranchMismatch),
    };
    Ok((image_owner, entry_owner))
}
