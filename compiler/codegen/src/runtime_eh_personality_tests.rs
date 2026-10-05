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
fn scoop_personality_implements_the_closed_two_phase_contract() {
    const COMPILE_AND_LINK_ARGUMENTS: &[&str] = &["-std=c11", "-Wall", "-Wextra", "-Werror"];
    const FORBIDDEN_LINK_ARGUMENTS: &[&str] = &["-lc++abi", "-lunwind"];

    for forbidden in FORBIDDEN_LINK_ARGUMENTS {
        assert!(
            !COMPILE_AND_LINK_ARGUMENTS.contains(forbidden),
            "EH personality test harness must not link {forbidden}"
        );
    }

    let workspace = workspace_root();
    let binary =
        std::env::temp_dir().join(format!("scoop_eh_personality_test_{}", std::process::id()));
    let compile = Command::new("cc")
        .args(COMPILE_AND_LINK_ARGUMENTS)
        .arg("-I")
        .arg(workspace.join("runtime/src"))
        .arg(workspace.join("runtime/src/eh_personality.c"))
        .arg(workspace.join("runtime/src/eh/lsda.c"))
        .arg(workspace.join("runtime/tests/eh_personality_test.c"))
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("compile the Scoop personality phase test");
    assert!(
        compile.status.success(),
        "Scoop personality phase test must compile cleanly:\n{}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let output = Command::new(&binary)
        .output()
        .expect("run the Scoop personality phase test");
    std::fs::remove_file(&binary).ok();
    assert!(
        output.status.success(),
        "Scoop personality phase test failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"EH personality tests passed\n");
}
