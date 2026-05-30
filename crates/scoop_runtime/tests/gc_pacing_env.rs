// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

#[cfg(feature = "gc-immix")]
mod immix {
    use std::process::Command;

    fn heap_growth_command() -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_gc_microbench"));
        cmd.arg("heap-growth")
            .arg("--allocations")
            .arg("200000")
            .arg("--sample-every")
            .arg("50000")
            .arg("--json")
            .env_remove("SCOOP_GC_PACING")
            .env_remove("SCOOP_GC_HEAP_TARGET_GROWTH_FACTOR")
            .env_remove("SCOOP_GC_HEAP_MIN_THRESHOLD_BYTES")
            .env_remove("SCOOP_GC_STRESS");
        cmd
    }

    fn run_heap_growth(mut cmd: Command) -> String {
        let output = cmd.output().expect("gc_microbench must execute");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "gc_microbench failed\nstatus={:?}\nstdout={}\nstderr={}",
            output.status,
            stdout,
            stderr
        );
        stdout.into_owned()
    }

    fn json_u64(output: &str, key: &str) -> u64 {
        let needle = format!("\"{key}\":");
        let start = output
            .find(&needle)
            .unwrap_or_else(|| panic!("missing JSON key `{key}` in {output}"))
            + needle.len();
        let rest = &output[start..];
        let end = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        rest[..end].parse::<u64>().unwrap_or_else(|err| {
            panic!("invalid JSON integer for `{key}`: {err}; output={output}")
        })
    }

    #[test]
    fn pacing_default_on_bounds_heap_and_off_still_uses_block_pool_hard_trigger() {
        let on = run_heap_growth(heap_growth_command());
        let on_bytes = json_u64(&on, "bytes");
        let on_peak_live = json_u64(&on, "peak_live");
        let on_peak_reserved = json_u64(&on, "peak_reserved");
        assert_eq!(on_bytes, 6_400_000);
        assert!(
            on_peak_live <= 5 * 1024 * 1024,
            "default pacing should keep live heap bounded; output={on}"
        );
        assert!(
            on_peak_reserved <= 8 * 1024 * 1024,
            "default pacing should keep reserved heap bounded; output={on}"
        );

        let mut off_cmd = heap_growth_command();
        // P7-T02 why: this is the explicit Immix opt-out regression; `PACING=off` disables only
        // the soft threshold while block-pool exhaustion must still full-GC before growing.
        off_cmd.env("SCOOP_GC_PACING", "off");
        let off = run_heap_growth(off_cmd);
        let off_bytes = json_u64(&off, "bytes");
        let off_peak_live = json_u64(&off, "peak_live");
        let off_peak_reserved = json_u64(&off, "peak_reserved");
        assert_eq!(off_bytes, on_bytes);
        assert!(
            off_peak_live < off_bytes,
            "PACING=off should still allow the block-pool hard trigger to collect; output={off}"
        );
        assert!(
            off_peak_reserved <= 8 * 1024 * 1024,
            "block-pool hard trigger should keep reserved heap bounded even with PACING=off; output={off}"
        );
    }

    #[test]
    fn pacing_min_threshold_and_growth_factor_env_bound_heap() {
        let mut cmd = heap_growth_command();
        cmd.arg("--allocations")
            .arg("10000")
            .arg("--sample-every")
            .arg("1000")
            .env("SCOOP_GC_HEAP_MIN_THRESHOLD_BYTES", "65536")
            .env("SCOOP_GC_HEAP_TARGET_GROWTH_FACTOR", "1.25");
        let out = run_heap_growth(cmd);
        let bytes = json_u64(&out, "bytes");
        let peak_live = json_u64(&out, "peak_live");
        assert_eq!(bytes, 320_000);
        assert!(
            peak_live <= 160 * 1024,
            "small pacing threshold should collect well before full growth; output={out}"
        );
    }

    #[test]
    fn gc_stress_bypasses_pacing_threshold() {
        let mut cmd = heap_growth_command();
        cmd.arg("--allocations")
            .arg("10000")
            .arg("--sample-every")
            .arg("1000")
            .env("SCOOP_GC_HEAP_MIN_THRESHOLD_BYTES", "65536")
            .env("SCOOP_GC_HEAP_TARGET_GROWTH_FACTOR", "1.25")
            .env("SCOOP_GC_STRESS", "1000000");
        let out = run_heap_growth(cmd);
        let bytes = json_u64(&out, "bytes");
        let peak_live = json_u64(&out, "peak_live");
        assert_eq!(bytes, 320_000);
        assert!(
            peak_live > 160 * 1024,
            "active stress should bypass the soft pacing threshold; output={out}"
        );
        assert!(
            peak_live < bytes,
            "block-pool hard trigger should still collect with stress enabled; output={out}"
        );
    }
}

#[cfg(any(feature = "gc-minimal", feature = "gc-hosted"))]
mod non_immix {
    use std::process::Command;

    fn heap_growth_command() -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_gc_microbench"));
        cmd.arg("heap-growth")
            .arg("--json")
            .env_remove("SCOOP_GC_PACING")
            .env_remove("SCOOP_GC_HEAP_TARGET_GROWTH_FACTOR")
            .env_remove("SCOOP_GC_HEAP_MIN_THRESHOLD_BYTES")
            .env_remove("SCOOP_GC_STRESS");
        cmd
    }

    fn run_heap_growth(mut cmd: Command) -> String {
        let output = cmd.output().expect("gc_microbench must execute");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "gc_microbench failed\nstatus={:?}\nstdout={}\nstderr={}",
            output.status,
            stdout,
            stderr
        );
        stdout.into_owned()
    }

    fn json_u64(output: &str, key: &str) -> u64 {
        let needle = format!("\"{key}\":");
        let start = output
            .find(&needle)
            .unwrap_or_else(|| panic!("missing JSON key `{key}` in {output}"))
            + needle.len();
        let rest = &output[start..];
        let end = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        rest[..end].parse::<u64>().unwrap_or_else(|err| {
            panic!("invalid JSON integer for `{key}`: {err}; output={output}")
        })
    }

    #[test]
    fn pacing_default_on_bounds_heap_and_off_keeps_unbounded_soft_trigger_behavior() {
        let mut on_cmd = heap_growth_command();
        on_cmd
            .arg("--allocations")
            .arg("200000")
            .arg("--sample-every")
            .arg("50000");
        let on = run_heap_growth(on_cmd);
        let on_bytes = json_u64(&on, "bytes");
        let on_peak_live = json_u64(&on, "peak_live");
        let on_peak_reserved = json_u64(&on, "peak_reserved");
        assert_eq!(on_bytes, 6_400_000);
        assert!(
            on_peak_live <= 4 * 1024 * 1024,
            "default pacing should keep live heap below the floor threshold; output={on}"
        );
        assert!(
            on_peak_reserved <= 4 * 1024 * 1024,
            "default pacing should keep reserved heap below the floor threshold; output={on}"
        );

        let mut off_cmd = heap_growth_command();
        off_cmd
            .arg("--allocations")
            .arg("200000")
            .arg("--sample-every")
            .arg("50000")
            // P7-T02 why: this is the explicit opt-out regression for non-Immix soft
            // pacing; the assertions below require the old unbounded counters.
            .env("SCOOP_GC_PACING", "off");
        let off = run_heap_growth(off_cmd);
        let off_bytes = json_u64(&off, "bytes");
        let off_peak_live = json_u64(&off, "peak_live");
        let off_peak_reserved = json_u64(&off, "peak_reserved");
        assert_eq!(off_bytes, on_bytes);
        assert_eq!(
            off_peak_live, off_bytes,
            "PACING=off should disable the soft threshold for non-Immix backends; output={off}"
        );
        assert_eq!(
            off_peak_reserved, off_bytes,
            "PACING=off should leave non-Immix reserved bytes unbounded; output={off}"
        );
    }

    #[test]
    fn pacing_min_threshold_and_growth_factor_env_bound_heap() {
        let mut cmd = heap_growth_command();
        cmd.arg("--allocations")
            .arg("10000")
            .arg("--sample-every")
            .arg("1000")
            .env("SCOOP_GC_HEAP_MIN_THRESHOLD_BYTES", "65536")
            .env("SCOOP_GC_HEAP_TARGET_GROWTH_FACTOR", "1.25");
        let out = run_heap_growth(cmd);
        let bytes = json_u64(&out, "bytes");
        let peak_live = json_u64(&out, "peak_live");
        assert_eq!(bytes, 320_000);
        assert!(
            peak_live <= 96 * 1024,
            "small pacing threshold should collect on hosted/minimal backends; output={out}"
        );
    }

    #[test]
    fn gc_stress_bypasses_pacing_threshold() {
        let mut cmd = heap_growth_command();
        cmd.arg("--allocations")
            .arg("10000")
            .arg("--sample-every")
            .arg("1000")
            .env("SCOOP_GC_HEAP_MIN_THRESHOLD_BYTES", "65536")
            .env("SCOOP_GC_HEAP_TARGET_GROWTH_FACTOR", "1.25")
            .env("SCOOP_GC_STRESS", "1000000");
        let out = run_heap_growth(cmd);
        let bytes = json_u64(&out, "bytes");
        let peak_live = json_u64(&out, "peak_live");
        assert_eq!(bytes, 320_000);
        assert_eq!(
            peak_live, bytes,
            "active stress should bypass the soft pacing threshold; output={out}"
        );
    }
}
