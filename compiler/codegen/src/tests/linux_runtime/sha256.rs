use super::{checked, workspace};
use std::process::Command;

#[test]
fn portable_sha256_matches_known_vectors_with_each_libc() {
    let workspace = workspace();
    let root = workspace.join("target/linux-runtime-tests");
    std::fs::create_dir_all(&root).unwrap();
    let directory = tempfile::tempdir_in(root).unwrap();
    let vendor = workspace.join("runtime/third_party/mbedtls");
    for (compiler, flags) in [
        ("gcc", ["-fPIE", "-pie"]),
        ("musl-gcc", ["-static", "-no-pie"]),
        ("musl-gcc", ["-fPIE", "-pie"]),
    ] {
        let binary = directory.path().join("sha256");
        let mut command = Command::new(compiler);
        command
            .env("TMPDIR", directory.path())
            .args([
                "-std=c11",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-O2",
                "-DMBEDTLS_CONFIG_FILE=\"scoop_sha256_config.h\"",
            ])
            .args(flags)
            .arg("-I")
            .arg(&vendor)
            .arg("-I")
            .arg(vendor.join("include"));
        for source in [
            "runtime/tests/sha256_test.c",
            "runtime/src/platform/image/portable_sha256.c",
            "runtime/third_party/mbedtls/library/sha256.c",
            "runtime/third_party/mbedtls/library/platform_util.c",
        ] {
            command.arg(workspace.join(source));
        }
        checked(command.arg("-o").arg(&binary));
        assert_eq!(
            checked(&mut Command::new(&binary)).stdout,
            b"SHA-256 empty, short, multi-block and million-byte vectors passed\n"
        );
        let symbols = checked(
            Command::new("nm")
                .args(["--defined-only", "-g"])
                .arg(binary),
        );
        assert!(
            !String::from_utf8_lossy(&symbols.stdout)
                .lines()
                .any(|line| line
                    .split_whitespace()
                    .last()
                    .is_some_and(|name| name.starts_with("mbedtls_")))
        );
    }
}
