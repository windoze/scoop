use super::*;

#[test]
fn relocation_coverage_is_not_emitted_without_diagnostic_support() {
    let workspace = workspace_root();
    let object = std::env::temp_dir().join(format!(
        "scoop_relocation_coverage_{}.o",
        std::process::id()
    ));
    for (stress, verify) in [(false, false), (true, false), (false, true)] {
        let output = std::process::Command::new("cc")
            .args([
                "-std=c11",
                "-D_POSIX_C_SOURCE=200809L",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-O0",
            ])
            .arg(format!("-DSCOOP_ENABLE_GC_STRESS={}", u8::from(stress)))
            .arg(format!("-DSCOOP_GC_VERIFY_HEAP={}", u8::from(verify)))
            .arg("-I")
            .arg(workspace.join("runtime/include"))
            .arg("-c")
            .arg(workspace.join("runtime/src/gc/evacuation.c"))
            .arg("-o")
            .arg(&object)
            .output()
            .expect("compile relocation coverage object");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let bytes = std::fs::read(&object).expect("read unstripped native object");
        for symbol in [
            "scoop_heap_claim_current_object",
            "scoop_gc_object_was_scanned_locked",
            "scoop_gc_visit_current_objects_locked",
        ] {
            assert_eq!(
                bytes
                    .windows(symbol.len())
                    .any(|window| window == symbol.as_bytes()),
                stress || verify,
                "{symbol}: stress={stress}, verify={verify}"
            );
        }
    }
    std::fs::remove_file(object).expect("remove relocation coverage object");
}

#[test]
fn stable_references_and_heap_object_starts_follow_runtime_configuration() {
    let optimized = ["-O2", "-DNDEBUG"];
    for (name, verify, stress, flags) in [
        ("stable_regular", false, false, optimized.as_slice()),
        ("stable_debug", false, false, &[]),
        ("stable_stress", false, true, optimized.as_slice()),
        ("stable_verify", true, false, optimized.as_slice()),
    ] {
        let output = compile_and_run_configuration(
            &workspace_root(),
            name,
            "runtime/tests/stable_reference_test.c",
            verify,
            stress,
            flags,
        );
        assert!(
            output.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.stdout,
            b"stable references and heap object starts passed\n"
        );
    }
}

#[test]
fn stress_environment_requires_explicit_runtime_support() {
    for enabled in [false, true] {
        let output = compile_and_run_configuration(
            &workspace_root(),
            if enabled {
                "stress_enabled"
            } else {
                "stress_disabled"
            },
            "runtime/tests/stress_configuration_test.c",
            false,
            enabled,
            &[],
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"stress configuration passed\n");
    }
}
