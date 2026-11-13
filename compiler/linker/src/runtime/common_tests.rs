use super::*;

pub(super) fn check_archive_and_inputs(
    runtime: &RuntimeObjectSet,
    paths: &[PathBuf],
    toolchain: &CBridgeToolchainProfileV1,
) {
    let bytes = std::fs::read(runtime.archive_path()).unwrap();
    let archive = object::read::archive::ArchiveFile::parse(bytes.as_slice()).unwrap();
    let members: Vec<_> = archive.members().map(Result::unwrap).collect();
    assert_eq!(members.len(), runtime.objects().len());
    for (index, (member, object)) in members.iter().zip(runtime.objects()).enumerate() {
        assert_eq!(member.name(), format!("{index}.o").as_bytes());
        assert_eq!(
            sha256(member.data(bytes.as_slice()).unwrap()),
            object.digest()
        );
    }
    assert_eq!(runtime.input_paths().len(), 2);
    assert_eq!(
        std::fs::read_dir(runtime.index_path().parent().unwrap())
            .unwrap()
            .count(),
        2
    );

    let directory = tempfile::tempdir().unwrap();
    let reordered = RuntimeObjectSet::from_paths(
        runtime.target(),
        toolchain,
        runtime.configuration().clone(),
        paths.iter().rev().cloned().collect(),
        &directory.path().join("reordered"),
        None,
    )
    .unwrap();
    assert_eq!(runtime.fingerprint(), reordered.fingerprint());
    assert_eq!(bytes, std::fs::read(reordered.archive_path()).unwrap());

    let startup = runtime
        .target()
        .contract()
        .native_symbol_normalization()
        .compiler_generated_object_symbol("scoop_rt_run_program");
    let missing_entry = paths
        .iter()
        .filter(|path| {
            !NativeObjectInfo::read_with_toolchain(&std::fs::read(path).unwrap(), toolchain)
                .unwrap()
                .definitions
                .contains_key(&startup)
        })
        .cloned()
        .collect();
    assert!(
        RuntimeObjectSet::from_paths(
            runtime.target(),
            toolchain,
            runtime.configuration().clone(),
            missing_entry,
            &directory.path().join("missing-entry"),
            None,
        )
        .unwrap_err()
        .to_string()
        .contains("startup entry")
    );
    let mut duplicate = paths.to_vec();
    duplicate.push(duplicate[0].clone());
    assert!(
        RuntimeObjectSet::from_paths(
            runtime.target(),
            toolchain,
            runtime.configuration().clone(),
            duplicate,
            &directory.path().join("duplicate"),
            None,
        )
        .unwrap_err()
        .to_string()
        .contains("duplicate object IDs")
    );
}
