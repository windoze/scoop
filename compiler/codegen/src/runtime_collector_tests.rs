use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("codegen crate is nested below the workspace root")
        .to_path_buf()
}

fn compile_and_run(workspace: &Path, test_name: &str, test_source: &str) -> Output {
    let binary = std::env::temp_dir().join(format!("scoop_{test_name}_{}", std::process::id()));
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
        "runtime/src/gc/heap_objects.c",
        "runtime/src/gc/handles.c",
        "runtime/src/gc/root_frames.c",
        "runtime/src/gc/roots.c",
        "runtime/src/gc/stackmap.c",
        "runtime/src/gc/stack_roots.c",
        "runtime/src/thread.c",
        "runtime/src/thread/collection.c",
        "runtime/src/thread/debug.c",
        "runtime/src/thread/roots.c",
        "runtime/src/thread/transitions.c",
        "runtime/src/platform/arch/aarch64.c",
        "runtime/src/platform/os/darwin.c",
        "runtime/tests/platform/fake.c",
        test_source,
    ] {
        compile.arg(workspace.join(source));
    }
    let compile_output = compile
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("compile fake-platform moving collector test");
    assert!(
        compile_output.status.success(),
        "fake-platform {test_name} test must compile cleanly:\n{}",
        String::from_utf8_lossy(&compile_output.stderr)
    );

    let output = Command::new(&binary)
        .env_remove("SCOOP_GC_STRESS_MOVE")
        .output()
        .expect("run fake-platform moving collector test");
    std::fs::remove_file(&binary).ok();
    output
}

#[test]
fn fake_platform_drives_the_real_moving_collector() {
    let workspace = workspace_root();
    let output = compile_and_run(
        &workspace,
        "moving_gc_test",
        "runtime/tests/moving_collector_test.c",
    );
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

#[test]
fn moving_collector_updates_all_thread_protocol_roots() {
    let workspace = workspace_root();
    let output = compile_and_run(
        &workspace,
        "moving_thread_protocol_test",
        "runtime/tests/moving_thread_protocol_test.c",
    );
    assert!(
        output.status.success(),
        "fake-platform moving thread-protocol test failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"moving collector thread-protocol tests passed\n"
    );
}

#[test]
fn initialization_coordinator_is_exactly_once_and_gc_cooperative() {
    let workspace = workspace_root();
    let binary = std::env::temp_dir().join(format!(
        "scoop_initialization_coordinator_test_{}",
        std::process::id()
    ));
    let output = Command::new("cc")
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
        .arg(workspace.join("runtime/include"))
        .arg(workspace.join("runtime/src/thread.c"))
        .arg(workspace.join("runtime/src/thread/collection.c"))
        .arg(workspace.join("runtime/src/thread/transitions.c"))
        .arg(workspace.join("runtime/src/platform/arch/aarch64.c"))
        .arg(workspace.join("runtime/src/platform/os/darwin.c"))
        .arg(workspace.join("runtime/tests/platform/fake.c"))
        .arg(workspace.join("runtime/tests/initialization_coordinator_test.c"))
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("compile initialization coordinator test");
    assert!(
        output.status.success(),
        "initialization coordinator test must compile cleanly:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(&binary)
        .output()
        .expect("run initialization coordinator test");
    std::fs::remove_file(&binary).ok();
    assert!(
        output.status.success(),
        "initialization coordinator test failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"initialization coordinator tests passed\n");
}

#[test]
fn generic_runtime_has_no_target_specific_vm_dependency() {
    let workspace = workspace_root();
    let generic_sources = [
        "runtime/src/gc/allocation.c",
        "runtime/src/gc/collector.c",
        "runtime/src/gc/evacuation.c",
        "runtime/src/gc/reclamation.c",
        "runtime/src/gc/heap.c",
        "runtime/src/gc/heap_objects.c",
        "runtime/src/gc/handles.c",
        "runtime/src/gc/root_frames.c",
        "runtime/src/gc/roots.c",
        "runtime/src/gc/stackmap.c",
        "runtime/src/gc/stack_roots.c",
        "runtime/src/thread.c",
        "runtime/src/thread/collection.c",
        "runtime/src/thread/debug.c",
        "runtime/src/thread/roots.c",
        "runtime/src/thread/transitions.c",
    ];
    let forbidden = [
        "<mach-o/",
        "<sys/mman.h>",
        "__APPLE__",
        "__aarch64__",
        "mmap(",
        "mprotect(",
    ];
    for source in generic_sources {
        let contents = std::fs::read_to_string(workspace.join(source))
            .unwrap_or_else(|error| panic!("read {source}: {error}"));
        for token in forbidden {
            assert!(
                !contents.contains(token),
                "generic runtime source {source} imports target VM detail {token:?}"
            );
        }
    }
}
