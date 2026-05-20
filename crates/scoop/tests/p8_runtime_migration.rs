#![cfg(all(feature = "llvm", not(windows)))]

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::tempdir;

fn scoop_bin() -> &'static str {
    env!("CARGO_BIN_EXE_scoop")
}

fn workspace_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn nm_global_symbols(path: &Path) -> String {
    let mut last_error = String::new();
    for tool in ["llvm-nm", "nm"] {
        match Command::new(tool).arg("-g").arg(path).output() {
            Ok(output) if output.status.success() => {
                return String::from_utf8_lossy(&output.stdout).into_owned();
            }
            Ok(output) => {
                last_error = format!(
                    "{tool} failed: status={:?}, stderr={}",
                    output.status.code(),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            Err(err) => {
                last_error = format!("{tool} failed to start: {err}");
            }
        }
    }
    panic!(
        "failed to inspect symbols for {}: {last_error}",
        path.display()
    );
}

#[test]
fn normal_build_does_not_export_runtime_test_helpers() {
    let fixture = workspace_path("tests/fixtures/run-pass/auto_prelude_core_basic.scoop");
    let dir = tempdir().unwrap();
    let output_exe = dir.path().join("normal-no-runtime-test");

    let build = Command::new(scoop_bin())
        .args([
            OsStr::new("build"),
            fixture.as_os_str(),
            OsStr::new("--no-incremental"),
            OsStr::new("-o"),
            output_exe.as_os_str(),
        ])
        .output()
        .unwrap();

    assert!(build.status.success(), "build failed: {build:?}");

    let symbols = nm_global_symbols(&output_exe);
    assert!(
        !symbols.contains("scoop_test_")
            && !symbols.contains("scoop_runtime_test_sync")
            && !symbols.contains("scoop_sync_"),
        "ordinary runtime link should not export migrated runtime helpers:\n{symbols}"
    );
}
