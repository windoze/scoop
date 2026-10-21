use super::*;
use crate::{DecodedSlibEnvelope, SlibMemberRole};

#[test]
fn source_declarations_leave_unused_machine_members_unread() {
    let directory = tempfile::tempdir().unwrap();
    let target = scoop_target_data::DARWIN_AARCH64_LLVM_22_1;
    let core_bytes = crate::layout_compile_decode::tests::layout_artifact_for(
        crate::ConeRecord::new(
            scoop_identity::ConeCoordinate::reserved_core(),
            crate::ConeKind::Library,
            crate::ConeSourceForm::Manifest,
        )
        .unwrap(),
        Vec::new(),
    );
    let core_path = directory.path().join("core.slib");
    std::fs::write(&core_path, core_bytes).unwrap();
    let core = ArtifactFile::open(&core_path, target).unwrap();
    let bytes = crate::layout_compile_decode::tests::layout_artifact_for(
        crate::strong_compile_decode::tests::cone(),
        vec![core.summary().unwrap().dependency_record()],
    );
    let path = directory.path().join("declarations.slib");
    std::fs::write(&path, &bytes).unwrap();
    let file = ArtifactFile::open(&path, target).unwrap();
    let identity = file.summary().unwrap().cone().identity();
    let current = crate::strong_compile_decode::tests::cone_named("unused-consumer").identity();
    let declarations = crate::CompileDependencyClosure::read_hir(
        current,
        target,
        vec![identity],
        vec![&core, &file],
    )
    .unwrap();
    assert_eq!(declarations.provider_count(), 2);
    let machine = declarations.read_machine_dependencies([]).unwrap();
    assert_eq!(machine.artifact_count(), 0);
    for file in [&core, &file] {
        let saved = file.directory().unwrap();
        for (record, (_, _, cached)) in saved.manifest.members().iter().zip(&saved.members) {
            assert_eq!(
                cached.get().is_some(),
                matches!(record.role(), SlibMemberRole::HirMetadata)
            );
        }
    }
    assert!(path.exists());
}

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
