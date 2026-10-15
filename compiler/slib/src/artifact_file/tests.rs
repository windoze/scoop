use super::*;
use crate::{DecodedSlibEnvelope, SlibMemberRole};

#[test]
fn complete_file_reads_only_requested_members_and_reuses_their_buffers() {
    let bytes = crate::link_decode::complete_strong_artifact_for_test();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("complete.slib");
    std::fs::write(&path, &bytes).unwrap();
    let target = scoop_target_data::DARWIN_AARCH64_LLVM_22_1;
    let file = ArtifactFile::open(&path, target).unwrap();
    let shared = file.clone();
    let summary = file.summary().unwrap();
    assert!(std::ptr::eq(summary, shared.summary().unwrap()));
    assert!(std::ptr::eq(summary, file.summary().unwrap()));
    let saved = file.directory().unwrap();
    assert!(
        saved
            .members
            .iter()
            .all(|(_, _, cached)| cached.get().is_none())
    );
    let memory = DecodedSlibEnvelope::open(&bytes, target).unwrap();
    let envelope = DecodedSlibEnvelope::from_file(&file).unwrap();
    let mut metadata = 0;
    let mut objects = 0;
    for (record, (_, _, cached)) in saved.manifest.members().iter().zip(&saved.members) {
        match record.role() {
            SlibMemberRole::HirMetadata
            | SlibMemberRole::MirMetadata
            | SlibMemberRole::LirMetadata => {
                let payload = envelope.member(record.id()).unwrap().unwrap();
                assert_eq!(payload, memory.member(record.id()).unwrap().unwrap());
                let position = file.0.file.borrow_mut().stream_position().unwrap();
                assert!(std::ptr::eq(
                    payload,
                    DecodedSlibEnvelope::from_file(&shared)
                        .unwrap()
                        .member(record.id())
                        .unwrap()
                        .unwrap()
                ));
                assert_eq!(
                    file.0.file.borrow_mut().stream_position().unwrap(),
                    position
                );
                metadata += 1;
            }
            SlibMemberRole::LinkObject { .. } => {
                assert!(cached.get().is_none());
                objects += 1;
            }
            _ => {}
        }
    }
    assert_eq!(metadata, 3);
    assert!(objects > 0);
    assert_eq!(
        saved
            .members
            .iter()
            .filter(|(_, _, cached)| cached.get().is_some())
            .count(),
        3
    );
}

#[test]
fn directory_input_reports_the_actual_read_error() {
    let directory = tempfile::tempdir().unwrap();
    let file = ArtifactFile::open(
        directory.path(),
        scoop_target_data::DARWIN_AARCH64_LLVM_22_1,
    )
    .unwrap();
    assert!(matches!(file.summary(), Err(SlibReadError::Io(error))
        if error.kind() == io::ErrorKind::IsADirectory));
}
