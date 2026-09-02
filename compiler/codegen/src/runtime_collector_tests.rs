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
fn fake_platform_drives_the_real_moving_collector() {
    let workspace = workspace_root();
    let binary = std::env::temp_dir().join(format!("scoop_moving_gc_test_{}", std::process::id()));
    let mut compile = Command::new("cc");
    compile
        .args([
            "-std=c11",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-pthread",
            "-fno-omit-frame-pointer",
            "-fno-optimize-sibling-calls",
        ])
        .arg("-I")
        .arg(workspace.join("runtime/include"));
    for source in [
        "runtime/src/gc/allocation.c",
        "runtime/src/gc/collector.c",
        "runtime/src/gc/evacuation.c",
        "runtime/src/gc/reclamation.c",
        "runtime/src/gc/heap.c",
        "runtime/src/gc/roots.c",
        "runtime/src/gc/stackmap.c",
        "runtime/src/gc/stack_roots.c",
        "runtime/src/thread.c",
        "runtime/src/platform/arch/aarch64.c",
        "runtime/src/platform/os/darwin.c",
        "runtime/tests/platform/fake.c",
        "runtime/tests/moving_collector_test.c",
    ] {
        compile.arg(workspace.join(source));
    }
    let output = compile
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("compile fake-platform moving collector test");
    assert!(
        output.status.success(),
        "fake-platform moving collector test must compile cleanly:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output = Command::new(&binary)
        .env_remove("SCOOP_GC_STRESS_MOVE")
        .output()
        .expect("run fake-platform moving collector test");
    std::fs::remove_file(&binary).ok();
    assert!(
        output.status.success(),
        "fake-platform moving collector test failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"moving collector fake-platform tests passed\n"
    );
}
