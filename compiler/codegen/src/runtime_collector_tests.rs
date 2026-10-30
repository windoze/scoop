use crate::tests::platform_support::{native_float_environment_source, native_os_source};
use compile::runtime_link_flags;
use compile::{compile_and_run, compile_and_run_configuration, compile_and_run_with_flags};
use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "runtime_collector_tests/compile.rs"]
mod compile;
#[path = "runtime_collector_tests/dispatch.rs"]
mod dispatch;

#[path = "runtime_collector_tests/nursery.rs"]
mod nursery;

#[path = "runtime_collector_tests/regions.rs"]
mod regions;

#[path = "runtime_collector_tests/parallel.rs"]
mod parallel;

#[path = "runtime_collector_tests/pins.rs"]
mod pins;

#[path = "runtime_collector_tests/thread_handshake.rs"]
mod thread_handshake;

#[path = "runtime_collector_tests/stress.rs"]
mod stress;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("codegen crate is nested below the workspace root")
        .to_path_buf()
}

#[test]
fn task_context_radix_and_snapshots_survive_moving_gc() {
    let output = compile_and_run(
        &workspace_root(),
        "task_context_test",
        "runtime/tests/task_context_test.c",
        true,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"task context radix, fork, restore and moving roots passed\n"
    );
}

#[test]
fn every_gateway_handshakes_before_activating_its_first_frame() {
    let output = compile_and_run(
        &workspace_root(),
        "gateway_entry_test",
        "runtime/tests/gateway_entry_test.c",
        false,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"gateway entry and GC handshake tests passed\n"
    );
}

#[test]
fn fake_platform_drives_the_real_moving_collector() {
    let workspace = workspace_root();
    let output = compile_and_run(
        &workspace,
        "moving_gc_test",
        "runtime/tests/moving_collector_test.c",
        false,
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
fn regular_allocations_and_range_barriers_cover_line_and_card_boundaries() {
    let output = compile_and_run(
        &workspace_root(),
        "allocation_barrier_test",
        "runtime/tests/allocation_barrier_test.c",
        false,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"regular allocation and range barrier tests passed\n"
    );
}

#[test]
fn release_hooks_run_once_before_dead_storage_is_retired() {
    let output = compile_and_run(
        &workspace_root(),
        "release_collector_test",
        "runtime/tests/release_collector_test.c",
        false,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"release collector lifecycle tests passed\n");
}

#[test]
fn moving_collector_updates_all_thread_protocol_roots() {
    let workspace = workspace_root();
    let output = compile_and_run(
        &workspace,
        "moving_thread_protocol_test",
        "runtime/tests/moving_thread_protocol_test.c",
        false,
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
fn initialization_waiters_share_success_and_failure_across_moving_gc() {
    let output = compile_and_run(
        &workspace_root(),
        "initialization_gc_test",
        "runtime/tests/initialization_gc_test.c",
        false,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"initialization waiters and moving GC tests passed\n"
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
            "-D_POSIX_C_SOURCE=200809L",
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
        .arg(workspace.join("runtime/src/platform/primitives/posix.c"))
        .arg(workspace.join("runtime/src/gc/mutator.c"))
        .arg(workspace.join(native_float_environment_source()))
        .arg(workspace.join("runtime/src/thread/collection.c"))
        .arg(workspace.join("runtime/src/thread/transitions.c"))
        .arg(workspace.join("runtime/src/platform/frame/darwin_aarch64.c"))
        .arg(workspace.join(native_os_source()))
        .arg(workspace.join("runtime/tests/platform/fake.c"))
        .arg(workspace.join("runtime/tests/initialization_coordinator_test.c"))
        .args(runtime_link_flags())
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
fn dynamic_unboxing_and_native_allocation_preserve_representation() {
    let workspace = workspace_root();
    let strings = workspace.join("runtime/src/strings.c");
    let output = compile_and_run_with_flags(
        &workspace,
        "value_representation_test",
        "runtime/tests/value_representation_test.c",
        false,
        &[strings.to_str().unwrap()],
    );
    assert!(
        output.status.success(),
        "descriptor-driven representation test failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    assert_eq!(
        output.stdout,
        b"descriptor-driven representation tests passed\n"
    );
}

#[test]
fn startup_gateway_statuses_and_failure_reporting_preserve_published_roots() {
    let output = compile_and_run(
        &workspace_root(),
        "startup_gateway_test",
        "runtime/tests/startup_gateway_test.c",
        false,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"startup gateway invariants passed\n");
}
