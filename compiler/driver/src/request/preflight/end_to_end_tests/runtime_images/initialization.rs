use super::*;

#[test]
fn failed_provider_stops_later_eager_units_and_main() {
    let target = resolved_target().unwrap();
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let failed = build_fixture(
        sysroot.path(),
        &target,
        "eager-bad-provider",
        "library",
        &[],
        &[],
    );
    let later = build_fixture(
        sysroot.path(),
        &target,
        "eager-later-provider",
        "library",
        &[&failed],
        &[],
    );
    let root = build_fixture(
        sysroot.path(),
        &target,
        "eager-later-root",
        "executable",
        &[&later],
        &[&failed],
    );
    let library = runtime::build(&target, &sysroot.path().join("runtime"));
    run(
        &target,
        &[&core, &failed, &later, &root],
        &library,
        &sysroot.path().join("run"),
        "1\n",
        Some("during initialization of"),
    );
}
