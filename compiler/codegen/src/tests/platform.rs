#[cfg(target_os = "macos")]
#[test]
fn loaded_image_ranges_use_current_vm_permissions() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap();
    let binary = std::env::temp_dir().join(format!("scoop_image_ranges_{}", std::process::id()));
    let compile = std::process::Command::new("cc")
        .args([
            "-std=c11",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-Wl,-rename_section,__LLVM_STACKMAPS,__llvm_stackmaps,__DATA_CONST,__llvm_stackmaps",
        ])
        .arg(workspace.join("runtime/src/platform/image/macho.c"))
        .arg(workspace.join("runtime/src/image/ranges.c"))
        .arg(workspace.join("runtime/tests/image_ranges_test.c"))
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let output = std::process::Command::new(&binary).output().unwrap();
    std::fs::remove_file(&binary).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"loaded image range tests passed\n");
}

#[test]
fn runtime_stackmap_v3_parser_rejects_incomplete_metadata() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("codegen crate is nested below the workspace root");
    let binary = std::env::temp_dir().join(format!("scoop_stackmap_test_{}", std::process::id()));
    let status = std::process::Command::new("cc")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror"])
        .arg(workspace.join("runtime/src/gc/stackmap.c"))
        .arg(workspace.join("runtime/src/gc/stackmap/parser.c"))
        .arg(workspace.join("runtime/src/gc/stackmap/records.c"))
        .arg(workspace.join("runtime/src/platform/arch/aarch64.c"))
        .arg(workspace.join("runtime/tests/stackmap_test.c"))
        .arg("-o")
        .arg(&binary)
        .status()
        .expect("run C compiler for stackmap parser tests");
    assert!(
        status.success(),
        "stackmap parser tests must compile cleanly"
    );
    let output = std::process::Command::new(&binary)
        .output()
        .expect("run stackmap parser tests");
    std::fs::remove_file(&binary).ok();
    assert!(
        output.status.success(),
        "stackmap parser test failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"stackmap parser tests passed\n");
}

#[test]
fn runtime_walks_and_rewrites_only_exact_stackmap_slots() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("codegen crate is nested below the workspace root");
    let binary =
        std::env::temp_dir().join(format!("scoop_exact_roots_test_{}", std::process::id()));
    let status = std::process::Command::new("cc")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-pthread"])
        .arg("-I")
        .arg(workspace.join("runtime/include"))
        .arg(workspace.join("runtime/src/gc/stackmap.c"))
        .arg(workspace.join("runtime/src/gc/stackmap/parser.c"))
        .arg(workspace.join("runtime/src/gc/stackmap/records.c"))
        .arg(workspace.join("runtime/src/gc/stack_roots.c"))
        .arg(workspace.join("runtime/src/platform/arch/aarch64.c"))
        .arg(workspace.join(super::platform_support::native_os_source()))
        .arg(workspace.join("runtime/tests/platform/fake.c"))
        .arg(workspace.join("runtime/tests/platform/stackmap_fixture.c"))
        .arg(workspace.join("runtime/tests/exact_stack_roots_test.c"))
        .arg("-o")
        .arg(&binary)
        .status()
        .expect("compile exact stack-root test");
    assert!(
        status.success(),
        "exact stack-root test must compile cleanly"
    );
    let output = std::process::Command::new(&binary)
        .output()
        .expect("run exact stack-root test");
    std::fs::remove_file(&binary).ok();
    assert!(
        output.status.success(),
        "exact stack-root test failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"exact stack root walk passed\n");
}

#[test]
fn native_ffi_header_cannot_name_managed_entries() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("codegen crate is nested below the workspace root");
    let output = std::process::Command::new("cc")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-fsyntax-only"])
        .arg("-I")
        .arg(workspace.join("runtime/include"))
        .arg(workspace.join("runtime/tests/ffi_header_rejects_managed_entry.c"))
        .output()
        .expect("compile the invalid native FFI header probe");
    assert!(
        !output.status.success(),
        "the public native FFI header must not declare ManagedEntry functions"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("scoop_rt_gc_collect"),
        "negative header failure must identify the forbidden ManagedEntry:\n{stderr}"
    );
}

#[test]
fn generated_entry_header_exposes_the_compiler_runtime_abi() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("codegen crate is nested below the workspace root");
    let status = std::process::Command::new("cc")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-fsyntax-only"])
        .arg("-I")
        .arg(workspace.join("runtime/include"))
        .arg("-I")
        .arg(workspace.join("runtime/src"))
        .arg(workspace.join("runtime/tests/generated_entry_header_test.c"))
        .status()
        .expect("compile the generated-entry header probe");
    assert!(
        status.success(),
        "the private generated-entry header must describe its complete ABI"
    );
}

#[test]
fn runtime_metadata_v1_header_has_the_frozen_layout() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("codegen crate is nested below the workspace root");
    let binary = std::env::temp_dir().join(format!(
        "scoop_runtime_metadata_v1_layout_test_{}",
        std::process::id()
    ));
    let compile = std::process::Command::new("cc")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror"])
        .arg("-I")
        .arg(workspace.join("runtime/include"))
        .arg(workspace.join("runtime/tests/runtime_metadata_v1_layout_test.c"))
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("compile runtime metadata v1 layout test");
    assert!(
        compile.status.success(),
        "runtime metadata v1 header must match the frozen C ABI:\n{}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let output = std::process::Command::new(&binary)
        .output()
        .expect("run runtime metadata v1 layout test");
    std::fs::remove_file(&binary).ok();
    assert!(
        output.status.success(),
        "runtime metadata v1 constants must match the frozen ABI:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn exception_control_flow_entries_stay_runtime_private() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("codegen crate is nested below the workspace root");
    let public = std::fs::read_to_string(workspace.join("runtime/include/scoop_rt.h"))
        .expect("read public runtime header");
    let generated = std::fs::read_to_string(workspace.join("runtime/src/generated_entries.h"))
        .expect("read generated-entry header");
    let internal = std::fs::read_to_string(workspace.join("runtime/src/eh_internal.h"))
        .expect("read EH-internal header");

    for entry in [
        "scoop_rt_throw(",
        "scoop_rt_rethrow(",
        "scoop_rt_begin_catch(",
        "scoop_rt_end_catch(",
        "scoop_rt_materialize_exception(",
    ] {
        assert!(
            !public.contains(entry),
            "public native FFI header exposes private EH entry {entry}"
        );
        assert!(
            generated.contains(entry),
            "generated-entry header omits private EH entry {entry}"
        );
    }
    assert!(!generated.contains("scoop_eh_personality("));
    assert!(internal.contains("scoop_eh_personality("));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn darwin_aarch64_managed_entries_preserve_the_direct_caller_anchor() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("codegen crate is nested below the workspace root");
    let object = std::env::temp_dir().join(format!(
        "scoop_aarch64_managed_entries_{}.o",
        std::process::id()
    ));
    let status = std::process::Command::new("cc")
        .args(["-c", "-target", "arm64-apple-darwin"])
        .arg(workspace.join("runtime/src/platform/arch/aarch64_anchor.S"))
        .arg("-o")
        .arg(&object)
        .status()
        .expect("assemble Darwin AArch64 managed entries");
    assert!(status.success(), "managed entries must assemble cleanly");

    let disassembly = std::process::Command::new("otool")
        .arg("-tvV")
        .arg(&object)
        .output()
        .expect("disassemble managed entries");
    assert!(disassembly.status.success());
    let text = String::from_utf8_lossy(&disassembly.stdout);
    let entries = [
        (
            "_scoop_rt_safepoint:",
            ["mov\tx0, x30", "mov\tx1, sp", "mov\tx2, x29"],
        ),
        (
            "_scoop_runtime_alloc_slow:",
            ["mov\tx2, x30", "mov\tx3, sp", "mov\tx4, x29"],
        ),
        (
            "_scoop_rt_gc_collect:",
            ["mov\tx0, x30", "mov\tx1, sp", "mov\tx2, x29"],
        ),
        (
            "_scoop_rt_string_concat:",
            ["mov\tx2, x30", "mov\tx3, sp", "mov\tx4, x29"],
        ),
        (
            "_scoop_rt_box_zst:",
            ["mov\tx1, x30", "mov\tx2, sp", "mov\tx3, x29"],
        ),
        (
            "_scoop_rt_box_value:",
            ["mov\tx2, x30", "mov\tx3, sp", "mov\tx4, x29"],
        ),
        (
            "_scoop_rt_materialize_exception:",
            ["mov\tx1, x30", "mov\tx2, sp", "mov\tx3, x29"],
        ),
        (
            "_scoop_rt_init_enter:",
            ["mov\tx1, x30", "mov\tx2, sp", "mov\tx3, x29"],
        ),
        (
            "_scoop_rt_init_succeed:",
            ["mov\tx1, x30", "mov\tx2, sp", "mov\tx3, x29"],
        ),
        (
            "_scoop_rt_init_fail:",
            ["mov\tx2, x30", "mov\tx3, sp", "mov\tx4, x29"],
        ),
        (
            "_scoop_rt_init_failure:",
            ["mov\tx1, x30", "mov\tx2, sp", "mov\tx3, x29"],
        ),
        (
            "_scoop_rt_init_cycle_message:",
            ["mov\tx1, x30", "mov\tx2, sp", "mov\tx3, x29"],
        ),
        (
            "_scoop_rt_array_clone:",
            ["mov\tx3, x30", "mov\tx4, sp", "mov\tx5, x29"],
        ),
        (
            "_scoop_rt_enter_native_safe:",
            ["mov\tx2, x30", "mov\tx3, sp", "mov\tx4, x29"],
        ),
        (
            "_scoop_rt_enter_native_borrowed:",
            ["mov\tx2, x30", "mov\tx3, sp", "mov\tx4, x29"],
        ),
    ];
    let lines: Vec<_> = text.lines().collect();
    for (label, moves) in entries {
        let start = lines
            .iter()
            .position(|line| *line == label)
            .unwrap_or_else(|| panic!("missing {label} in:\n{text}"));
        for (offset, expected) in moves.iter().enumerate() {
            assert!(
                lines[start + offset + 1].ends_with(expected),
                "{label} must preserve the direct caller context:\n{text}"
            );
        }
        assert!(
            lines[start + 4].contains("\tb\t"),
            "{label} must tail-branch without creating a frame:\n{text}"
        );
    }

    let relocations = std::process::Command::new("otool")
        .arg("-rv")
        .arg(&object)
        .output()
        .expect("read managed-entry relocations");
    std::fs::remove_file(&object).ok();
    assert!(relocations.status.success());
    let relocations = String::from_utf8_lossy(&relocations.stdout);
    for implementation in [
        "_scoop_rt_safepoint_impl",
        "_scoop_runtime_alloc_slow_impl",
        "_scoop_rt_gc_collect_impl",
        "_scoop_rt_string_concat_impl",
        "_scoop_rt_box_zst_impl",
        "_scoop_rt_box_value_impl",
        "_scoop_rt_materialize_exception_impl",
        "_scoop_rt_init_enter_impl",
        "_scoop_rt_init_succeed_impl",
        "_scoop_rt_init_fail_impl",
        "_scoop_rt_init_failure_impl",
        "_scoop_rt_init_cycle_message_impl",
        "_scoop_rt_array_clone_impl",
        "_scoop_rt_context_push_impl",
        "_scoop_rt_context_fork_impl",
        "_scoop_rt_context_ensure_root_impl",
        "_scoop_rt_enter_native_safe_impl",
        "_scoop_rt_enter_native_borrowed_impl",
    ] {
        assert!(
            relocations
                .lines()
                .any(|line| { line.contains("BR26") && line.ends_with(implementation) }),
            "missing tail-branch relocation for {implementation}:\n{relocations}"
        );
    }
    assert_eq!(relocations.matches("BR26").count(), 18);
}
