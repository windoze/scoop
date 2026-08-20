use super::*;
use crate::{MemberStableKey, NativeLinkObject, SlibMemberRole, ValidatedGraphArtifact};

pub(super) fn read(
    graph: &mut ValidatedGraphArtifact<'_>,
    target: lir::LirTargetProfile,
) -> Result<Vec<NativeLinkObject>, ProgramLinkReadError> {
    let records: Vec<_> = graph
        .envelope
        .manifest()
        .members()
        .iter()
        .filter(|record| crate::is_native_link_object(record))
        .cloned()
        .collect();
    records.into_iter().map(|record| {
        if !matches!(record.role(), SlibMemberRole::LinkObject { target_profile, object_format, .. }
            if *target_profile == target.wire_id() && *object_format == target.id().object_format()) {
            return Err(error(format!("native object {} has a different target", record.id())));
        }
        let MemberStableKey::LinkObject { logical_key, .. } = record.stable_key() else {
            return Err(error("native object has an invalid member key"));
        };
        let source = std::str::from_utf8(logical_key.as_bytes()).map_err(error)?;
        let source = scoop_identity::NormalizedSourcePath::new(source).map_err(error)?;
        let bytes = graph.envelope.member(record.id())
            .ok_or_else(|| error(format!("missing native object {}", record.id())))?;
        Ok(NativeLinkObject { member: record.id(), source, bytes: bytes.into() })
    }).collect()
}
