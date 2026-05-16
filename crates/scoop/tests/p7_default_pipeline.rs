#![cfg(all(feature = "llvm", not(windows)))]

use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::{Command, Output};

use tempfile::tempdir;

fn scoop_bin() -> &'static str {
    env!("CARGO_BIN_EXE_scoop")
}

fn workspace_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn run_scoop<I, S>(args: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Command::new(scoop_bin()).args(args).output().unwrap()
}

#[test]
fn effect_pipeline_selector_removed_for_scoop_cli_smoke() {
    let fixture = workspace_path("tests/fixtures/parse/hello.scoop");
    let output = run_scoop([
        OsStr::new("--effect-pipeline"),
        OsStr::new("legacy"),
        OsStr::new("dump-ast"),
        fixture.as_os_str(),
    ]);

    assert!(
        !output.status.success(),
        "removed selector should fail: {output:?}"
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("--effect-pipeline"));
}

#[test]
fn single_effect_pipeline_dump_mir_cli_works() {
    let fixture = workspace_path("tests/fixtures/mir/handle_perform.scoop");
    let output = run_scoop([OsStr::new("dump-mir"), fixture.as_os_str()]);

    assert!(output.status.success(), "dump-mir failed: {output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("site_id: site"));
}

#[test]
fn single_effect_pipeline_build_emit_llvm_cli_works() {
    let fixture = workspace_path("tests/fixtures/build/emit_llvm_basic.scoop");
    let dir = tempdir().unwrap();
    let output_ll = dir.path().join("single.ll");

    let output = run_scoop([
        OsStr::new("build"),
        OsStr::new("--emit-llvm"),
        OsStr::new("--no-incremental"),
        fixture.as_os_str(),
        OsStr::new("-o"),
        output_ll.as_os_str(),
    ]);

    assert!(output.status.success(), "build failed: {output:?}");
    let ir = std::fs::read_to_string(&output_ll).unwrap();
    assert!(ir.contains("define i32 @main("));
}

#[test]
fn single_effect_pipeline_run_cli_works() {
    let fixture = workspace_path(
        "tests/fixtures/run-pass/continuation_resume_surface_named_tuple_and_unit_basic.scoop",
    );

    let output = run_scoop([
        OsStr::new("run"),
        OsStr::new("--no-incremental"),
        fixture.as_os_str(),
    ]);

    assert!(output.status.success(), "run failed: {output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("done"));
}

#[test]
fn single_pipeline_runs_hidden_suspend_dynamic_dispatch_helpers_cli() {
    for (fixture, expected_stdout) in [
        (
            "tests/fixtures/run-pass/effect_handle_hidden_suspend_virtual_helper_basic.scoop",
            "helper_before\nderived\n42\ndone\n",
        ),
        (
            "tests/fixtures/run-pass/effect_handle_hidden_suspend_interface_helper_basic.scoop",
            "helper_before\nimpl\n53\ndone\n",
        ),
    ] {
        let fixture = workspace_path(fixture);
        let output = run_scoop([
            OsStr::new("run"),
            OsStr::new("--no-incremental"),
            fixture.as_os_str(),
        ]);

        assert!(
            output.status.success(),
            "single pipeline dynamic dispatch hidden suspend fixture should run: {output:?}"
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout), expected_stdout);
    }
}

#[test]
fn single_pipeline_runs_higher_order_function_value_handled_effect_cli() {
    let fixture = workspace_path(
        "tests/fixtures/run-pass/effect_indirect_perform_nonresuming_function_value_higher_order_when_direct.scoop",
    );

    let output = run_scoop([
        OsStr::new("run"),
        OsStr::new("--no-incremental"),
        fixture.as_os_str(),
    ]);

    assert_eq!(
        output.status.code(),
        Some(10),
        "single pipeline higher-order function-value fixture should exit 10: {output:?}"
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "5\ncaught\n9\n10\n"
    );
}

#[test]
fn single_pipeline_runs_indirect_perform_closure_resume_cli() {
    let fixture = workspace_path(
        "tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_closure.scoop",
    );

    let output = run_scoop([
        OsStr::new("run"),
        OsStr::new("--no-incremental"),
        fixture.as_os_str(),
    ]);

    assert!(
        output.status.success(),
        "single pipeline closure continuation fixture should run: {output:?}"
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "body_start\nclosure_enter\narm\nresult\n99\nclosure_resume\n32\nbody_done\n42\nafter_resume\n"
    );
}

#[test]
fn single_pipeline_runs_multi_type_param_effect_payload_dispatch_cli() {
    let fixture =
        workspace_path("tests/fixtures/run-pass/effect_multi_type_params_dispatch_basic.scoop");

    let output = run_scoop([
        OsStr::new("run"),
        OsStr::new("--no-incremental"),
        fixture.as_os_str(),
    ]);

    assert!(
        output.status.success(),
        "single pipeline multi type-param effect payload fixture should run: {output:?}"
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "left\n7\nright\n107\n10\n"
    );
}

#[test]
fn single_pipeline_runs_raise_cleanup_gc_cli() {
    let fixture = workspace_path("tests/fixtures/run-pass/effect_raise_cleanup_gc_basic.scoop");

    let output = run_scoop([
        OsStr::new("run"),
        OsStr::new("--no-incremental"),
        fixture.as_os_str(),
    ]);

    assert!(
        output.status.success(),
        "single pipeline raise cleanup GC fixture should run: {output:?}"
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "0\n");
}

#[test]
fn single_pipeline_build_emit_llvm_cli_preserves_target_shape_effect_contract() {
    let fixture =
        workspace_path("tests/fixtures/build/effect_lowered_direct_handle_resume_emit_llvm.scoop");
    let dir = tempdir().unwrap();
    let output_ll = dir.path().join("effect_contract.ll");

    let output = run_scoop([
        OsStr::new("build"),
        OsStr::new("--emit-llvm"),
        OsStr::new("--no-incremental"),
        fixture.as_os_str(),
        OsStr::new("-o"),
        output_ll.as_os_str(),
    ]);

    assert!(
        output.status.success(),
        "single pipeline effect-contract fixture should build IR: {output:?}"
    );
    let ir = std::fs::read_to_string(&output_ll).unwrap();
    assert!(
        ir.contains("ScoopEffectCtx") && ir.contains("ScoopEffectOutcome"),
        "single pipeline CLI IR should keep explicit EffectCtx / EffectOutcome surface: {ir}"
    );
    assert!(
        ir.contains("@__scoop_priv0__lowered_surface_resume__outcome__h")
            && ir.contains("cmpxchg")
            && ir.contains("step_is_complete"),
        "single pipeline CLI IR should keep target-shape Step_F / surface-resume contract: {ir}"
    );
}

#[test]
fn single_pipeline_runs_receiver_effect_op_cli() {
    let fixture = workspace_path("tests/fixtures/run-pass/effect_receiver_op_basic.scoop");

    let output = run_scoop([
        OsStr::new("run"),
        OsStr::new("--no-incremental"),
        fixture.as_os_str(),
    ]);

    assert_eq!(
        output.status.code(),
        Some(30),
        "single pipeline receiver effect op fixture should exit 30: {output:?}"
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "go\n2\ndirect_result\n4\nimmediate_before\nhey\n3\nimmediate_after\n6\nimmediate_result\n16\nescape_before\nz\n4\nafter_escape_handle\nescape_after\n9\nescape_total\n10\n"
    );
}

#[test]
fn single_effect_pipeline_test_fixtures_cli_works() {
    let fixture = workspace_path("tests/fixtures/build/emit_llvm_basic.scoop");
    let output = run_scoop([
        OsStr::new("test"),
        OsStr::new("--fixtures"),
        fixture.as_os_str(),
    ]);

    assert!(output.status.success(), "fixture run failed: {output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("fixtures: ok (1)"));
}

#[test]
fn single_pipeline_fixture_harness_has_no_hidden_legacy_fallback() {
    let fixture =
        workspace_path("tests/fixtures/build/effect_lowered_no_legacy_handler_stack_calls.scoop");
    let output = run_scoop([
        OsStr::new("test"),
        OsStr::new("--fixtures"),
        fixture.as_os_str(),
    ]);

    assert!(
        output.status.success(),
        "single pipeline fixture harness should run the build fixture: {output:?}"
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("fixtures: ok (1)"));
}
