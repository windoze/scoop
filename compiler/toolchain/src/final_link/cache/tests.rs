use super::*;

#[test]
fn probe_headers_invalidate_discovery_and_failure_does_not_publish() {
    let directory = tempfile::tempdir().unwrap();
    let target = ResolvedTargetProfile::resolve("x86_64-unknown-linux-gnu").unwrap();
    let original = crate::selected_unwind_prefix(target.lir_target(), None, None).unwrap();
    let prefix = directory.path().join("unwind");
    std::fs::create_dir_all(prefix.join("include")).unwrap();
    std::fs::create_dir_all(prefix.join("lib")).unwrap();
    std::fs::copy(
        original.join("lib/libunwind.a"),
        prefix.join("lib/libunwind.a"),
    )
    .unwrap();
    for entry in std::fs::read_dir(original.join("include")).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name() != "unwind.h" {
            std::os::unix::fs::symlink(
                entry.path(),
                prefix.join("include").join(entry.file_name()),
            )
            .unwrap();
        }
    }
    let header = prefix.join("include/unwind.h");
    let contents = std::fs::read_to_string(original.join("include/unwind.h")).unwrap();
    std::fs::write(&header, &contents).unwrap();
    let options = FinalLinkOptions {
        unwind_prefix: Some(prefix),
        ..Default::default()
    };
    let cache = directory.path().join("cache");
    let cold = resolve(&target, &options, false, &cache).unwrap();
    let record = std::fs::read_dir(cache.join("final-tools-v1"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|ext| ext == "json"))
        .unwrap();
    let modified = || record.metadata().unwrap().modified().unwrap();
    let before = modified();
    let warm = resolve(&target, &options, false, &cache).unwrap();
    assert_eq!(cold, warm);
    assert_eq!(before, modified());

    std::fs::write(&header, "#error changed_unwind_header\n").unwrap();
    let error = resolve(&target, &options, false, &cache).unwrap_err();
    assert!(
        error.to_string().contains("changed_unwind_header"),
        "{error}"
    );
    assert_eq!(before, modified());

    std::fs::write(&header, format!("{contents}// Updated header input.\n")).unwrap();
    let updated = resolve(&target, &options, false, &cache).unwrap();
    assert_ne!(before, modified());
    assert_eq!(cold.fingerprint().unwrap(), updated.fingerprint().unwrap());
    let after = modified();
    assert_eq!(updated, resolve(&target, &options, false, &cache).unwrap());
    assert_eq!(after, modified());
}
