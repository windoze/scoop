use scoop_process::CommandExt;
use std::path::Path;
use std::process::Command;

#[test]
fn darwin_page_discard_preserves_one_shot_signal_handlers() {
    let directory = tempfile::tempdir().unwrap();
    let runtime = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime");
    let mut targets = vec![crate::host_target_triple().unwrap()];
    if cfg!(target_arch = "aarch64") {
        let rosetta = Command::new("/usr/bin/arch")
            .args(["-x86_64", "/usr/bin/true"])
            .scoop_output()
            .unwrap();
        if rosetta.status.success() {
            targets.push(
                scoop_target_data::DARWIN_X86_64
                    .contract()
                    .canonical_triple(),
            );
        } else {
            eprintln!("Rosetta is unavailable; testing the native Darwin target");
        }
    }
    for target in targets {
        eprintln!("Testing Darwin VM signal delivery for {target}");
        let binary = directory.path().join(target);
        let output = Command::new("/usr/bin/clang")
            .args([
                "-target", target, "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror",
            ])
            .arg("-I")
            .arg(runtime.join("src"))
            .arg(runtime.join("src/platform/os/darwin.c"))
            .arg(runtime.join("tests/darwin_vm_signal_test.c"))
            .arg("-o")
            .arg(&binary)
            .scoop_output()
            .unwrap();
        assert!(
            output.status.success(),
            "{target}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = Command::new(binary).scoop_output().unwrap();
        assert!(
            output.status.success(),
            "{target}: {:?}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
