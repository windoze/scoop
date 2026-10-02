use super::*;

#[test]
fn native_global_and_tls_properties_read_write_and_take_addresses() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let root = environment.build(
        directory.path(),
        "storage",
        "executable",
        &native_fixture("storage/root.scoop"),
        &[],
    );
    stage_snapshots(
        environment,
        directory.path(),
        "storage",
        &[],
        &[],
        "storage/root",
    );
    compile_native(
        directory.path(),
        "m23_storage",
        &native_fixture("storage/native.c"),
        &[],
    );
    std::fs::remove_dir_all(directory.path().join("sources")).unwrap();
    let (program, _) = link_native(environment, &root, &[], directory.path());
    for stress in [false, true] {
        assert_eq!(run(&program, stress), "30\n40\n43\n32\n31\n0\n33\n34\n");
    }
}

#[test]
fn imported_native_properties_use_provider_storage_for_all_input_kinds() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let provider = environment.build(
        directory.path(),
        "storage-provider",
        "library",
        &native_fixture("storage/provider.scoop"),
        &[],
    );
    stage_snapshots(
        environment,
        directory.path(),
        "storage-provider",
        &[],
        &[],
        "storage/provider",
    );
    std::fs::remove_dir_all(directory.path().join("sources/storage-provider")).unwrap();
    let facade = environment.build(
        directory.path(),
        "storage-facade",
        "library",
        &native_fixture("storage/facade.scoop"),
        &[("storage-provider", &provider)],
    );
    stage_snapshots(
        environment,
        directory.path(),
        "storage-facade",
        &[&provider],
        &[],
        "storage/facade",
    );
    std::fs::remove_dir_all(directory.path().join("sources/storage-facade")).unwrap();
    let root = environment.build_with_support(
        directory.path(),
        "storage-root",
        "executable",
        &native_fixture("storage/combined.scoop"),
        &[("storage-facade", &facade)],
        &[&provider],
    );
    stage_snapshots(
        environment,
        directory.path(),
        "storage-root",
        &[&facade],
        &[&provider],
        "storage/combined",
    );
    std::fs::remove_dir_all(directory.path().join("sources")).unwrap();
    for kind in ["object", "archive", "dynamic"] {
        let case = directory.path().join(kind);
        match kind {
            "object" => {
                compile_native(
                    &case,
                    "m23_storage",
                    &native_fixture("storage/native.c"),
                    &[],
                );
            }
            "archive" => {
                let object = compile_native(
                    &case,
                    "storage-payload",
                    &native_fixture("storage/native.c"),
                    &[],
                );
                archive_native(&case, "m23_storage", &[&object]);
            }
            "dynamic" => {
                dylib(
                    &case,
                    "m23_storage",
                    &native_fixture("storage/native.c"),
                    "@rpath/libm23_storage.dylib",
                    &[],
                );
            }
            _ => unreachable!(),
        }
        std::fs::remove_dir_all(case.join("sources")).unwrap();
        let (program, _) = link_native(environment, &root, &[&provider, &facade], &case);
        for stress in [false, true] {
            assert_eq!(
                run(&program, stress),
                "32\n32\n41\n32\n105\ntrue\n42\n35\ncaller-alive\n",
                "{kind}"
            );
        }
    }
}
