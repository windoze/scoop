use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root exists")
}

fn unique_temp_path(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time follows the Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "scoop-warning-{label}-{}-{nonce}",
        std::process::id()
    ))
}

#[test]
fn warning_is_rendered_to_stderr_without_failing_the_cli() {
    let workspace = workspace_root();
    let fixture = workspace.join("tests/fixtures/m22-patterns/suspicious-enum-catch-all.scoop");
    let out_dir = unique_temp_path("cli");

    let output = Command::new(env!("CARGO_BIN_EXE_scoopc"))
        .current_dir(&workspace)
        .arg("build")
        .arg(&fixture)
        .arg("--emit")
        .arg("hir")
        .arg("-o")
        .arg(&out_dir)
        .output()
        .expect("run scoopc");
    let _ = std::fs::remove_dir_all(&out_dir);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "scoopc failed:\n{stderr}");
    assert!(
        stdout.contains("enum Signal"),
        "missing HIR dump:\n{stdout}"
    );
    assert!(
        !stdout.contains("warning:"),
        "warning leaked into --emit stdout:\n{stdout}"
    );
    assert!(
        stderr.contains(
            "warning: `Raedy` is a catch-all binding because `Signal` has no variant named `Raedy`"
        ),
        "missing warning:\n{stderr}"
    );
    assert!(!stderr.contains("error:"), "unexpected error:\n{stderr}");
}

#[test]
fn warning_is_retained_when_a_later_driver_step_fails() {
    let workspace = workspace_root();
    let fixture = workspace.join("tests/fixtures/m22-patterns/suspicious-enum-catch-all.scoop");
    let blocked_output = unique_temp_path("blocked-output");
    std::fs::write(&blocked_output, "not a directory").expect("create output blocker");

    let diagnostics = match scoopc::compile_file(&fixture, &blocked_output) {
        Ok(_) => panic!("an existing file cannot be used as the output directory"),
        Err(diagnostics) => diagnostics,
    };
    let _ = std::fs::remove_file(&blocked_output);

    assert_eq!(diagnostics.len(), 2);
    assert_eq!(
        diagnostics[0].severity,
        scoop_ast::DiagnosticSeverity::Error
    );
    assert!(
        diagnostics[0]
            .message
            .contains("cannot create output directory")
    );
    assert_eq!(
        diagnostics[1].severity,
        scoop_ast::DiagnosticSeverity::Warning
    );
    assert!(diagnostics[1].message.contains("catch-all binding"));
}
