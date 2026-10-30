use super::*;

#[test]
fn object_records_reuse_outputs_and_follow_actual_header_changes() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("cone");
    let manifest = manifest(
        &root,
        "#include <value.h>\nint answer(void);\nint answer(void) { return VALUE; }\n",
    );
    std::fs::write(root.join("include/value.h"), "#define VALUE 41\n").unwrap();
    let cache = directory.path().join("cache");
    let prepare = || {
        prepare_native_objects(
            &manifest,
            target(),
            OptimizationMode::Debug,
            &crate::development_runtime_root(),
            &cache,
            scoop_protocol::CompilationPurpose::Production,
        )
        .unwrap()
    };
    let cold = prepare();
    let before = cold.objects()[0].metadata().unwrap().modified().unwrap();
    assert_eq!(prepare(), cold);
    std::fs::write(root.join("program.scoop"), "fun value(): Int = 1\n").unwrap();
    assert_eq!(prepare(), cold);
    std::fs::write(root.join("program.scoop"), "fun value(): Int = 2\n").unwrap();
    assert_eq!(prepare(), cold);
    assert_eq!(
        cold.objects()[0].metadata().unwrap().modified().unwrap(),
        before
    );
    std::fs::write(root.join("include/value.h"), "#define VALUE 42\n").unwrap();
    let changed = prepare();
    assert_ne!(cold.key(), changed.key());
    assert_ne!(cold.objects(), changed.objects());
    assert_ne!(
        std::fs::read(&cold.objects()[0]).unwrap(),
        std::fs::read(&changed.objects()[0]).unwrap()
    );
    assert_eq!(prepare(), changed);
}

#[test]
fn quoted_macro_include_observes_its_own_directory() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("cone");
    let manifest = manifest(
        &root,
        "#include <nested/entry.h>\nint answer(void);\nint answer(void) { return VALUE; }\n",
    );
    std::fs::create_dir_all(root.join("include/nested")).unwrap();
    std::fs::write(
        root.join("include/nested/entry.h"),
        "#define HEADER \"value.h\"\n#include HEADER\n",
    )
    .unwrap();
    std::fs::write(root.join("include/value.h"), "#define VALUE 1\n").unwrap();
    let cache = directory.path().join("cache");
    let prepare = || {
        prepare_native_objects(
            &manifest,
            target(),
            OptimizationMode::Debug,
            &crate::development_runtime_root(),
            &cache,
            scoop_protocol::CompilationPurpose::Production,
        )
        .unwrap()
    };
    let cold = prepare();
    std::fs::write(root.join("include/nested/unrelated.h"), "").unwrap();
    assert_eq!(prepare(), cold);
    std::fs::write(root.join("include/nested/value.h"), "#define VALUE 2\n").unwrap();
    let changed = prepare();
    assert_ne!(changed.key(), cold.key());
    assert_ne!(
        std::fs::read(&cold.objects()[0]).unwrap(),
        std::fs::read(&changed.objects()[0]).unwrap()
    );
    assert_eq!(prepare(), changed);
}

#[test]
fn native_compilation_failure_does_not_publish_a_record() {
    let directory = tempfile::tempdir().unwrap();
    let manifest = manifest(&directory.path().join("cone"), "this is not valid C;\n");
    let cache = directory.path().join("cache");
    assert!(
        prepare_native_objects(
            &manifest,
            target(),
            OptimizationMode::Debug,
            &crate::development_runtime_root(),
            &cache,
            scoop_protocol::CompilationPurpose::Production,
        )
        .is_err()
    );
    assert!(
        std::fs::read_dir(cache.join("native-objects-v2"))
            .unwrap()
            .all(|entry| entry
                .unwrap()
                .path()
                .extension()
                .is_none_or(|e| e != "json"))
    );
}
