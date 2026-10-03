use super::*;

#[test]
fn empty_and_nogc_roots_and_early_initialization_chain() {
    let environment = environment();
    for (name, expected) in [("empty", ""), ("nogc", "")] {
        let directory = tempfile::tempdir().unwrap();
        let root = environment.build(
            directory.path(),
            name,
            "executable",
            &fixture(&format!("{name}.scoop")),
            &[],
        );
        let output = directory.path().join("program");
        std::fs::remove_dir_all(directory.path().join("sources")).unwrap();
        environment.link(&root, &[], &output);
        for stress in [false, true] {
            assert_eq!(run(&output, stress), expected);
        }
    }
    pair("early-chain", "early-root", "6\n5\n4\n3\n2\n1\n7\n");
}

#[test]
fn lazy_failure_cycle_and_recovery_preserve_the_original_failure() {
    pair("lazy-failure-provider", "lazy-failure-root", "1\n42\n");
    pair("lazy-failure-provider", "unused-lazy-root", "42\n");
    pair("lazy-cycle-provider", "lazy-cycle-root", "1\n42\n");
    pair("cycle-recovery-provider", "cycle-recovery-root", "42\n");
}

fn pair(provider: &str, consumer: &str, expected: &str) {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let library = environment.build(
        directory.path(),
        provider,
        "library",
        &fixture(&format!("{provider}.scoop")),
        &[],
    );
    let root = environment.build(
        directory.path(),
        consumer,
        "executable",
        &fixture(&format!("{consumer}.scoop")),
        &[(provider, &library)],
    );
    std::fs::remove_dir_all(directory.path().join("sources")).unwrap();
    let output = directory.path().join("program");
    environment.link(&root, &[&library], &output);
    for stress in [false, true] {
        assert_eq!(run(&output, stress), expected, "{consumer}");
    }
}

#[test]
fn diamond_uses_the_current_ready_set_and_has_a_locator_independent_plan() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path();
    let a = environment.build(
        path,
        "a-base",
        "library",
        &fixture("schedule-a-base.scoop"),
        &[],
    );
    let b = environment.build(
        path,
        "b-next",
        "library",
        &fixture("schedule-b-next.scoop"),
        &[("a-base", &a)],
    );
    let c = environment.build(
        path,
        "c-peer",
        "library",
        &fixture("schedule-c-peer.scoop"),
        &[],
    );
    let d = environment.build_with_support(
        path,
        "d-join",
        "library",
        &fixture("schedule-d-join.scoop"),
        &[("b-next", &b), ("c-peer", &c)],
        &[&a],
    );
    let root = environment.build_with_support(
        path,
        "root",
        "executable",
        &fixture("schedule-root.scoop"),
        &[("d-join", &d)],
        &[&a, &b, &c],
    );
    std::fs::remove_dir_all(path.join("sources")).unwrap();
    let output = path.join("program");
    let plan = environment.link(&root, &[&d, &c, &b, &a], &output);
    assert!(plan.contains("images=6 "), "{plan}");
    for stress in [false, true] {
        assert_eq!(run(&output, stress), "10\n20\n30\n40\n50\n");
    }
    let relocated = path.join("elsewhere");
    std::fs::create_dir(&relocated).unwrap();
    let copies: Vec<_> = [&a, &b, &c, &d, &root]
        .iter()
        .enumerate()
        .map(|(index, artifact)| {
            let copy = relocated.join(format!("{index}.slib"));
            std::fs::rename(artifact, &copy).unwrap();
            copy
        })
        .collect();
    let reordered = environment.link(
        &copies[4],
        &[&copies[2], &copies[0], &copies[1], &copies[3], &copies[0]],
        &relocated.join("renamed-program"),
    );
    assert_eq!(plan, reordered);
}

#[test]
fn uncaught_root_and_eager_failures_stop_at_the_runtime_gateway() {
    use std::os::unix::process::ExitStatusExt;
    let environment = environment();
    for name in ["root-failure", "eager-failure"] {
        let directory = tempfile::tempdir().unwrap();
        let root = environment.build(
            directory.path(),
            name,
            "executable",
            &fixture(&format!("{name}.scoop")),
            &[],
        );
        let program = directory.path().join("program");
        std::fs::remove_dir_all(directory.path().join("sources")).unwrap();
        environment.link(&root, &[], &program);
        for stress in [false, true] {
            let output = Command::new(&program)
                .env("SCOOP_GC_STRESS_MOVE", if stress { "1" } else { "0" })
                .output()
                .unwrap();
            assert_eq!(output.status.signal(), Some(6));
            let error = String::from_utf8(output.stderr).unwrap();
            assert!(error.contains("IllegalStateException"), "{error}");
            assert_eq!(
                output.stdout,
                if name == "eager-failure" {
                    b"11\n".as_slice()
                } else {
                    b""
                }
            );
        }
    }
}

#[test]
fn failed_dependency_prevents_later_eager_initializers_and_main() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path();
    let bad = environment.build(
        path,
        "a-bad",
        "library",
        &fixture("eager-bad-provider.scoop"),
        &[],
    );
    let later = environment.build(
        path,
        "b-later",
        "library",
        &fixture("eager-later-provider.scoop"),
        &[],
    );
    let root = environment.build(
        path,
        "root",
        "executable",
        &fixture("eager-later-root.scoop"),
        &[("a-bad", &bad), ("b-later", &later)],
    );
    std::fs::remove_dir_all(path.join("sources")).unwrap();
    let program = path.join("program");
    environment.link(&root, &[&later, &bad], &program);
    for stress in [false, true] {
        let output = Command::new(&program)
            .env("SCOOP_GC_STRESS_MOVE", if stress { "1" } else { "0" })
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert_eq!(output.stdout, b"1\n");
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(
            error.contains("during initialization of") && error.contains("IllegalStateException"),
            "{error}"
        );
    }
}
