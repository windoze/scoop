use super::*;

#[test]
fn simultaneous_links_publish_one_complete_executable() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let root = environment.build(
        directory.path(),
        "root",
        "executable",
        &fixture("read-println.scoop"),
        &[],
    );
    std::fs::remove_dir_all(directory.path().join("sources")).unwrap();
    let output = directory.path().join("program");
    std::fs::write(&output, "previous output").unwrap();
    let first = environment
        .link_command(&root, &[], &output)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let second = environment
        .link_command(&root, &[], &output)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let first = first.wait_with_output().unwrap();
    let second = second.wait_with_output().unwrap();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert_eq!(first.stdout, second.stdout);
    assert_eq!(run(&output, false), "42\n");
    assert_eq!(run(&output, true), "42\n");
    assert!(!std::fs::read_dir(directory.path()).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".scoop-link-")
    }));
}

#[test]
fn failed_rename_leaves_the_original_output_directory_untouched() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let root = environment.build(
        directory.path(),
        "root",
        "executable",
        &fixture("empty.scoop"),
        &[],
    );
    std::fs::remove_dir_all(directory.path().join("sources")).unwrap();
    let output = directory.path().join("existing");
    std::fs::create_dir(&output).unwrap();
    std::fs::write(output.join("keep"), "old contents").unwrap();
    let result = environment
        .link_command(&root, &[], &output)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("cannot publish executable"));
    assert_eq!(std::fs::read(output.join("keep")).unwrap(), b"old contents");
}
