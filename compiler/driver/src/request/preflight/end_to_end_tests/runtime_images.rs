use super::imported_classes::runtime;
use super::*;

mod initialization;
mod lifecycle;
mod support;
use support::{build_fixture, run};

#[test]
fn multi_image_startup_reports_managed_root_and_eager_failures() {
    let target = resolved_target().unwrap();
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let provider = build_fixture(
        sysroot.path(),
        &target,
        "empty-provider",
        "library",
        &[],
        &[],
    );
    let library = runtime::build(&target, &sysroot.path().join("runtime"));
    for (name, expected) in [("root-failure", ""), ("eager-failure", "11\n")] {
        let root = build_fixture(
            sysroot.path(),
            &target,
            name,
            "executable",
            &[&provider],
            &[],
        );
        run(
            &target,
            &[&core, &provider, &root],
            &library,
            &sysroot.path().join(format!("run-{name}")),
            expected,
            Some("x=IllegalStateException)"),
        );
    }
}
