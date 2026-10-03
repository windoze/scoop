//! The production artifact consumer runs without the source trees or compiler.

#[path = "program_link/mod.rs"]
mod support;

use support::{environment, fixture, run};

#[test]
fn artifact_only_process_links_core_library_and_executable() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let library = environment.build(
        directory.path(),
        "library",
        "library",
        &fixture("basic-library.scoop"),
        &[],
    );
    let root = environment.build(
        directory.path(),
        "root",
        "executable",
        &fixture("basic-root.scoop"),
        &[("library", &library)],
    );
    std::fs::remove_dir_all(directory.path().join("sources")).unwrap();
    let executable = directory.path().join("program");
    let result = environment.link(&root, &[&library], &executable);
    assert!(result.contains("program-link v1\n"), "{result}");
    assert!(result.contains("images=3 "), "{result}");
    support::assert_plan_snapshot(&result);
    assert_eq!(run(&executable, false), "42\n");
    assert_eq!(run(&executable, true), "42\n");
}
