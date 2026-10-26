use std::path::Path;
use std::process::Command;

#[test]
fn decimal_writer_covers_boundaries_and_writes_only_the_actual_bytes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let directory = tempfile::tempdir_in(root.join("tmp")).unwrap();
    let executable = directory.path().join("integer-decimal");
    let compile = Command::new("cc")
        .args(["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", "-I"])
        .arg(root.join("runtime/src"))
        .arg("-I")
        .arg(root.join("sysroot/vendor/ryu"))
        .arg(root.join("runtime/tests/integer_decimal_test.c"))
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let output = Command::new(executable).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"integer decimal tests passed\n");
}
