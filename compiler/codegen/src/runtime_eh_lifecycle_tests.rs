use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("codegen crate is nested below the workspace root")
        .to_path_buf()
}

#[test]
fn scoop_exception_record_lifecycle_uses_only_the_level_one_unwinder_abi() {
    const COMPILE_AND_LINK_ARGUMENTS: &[&str] =
        &["-std=c11", "-Wall", "-Wextra", "-Werror", "-pthread"];
    const FORBIDDEN_LINK_ARGUMENTS: &[&str] = &["-lc++abi", "-lunwind"];

    for forbidden in FORBIDDEN_LINK_ARGUMENTS {
        assert!(
            !COMPILE_AND_LINK_ARGUMENTS.contains(forbidden),
            "EH lifecycle test harness must not link {forbidden}"
        );
    }

    let workspace = workspace_root();
    let binary =
        std::env::temp_dir().join(format!("scoop_eh_lifecycle_test_{}", std::process::id()));
    let compile = Command::new("cc")
        .args(COMPILE_AND_LINK_ARGUMENTS)
        .arg("-I")
        .arg(workspace.join("runtime/include"))
        .arg("-I")
        .arg(workspace.join("runtime/src"))
        .arg(workspace.join("runtime/src/eh.c"))
        .arg(workspace.join("runtime/tests/eh_lifecycle_test.c"))
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("compile the Scoop EH record lifecycle test");
    assert!(
        compile.status.success(),
        "Scoop EH record lifecycle test must compile cleanly:\n{}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let output = Command::new(&binary)
        .output()
        .expect("run the Scoop EH record lifecycle test");
    std::fs::remove_file(&binary).ok();
    assert!(
        output.status.success(),
        "Scoop EH record lifecycle test failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"EH lifecycle tests passed\n");
}
