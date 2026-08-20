use super::{checked, workspace};
use std::process::Command;

#[test]
fn elf_images_use_loaded_segments_and_actual_read_only_mappings() {
    let workspace = workspace();
    let root = workspace.join("target/linux-runtime-tests");
    std::fs::create_dir_all(&root).unwrap();
    let directory = tempfile::tempdir_in(root).unwrap();
    for (compiler, mode, flags) in [
        ("gcc", "dynamic", ["-fPIE", "-pie"]),
        ("musl-gcc", "static", ["-static", "-no-pie"]),
        ("musl-gcc", "dynamic", ["-fPIE", "-pie"]),
    ] {
        let binary = directory.path().join("image");
        let script = workspace.join(format!("compiler/toolchain/src/elf/{mode}.ld"));
        let mut command = Command::new(compiler);
        command
            .env("TMPDIR", directory.path())
            .args([
                "-std=c11",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-O2",
                "-Wl,-z,relro,-z,now,-z,text",
            ])
            .args(flags)
            .arg("-Wl,-T")
            .arg(script);
        for source in [
            "runtime/tests/linux_image_test.c",
            "runtime/src/platform/image/elf.c",
            "runtime/src/platform/common.c",
            "runtime/src/platform/profiles/linux_x86_64.c",
            "runtime/src/platform/os/linux.c",
            "runtime/src/platform/arch/x86_64.c",
            "runtime/src/image/ranges.c",
        ] {
            command.arg(workspace.join(source));
        }
        checked(command.arg("-o").arg(&binary));
        let headers = checked(Command::new("readelf").args(["-l", "-d"]).arg(&binary));
        let headers = String::from_utf8(headers.stdout).unwrap();
        assert!(!headers.contains("TEXTREL"));
        assert_eq!(headers.contains("INTERP"), mode == "dynamic");
        if mode == "static" {
            assert!(!headers.contains("NEEDED"));
        }
        let expected = b"ELF load bias, stackmap bounds and actual metadata permissions passed\n";
        assert_eq!(checked(&mut Command::new(&binary)).stdout, expected);
        // The kernel and runtime need program headers only. Remove the section
        // table locator to exercise this on the same executable without a reader.
        let mut bytes = std::fs::read(&binary).unwrap();
        assert_eq!(&bytes[..6], b"\x7fELF\x02\x01");
        bytes[40..48].fill(0);
        bytes[58..64].fill(0);
        std::fs::write(&binary, bytes).unwrap();
        assert_eq!(checked(&mut Command::new(&binary)).stdout, expected);
    }
}
