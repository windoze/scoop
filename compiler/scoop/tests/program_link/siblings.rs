use super::*;

#[test]
fn sibling_generic_delegates_share_storage_and_cached_failures() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path();
    let provider = environment.build(
        path,
        "provider",
        "library",
        &fixture("delegate-provider.scoop"),
        &[],
    );
    let left = environment.build(
        path,
        "left",
        "library",
        &fixture("delegate-left.scoop"),
        &[("provider", &provider)],
    );
    let right = environment.build(
        path,
        "right",
        "library",
        &fixture("delegate-right.scoop"),
        &[("provider", &provider)],
    );
    let root = environment.build(
        path,
        "root",
        "executable",
        &fixture("delegate-consumer.scoop"),
        &[("provider", &provider), ("left", &left), ("right", &right)],
    );
    std::fs::remove_dir_all(path.join("sources")).unwrap();
    let output = path.join("program");
    environment.link(&root, &[&right, &provider, &left], &output);
    for stress in [false, true] {
        assert_eq!(run(&output, stress), "42\n");
    }
}
