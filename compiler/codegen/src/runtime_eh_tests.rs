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
fn closed_lsda_decoder_accepts_only_the_qualified_profile() {
    let workspace = workspace_root();
    let binary = std::env::temp_dir().join(format!("scoop_eh_decoder_test_{}", std::process::id()));
    let compile = Command::new("cc")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror"])
        .arg("-I")
        .arg(workspace.join("runtime/src"))
        .arg(workspace.join("runtime/src/eh_personality.c"))
        .arg(workspace.join("runtime/src/eh/lsda.c"))
        .arg(workspace.join("runtime/tests/eh_decoder_test.c"))
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("compile the bounded LSDA decoder test");
    assert!(
        compile.status.success(),
        "bounded LSDA decoder test must compile cleanly:\n{}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let output = Command::new(&binary)
        .output()
        .expect("run the bounded LSDA decoder test");
    std::fs::remove_file(&binary).ok();
    assert!(
        output.status.success(),
        "bounded LSDA decoder test failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"EH decoder tests passed\n");
}
