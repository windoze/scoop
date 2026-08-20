use std::path::Path;
use std::process::Command;

#[test]
fn runtime_utf8_decoder_preserves_scalars_and_maximal_subparts_without_overreading() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap();
    let temporary = tempfile::tempdir().unwrap();
    for optimization in ["-O0", "-O2"] {
        let binary = temporary.path().join("utf8-decode");
        let compile = Command::new("cc")
            .args([
                "-std=c11",
                "-D_DEFAULT_SOURCE",
                "-Wall",
                "-Wextra",
                "-Werror",
                optimization,
            ])
            .arg(workspace.join("runtime/src/utf8.c"))
            .arg(workspace.join("runtime/tests/utf8_decode_test.c"))
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            compile.status.success(),
            "{}",
            String::from_utf8_lossy(&compile.stderr)
        );
        let output = Command::new(&binary).output().unwrap();
        assert!(output.status.success(), "{optimization}: {output:?}");
        assert_eq!(
            output.stdout,
            b"UTF-8 scalars, maximal subparts and bounded reads passed\n"
        );
    }
}
