use super::*;

#[test]
fn lazy_initialization_retains_failed_roots_and_catches_cycles() {
    let target = resolved_target().unwrap();
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let library = runtime::build(&target, &sysroot.path().join("runtime"));
    for (provider_name, roots) in [
        (
            "lazy-failure-provider",
            vec![
                ("unused-lazy-root", "42\n"),
                ("lazy-failure-root", "1\n42\n"),
            ],
        ),
        ("lazy-cycle-provider", vec![("lazy-cycle-root", "1\n42\n")]),
        (
            "cycle-recovery-provider",
            vec![("cycle-recovery-root", "42\n")],
        ),
    ] {
        let provider = build_fixture(sysroot.path(), &target, provider_name, "library", &[], &[]);
        for (name, expected) in roots {
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
}

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
