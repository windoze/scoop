use super::*;
use crate::{DecodedSlibEnvelope, SlibMemberRole};

#[test]
fn truncated_file_keeps_complete_members_readable_and_reports_the_missing_payload() {
    let bytes = crate::link_decode::complete_strong_artifact_for_test();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("truncated.slib");
    std::fs::write(&path, &bytes[..bytes.len() - 1]).unwrap();
    let file = ArtifactFile::open(&path, scoop_target_data::DARWIN_AARCH64_LLVM_22_1).unwrap();
    let saved = file.directory().unwrap();
    let first = saved.manifest.members().first().unwrap().id();
    let last = saved.manifest.members().last().unwrap().id();
    assert_ne!(first, last);
    assert!(file.member(saved, first).unwrap().is_some());
    assert!(matches!(
        file.member(saved, last),
        Err(SlibReadError::Io(error)) if error.kind() == io::ErrorKind::UnexpectedEof
    ));
}

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
    let (providers, identities) = declarations.into_semantic_inputs();
    assert_eq!(identities.origin_count(), 2);
    let world = scoop_hir::ImportedSemanticWorld::from_dependencies(
        current,
        vec![providers[1].input()],
        vec![providers[0].input()],
    )
    .unwrap();
    assert!(world.direct_provider(identity).is_some());
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
                let position = file.0.file.lock().unwrap().stream_position().unwrap();
                assert!(std::ptr::eq(
                    payload,
                    DecodedSlibEnvelope::from_file(&shared)
                        .unwrap()
                        .member(record.id())
                        .unwrap()
                        .unwrap()
                ));
                assert_eq!(
                    file.0.file.lock().unwrap().stream_position().unwrap(),
                    position
                );
                metadata += 1;
            }
            SlibMemberRole::LinkObject | SlibMemberRole::NativeObject { .. } => {
                assert!(cached.get().is_none());
                let payload = envelope.member(record.id()).unwrap().unwrap();
                assert_eq!(payload, memory.member(record.id()).unwrap().unwrap());
                assert!(std::ptr::eq(
                    payload,
                    envelope.member(record.id()).unwrap().unwrap()
                ));
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
        metadata + objects
    );
}

#[test]
fn directory_input_reports_the_actual_read_error() {
    let directory = tempfile::tempdir().unwrap();
    let opened = ArtifactFile::open(
        directory.path(),
        scoop_target_data::DARWIN_AARCH64_LLVM_22_1,
    );
    // Windows rejects a directory at open; Unix rejects the later read.
    if cfg!(windows) {
        assert_eq!(opened.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
    } else {
        assert!(
            matches!(opened.unwrap().summary(), Err(SlibReadError::Io(error))
            if error.kind() == io::ErrorKind::IsADirectory)
        );
    }
}

#[test]
fn analysis_threads_share_directory_and_member_buffers() {
    let bytes = crate::link_decode::complete_strong_artifact_for_test();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("shared.slib");
    std::fs::write(&path, bytes).unwrap();
    let file = ArtifactFile::open(&path, scoop_target_data::DARWIN_AARCH64_LLVM_22_1).unwrap();
    std::thread::scope(|scope| {
        let jobs = (0..4)
            .map(|_| {
                scope.spawn(|| {
                    let saved = file.directory().unwrap();
                    let record = saved
                        .manifest
                        .members()
                        .iter()
                        .find(|record| *record.role() == SlibMemberRole::HirMetadata)
                        .unwrap();
                    file.member(saved, record.id()).unwrap().unwrap()
                })
            })
            .collect::<Vec<_>>();
        let buffers = jobs
            .into_iter()
            .map(|job| job.join().unwrap())
            .collect::<Vec<_>>();
        assert!(
            buffers
                .windows(2)
                .all(|pair| std::ptr::eq(pair[0], pair[1]))
        );
    });
}
