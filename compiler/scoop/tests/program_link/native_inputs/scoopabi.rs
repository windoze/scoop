use super::*;

fn managed_object(directory: &Path, name: &str) -> PathBuf {
    let include = format!("-I{}", workspace().join("runtime/include").display());
    compile_native(
        directory,
        name,
        &native_fixture("scoopabi/native.c"),
        &[&include],
    )
}

#[test]
fn native_scoop_abi_keeps_borrowed_references_and_elides_zst_payloads() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let root = environment.build(
        directory.path(),
        "scoopabi",
        "executable",
        &native_fixture("scoopabi/standalone.scoop"),
        &[],
    );
    stage_snapshots(
        environment,
        directory.path(),
        "scoopabi",
        &[],
        &[],
        "scoopabi/standalone",
    );
    managed_object(directory.path(), "m23_scoopabi");
    std::fs::remove_dir_all(directory.path().join("sources")).unwrap();
    let (program, _) = link_native(environment, &root, &[], directory.path());
    for stress in [false, true] {
        assert_eq!(run(&program, stress), "managed-alive\n42\ncaller-alive\n");
    }
}

#[test]
fn imported_scoop_abi_publishes_native_roots_and_indirect_results_across_gc() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let provider = environment.build(
        directory.path(),
        "scoopabi-provider",
        "library",
        &native_fixture("scoopabi/provider.scoop"),
        &[],
    );
    stage_snapshots(
        environment,
        directory.path(),
        "scoopabi-provider",
        &[],
        &[],
        "scoopabi/provider",
    );
    std::fs::remove_dir_all(directory.path().join("sources/scoopabi-provider")).unwrap();
    let facade = environment.build(
        directory.path(),
        "scoopabi-facade",
        "library",
        &native_fixture("scoopabi/facade.scoop"),
        &[("scoopabi-provider", &provider)],
    );
    stage_snapshots(
        environment,
        directory.path(),
        "scoopabi-facade",
        &[&provider],
        &[],
        "scoopabi/facade",
    );
    std::fs::remove_dir_all(directory.path().join("sources/scoopabi-facade")).unwrap();
    let root = environment.build_with_support(
        directory.path(),
        "scoopabi-root",
        "executable",
        &native_fixture("scoopabi/root.scoop"),
        &[("scoopabi-facade", &facade)],
        &[&provider],
    );
    stage_snapshots(
        environment,
        directory.path(),
        "scoopabi-root",
        &[&facade],
        &[&provider],
        "scoopabi/root",
    );
    std::fs::remove_dir_all(directory.path().join("sources")).unwrap();
    for kind in ["object", "archive", "dynamic"] {
        let case = directory.path().join(kind);
        if kind == "object" {
            managed_object(&case, "m23_scoopabi");
            compile_native(
                &case,
                "m23_scoop_leaf",
                &native_fixture("scoopabi/leaf.c"),
                &[],
            );
        } else {
            let object = managed_object(&case, "managed");
            archive_native(&case, "m23_scoopabi", &[&object]);
            if kind == "archive" {
                let object = compile_native(&case, "leaf", &native_fixture("scoopabi/leaf.c"), &[]);
                archive_native(&case, "m23_scoop_leaf", &[&object]);
            } else {
                dylib(
                    &case,
                    "m23_scoop_leaf",
                    &native_fixture("scoopabi/leaf.c"),
                    "@rpath/libm23_scoop_leaf.dylib",
                    &[],
                );
            }
        }
        std::fs::remove_dir_all(case.join("sources")).unwrap();
        let (program, _) = link_native(environment, &root, &[&provider, &facade], &case);
        for stress in [false, true] {
            assert_eq!(
                run(&program, stress),
                "managed-alive\n11\n22\n42\ncaller-alive\n",
                "{kind}"
            );
        }
    }
}
