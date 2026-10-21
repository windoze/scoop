use std::path::Path;
use std::process::{Command, Output};

use crate::tests::platform_support::{native_float_environment_source, native_os_source};

pub(super) fn runtime_link_flags() -> &'static [&'static str] {
    if cfg!(target_os = "macos") {
        &["-Wl,-dead_strip"]
    } else {
        &[
            "-ffunction-sections",
            "-fdata-sections",
            "-Wl,--gc-sections",
            "-lm",
        ]
    }
}

pub(super) fn compile_and_run(
    workspace: &Path,
    test_name: &str,
    test_source: &str,
    verify_heap: bool,
) -> Output {
    compile_and_run_with_flags(workspace, test_name, test_source, verify_heap, &[])
}

pub(super) fn compile_and_run_with_flags(
    workspace: &Path,
    test_name: &str,
    test_source: &str,
    verify_heap: bool,
    flags: &[&str],
) -> Output {
    compile_and_run_configuration(workspace, test_name, test_source, verify_heap, true, flags)
}

pub(super) fn compile_and_run_configuration(
    workspace: &Path,
    test_name: &str,
    test_source: &str,
    verify_heap: bool,
    enable_gc_stress: bool,
    flags: &[&str],
) -> Output {
    let binary = std::env::temp_dir().join(format!("scoop_{test_name}_{}", std::process::id()));
    let mut compile = Command::new("cc");
    compile.arg(if enable_gc_stress {
        "-DSCOOP_ENABLE_GC_STRESS=1"
    } else {
        "-DSCOOP_ENABLE_GC_STRESS=0"
    });
    if verify_heap {
        compile.arg("-DSCOOP_GC_VERIFY_HEAP=1");
    }
    compile
        .args(flags)
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
        .arg(workspace.join("sysroot/vendor/ryu"))
        .arg("-I")
        .arg(workspace.join("runtime/include"));
    for source in [
        "runtime/src/image/lookup.c",
        "runtime/src/image/active.c",
        "runtime/tests/platform/image_fixture.c",
        "runtime/tests/platform/stackmap_fixture.c",
        "runtime/src/startup/failure.c",
        "runtime/src/process.c",
        "runtime/src/startup/gateway.c",
        "runtime/src/boxing.c",
        "runtime/src/arrays.c",
        "runtime/src/task_context.c",
        "runtime/src/value_shape.c",
        "runtime/src/gc/allocation.c",
        "runtime/src/gc/mutator.c",
        "runtime/src/gc_binding/threads.c",
        "runtime/src/gc_binding/values.c",
        "runtime/src/gc/collector.c",
        "runtime/src/gc_binding/entries.c",
        "runtime/src/gc_binding/pressure.c",
        "runtime/src/gc/pressure.c",
        "runtime/src/gc/mark.c",
        "runtime/src/gc/mark_objects.c",
        "runtime/src/gc/mark_queue.c",
        "runtime/src/gc/mark_pool.c",
        "runtime/src/gc/update.c",
        "runtime/src/gc/time.c",
        "runtime/src/gc_binding/collector_roots.c",
        "runtime/src/gc_binding/scan.c",
        "runtime/src/gc/remembered.c",
        "runtime/src/gc/evacuation_plan.c",
        "runtime/src/gc/evacuation_sources.c",
        "runtime/src/gc/statistics.c",
        "runtime/src/gc/evacuation.c",
        "runtime/src/gc/reclamation.c",
        "runtime/src/gc/block_release.c",
        "runtime/src/gc/block_sweep.c",
        "runtime/src/gc/memory_return.c",
        "runtime/src/gc/heap.c",
        "runtime/src/gc/regions.c",
        "runtime/src/gc/page_map.c",
        "runtime/src/gc/cards.c",
        "runtime/src/gc/heap_objects.c",
        "runtime/src/gc/handles.c",
        "runtime/src/gc_binding/root_frames.c",
        "runtime/src/gc_binding/roots.c",
        "runtime/src/gc_binding/stackmap.c",
        "runtime/src/gc_binding/stackmap/parser.c",
        "runtime/src/gc_binding/stackmap/records.c",
        "runtime/src/gc_binding/stack_roots.c",
        "runtime/src/thread.c",
        // Discard unused formatter entries in this collector-only runtime subset.
        "runtime/src/floating.c",
        native_float_environment_source(),
        "runtime/src/thread/collection.c",
        "runtime/src/thread/debug.c",
        "runtime/src/thread/roots.c",
        "runtime/src/thread/transitions.c",
        "runtime/src/platform/arch/aarch64.c",
        native_os_source(),
        "runtime/tests/platform/fake.c",
        test_source,
    ] {
        compile.arg(workspace.join(source));
    }
    let compile_output = compile
        .args(runtime_link_flags())
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
        .env_remove("SCOOP_GC_STRESS_MINOR")
        .env_remove("SCOOP_GC_STATS")
        .env_remove("SCOOP_GC_FULL_ONLY")
        .env_remove("SCOOP_GC_WORKERS")
        .output()
        .expect("run fake-platform moving collector test");
    std::fs::remove_file(&binary).ok();
    output
}
