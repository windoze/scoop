use super::*;

#[test]
fn early_ensure_runs_each_eager_unit_once_before_its_scheduled_turn() {
    let target = resolved_target().unwrap();
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let provider = build_fixture(sysroot.path(), &target, "early-chain", "library", &[], &[]);
    let root = build_fixture(
        sysroot.path(),
        &target,
        "early-root",
        "executable",
        &[&provider],
        &[],
    );
    let closure = runtime::read(&target, &[&core, &provider, &root]);
    let (sections, _) = closure
        .artifact(
            provider
                .artifact()
                .summary()
                .coordinate()
                .identity()
                .unwrap(),
        )
        .unwrap();
    let units = sections
        .lir_strong_production()
        .registration_production()
        .initialization_units()
        .registrations();
    assert_eq!(units.len(), 6);
    // The executed dependency chain differs from scheduling by persistent unit ID.
    // At least one ensure therefore runs a unit before its own gateway turn.
    let paths = units
        .iter()
        .map(|unit| unit.semantic().diagnostic_path())
        .collect::<Vec<_>>();
    assert!(
        !paths.windows(2).all(|pair| pair[0] > pair[1]),
        "fixture must exercise early ensure"
    );
    let library = runtime::build(&target, &sysroot.path().join("runtime"));
    run(
        &target,
        &[&core, &provider, &root],
        &library,
        &sysroot.path().join("run"),
        "6\n5\n4\n3\n2\n1\n7\n",
        None,
    );
}

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
