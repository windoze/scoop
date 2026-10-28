use std::{fs, process::Command};

#[test]
fn creates_and_lists_a_virtual_workspace_without_a_toolchain() {
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("project");
    let executable = env!("CARGO_BIN_EXE_scoop-pm");
    let output = Command::new(executable)
        .args(["new", "--workspace"])
        .arg(&destination)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(executable)
        .args(["members", "--json"])
        .current_dir(&destination)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["members"].as_array().unwrap().len(), 0);
    let before = fs::read(destination.join("Cone.toml")).unwrap();
    let output = Command::new(executable)
        .args(["new", "--workspace"])
        .arg(&destination)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(fs::read(destination.join("Cone.toml")).unwrap(), before);
}

#[test]
fn malformed_manifest_reports_its_path_and_location() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(
        directory.path().join("Cone.toml"),
        "schema = 1\n[workspace]\nmembers = ['../outside']\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_scoop-pm"))
        .arg("members")
        .current_dir(directory.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains("Cone.toml") && error.contains("bytes") && error.contains("escape"));
}
