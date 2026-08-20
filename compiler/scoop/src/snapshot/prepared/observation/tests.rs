use super::*;
use scoop_protocol::{HostPathCarrier, ProtocolDumpContentDigest, StageDumpKindV1};

fn dump(directory: &Path, stages: StageDumpSet) -> PreparedDump {
    PreparedDump {
        stages,
        private_directory: directory.to_owned(),
        destination: directory.join("published"),
    }
}

fn descriptor(directory: &Path, stage: StageDumpKindV1, bytes: &[u8]) -> EmittedDumpDescriptorV1 {
    let path = directory.join(stage.file_name());
    std::fs::write(&path, bytes).unwrap();
    EmittedDumpDescriptorV1::new(
        stage,
        EmittedDumpDestinationV1::File(HostPathCarrier::from_path(&path).unwrap()),
        ProtocolDumpContentDigest::from_array(*scoop_wire::sha256(bytes).as_array()),
    )
}

#[test]
fn captures_the_exact_ordered_set_and_keeps_observed_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let dump = dump(directory.path(), StageDumpSet::all());
    let descriptors = dump
        .stages
        .iter()
        .map(|stage| descriptor(directory.path(), stage, stage.file_name().as_bytes()))
        .collect::<Vec<_>>();
    let snapshots = capture_dumps(&dump, &descriptors).unwrap();
    for (stage, snapshot) in dump.stages.iter().zip(snapshots) {
        std::fs::write(directory.path().join(stage.file_name()), b"later bytes").unwrap();
        assert_eq!(snapshot.as_bytes(), stage.file_name().as_bytes());
    }
}

#[test]
fn rejects_incomplete_reordered_and_unrequested_files() {
    let directory = tempfile::tempdir().unwrap();
    let dump = dump(directory.path(), StageDumpSet::all());
    let mut descriptors = dump
        .stages
        .iter()
        .map(|stage| descriptor(directory.path(), stage, b"stage"))
        .collect::<Vec<_>>();
    assert!(capture_dumps(&dump, &descriptors[..3]).is_err());
    descriptors.swap(0, 1);
    assert!(capture_dumps(&dump, &descriptors).is_err());
    descriptors.swap(0, 1);
    std::fs::write(directory.path().join("unexpected.txt"), b"extra").unwrap();
    assert!(capture_dumps(&dump, &descriptors).is_err());
}

#[test]
fn rejects_wrong_paths_digests_and_symlinks() {
    let directory = tempfile::tempdir().unwrap();
    let stage = StageDumpKindV1::Hir;
    let dump = dump(directory.path(), StageDumpSet::one(stage));
    let actual = descriptor(directory.path(), stage, b"hir");
    let wrong = EmittedDumpDescriptorV1::new(
        stage,
        EmittedDumpDestinationV1::File(
            HostPathCarrier::from_path(Path::new("elsewhere/hir.txt")).unwrap(),
        ),
        actual.content_digest(),
    );
    assert!(capture_dumps(&dump, &[wrong]).is_err());
    std::fs::write(directory.path().join(stage.file_name()), b"wrong").unwrap();
    assert!(capture_dumps(&dump, std::slice::from_ref(&actual)).is_err());
    #[cfg(unix)]
    {
        std::fs::remove_file(directory.path().join(stage.file_name())).unwrap();
        std::os::unix::fs::symlink("/dev/null", directory.path().join(stage.file_name())).unwrap();
        assert!(capture_dumps(&dump, &[actual]).is_err());
    }
}
