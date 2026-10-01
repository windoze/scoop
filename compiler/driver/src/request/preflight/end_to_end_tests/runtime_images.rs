use super::imported_classes::runtime;
use super::*;

mod support;
use support::{build_fixture, run};

#[test]
fn multi_image_startup_runs_empty_ordinary_and_nogc_roots() {
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
    for (name, expected) in [("empty", ""), ("ordinary", "42\n"), ("nogc", "")] {
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
            None,
        );
    }
}

#[test]
fn multi_image_startup_uses_the_current_ready_set_and_complete_roots() {
    let target = resolved_target().unwrap();
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let base = build_fixture(
        sysroot.path(),
        &target,
        "schedule-a-base",
        "library",
        &[],
        &[],
    );
    let next = build_fixture(
        sysroot.path(),
        &target,
        "schedule-b-next",
        "library",
        &[&base],
        &[],
    );
    let peer = build_fixture(
        sysroot.path(),
        &target,
        "schedule-c-peer",
        "library",
        &[],
        &[],
    );
    let join = build_fixture(
        sysroot.path(),
        &target,
        "schedule-d-join",
        "library",
        &[&next, &peer],
        &[&base],
    );
    let root = build_fixture(
        sysroot.path(),
        &target,
        "schedule-root",
        "executable",
        &[&join],
        &[&base, &next, &peer],
    );
    let library = runtime::build(&target, &sysroot.path().join("runtime"));
    run(
        &target,
        &[&core, &base, &next, &peer, &join, &root],
        &library,
        &sysroot.path().join("run"),
        "10\n20\n30\n40\n50\n",
        None,
    );
}

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
