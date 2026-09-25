use super::super::link_archive::{self, Rewrite};
use super::*;

mod projection;

pub(super) fn replace(
    artifact: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    failure: Failure,
) -> Vec<u8> {
    let mut changed = false;
    link_archive::rewrite(artifact, |record, data| {
        if !changed
            && matches!(failure, Failure::MissingObject)
            && matches!(record.role(), slib::SlibMemberRole::LinkObject { .. })
        {
            changed = true;
            return Rewrite::Remove;
        }
        if !matches!(failure, Failure::MissingObject)
            && matches!(record.role(), slib::SlibMemberRole::LirMetadata)
        {
            return Rewrite::Replace(link_archive::metadata(data, |section| {
                if section.capability() == &slib::lir_link_identity_closure_capability() {
                    assert!(!changed);
                    changed = true;
                    projection::mutate(section.payload(), failure)
                } else {
                    section.payload().to_vec()
                }
            }));
        }
        Rewrite::Keep
    })
}
