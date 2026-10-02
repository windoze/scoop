use super::*;

#[test]
fn imported_core_runtime_intrinsics_collect_with_live_references() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let root = environment.build(
        directory.path(),
        "gc",
        "executable",
        &native_fixture("runtime/gc.scoop"),
        &[],
    );
    stage_snapshots(environment, directory.path(), "gc", &[], &[], "runtime/gc");
    std::fs::remove_dir_all(directory.path().join("sources")).unwrap();
    let program = directory.path().join("program");
    environment.link(&root, &[], &program);
    for stress in [false, true] {
        assert_eq!(run(&program, stress), "runtime-alive\ntrue\n");
    }
}

#[test]
fn generic_provider_bodies_preserve_pin_and_handle_results_across_gc() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let provider = environment.build(
        directory.path(),
        "gc-provider",
        "library",
        &native_fixture("runtime/provider.scoop"),
        &[],
    );
    stage_snapshots(
        environment,
        directory.path(),
        "gc-provider",
        &[],
        &[],
        "runtime/provider",
    );
    std::fs::remove_dir_all(directory.path().join("sources/gc-provider")).unwrap();
    let root = environment.build(
        directory.path(),
        "gc-combined",
        "executable",
        &native_fixture("runtime/combined.scoop"),
        &[("gc-provider", &provider)],
    );
    stage_snapshots(
        environment,
        directory.path(),
        "gc-combined",
        &[&provider],
        &[],
        "runtime/combined",
    );
    std::fs::remove_dir_all(directory.path().join("sources")).unwrap();
    let program = directory.path().join("program");
    environment.link(&root, &[&provider], &program);
    for stress in [false, true] {
        assert_eq!(
            run(&program, stress),
            "handles-alive\nmarker-alive\nhandles-alive\n"
        );
    }
}
