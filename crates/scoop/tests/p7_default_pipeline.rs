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

fn assert_same_observable(default: &Output, explicit: &Output, context: &str) {
    assert_eq!(
        default.status.success(),
        explicit.status.success(),
        "{context}: default/refactor success differed\ndefault={default:?}\nexplicit={explicit:?}"
    );
    assert_eq!(
        String::from_utf8_lossy(&default.stdout),
        String::from_utf8_lossy(&explicit.stdout),
        "{context}: default/refactor stdout differed"
    );
    assert_eq!(
        String::from_utf8_lossy(&default.stderr),
        String::from_utf8_lossy(&explicit.stderr),
        "{context}: default/refactor stderr differed"
    );
}

#[test]
fn default_pipeline_matches_explicit_refactor_dump_mir_cli() {
    let fixture = workspace_path("tests/fixtures/mir/handle_perform.scoop");

    let default = run_scoop([OsStr::new("dump-mir"), fixture.as_os_str()]);
    let explicit = run_scoop([
        OsStr::new("--effect-pipeline"),
        OsStr::new("refactor"),
        OsStr::new("dump-mir"),
        fixture.as_os_str(),
    ]);

    assert_same_observable(&default, &explicit, "dump-mir");
    assert!(default.status.success(), "dump-mir failed: {default:?}");
    assert!(
        String::from_utf8_lossy(&default.stdout).contains("site_id: site"),
        "default dump-mir should expose refactor MIR site metadata"
    );
}

#[test]
fn default_pipeline_matches_explicit_refactor_build_emit_llvm_cli() {
    let fixture = workspace_path("tests/fixtures/build/emit_llvm_basic.scoop");
    let dir = tempdir().unwrap();
    let default_ll = dir.path().join("default.ll");
    let explicit_ll = dir.path().join("explicit_refactor.ll");

    let default = run_scoop([
        OsStr::new("build"),
        OsStr::new("--emit-llvm"),
        OsStr::new("--no-incremental"),
        fixture.as_os_str(),
        OsStr::new("-o"),
        default_ll.as_os_str(),
    ]);
    let explicit = run_scoop([
        OsStr::new("--effect-pipeline"),
        OsStr::new("refactor"),
        OsStr::new("build"),
        OsStr::new("--emit-llvm"),
        OsStr::new("--no-incremental"),
        fixture.as_os_str(),
        OsStr::new("-o"),
        explicit_ll.as_os_str(),
    ]);

    assert_same_observable(&default, &explicit, "build --emit-llvm");
    assert!(
        default.status.success(),
        "default build failed: {default:?}"
    );
    let default_ir = std::fs::read_to_string(&default_ll).unwrap();
    let explicit_ir = std::fs::read_to_string(&explicit_ll).unwrap();
    assert_eq!(default_ir, explicit_ir, "default/refactor LLVM IR differed");
    assert!(default_ir.contains("define i32 @main("));
}

#[test]
fn default_pipeline_matches_explicit_refactor_run_cli() {
    let fixture = workspace_path(
        "tests/fixtures/run-pass/continuation_resume_surface_named_tuple_and_unit_basic.scoop",
    );

    let default = run_scoop([
        OsStr::new("run"),
        OsStr::new("--no-incremental"),
        fixture.as_os_str(),
    ]);
    let explicit = run_scoop([
        OsStr::new("--effect-pipeline"),
        OsStr::new("refactor"),
        OsStr::new("run"),
        OsStr::new("--no-incremental"),
        fixture.as_os_str(),
    ]);

    assert_same_observable(&default, &explicit, "run");
    assert!(default.status.success(), "default run failed: {default:?}");
    assert!(String::from_utf8_lossy(&default.stdout).contains("done"));
}

#[test]
fn default_refactor_runs_async_await_task_resume_payload_cli() {
    let fixture = workspace_path("tests/fixtures/run-pass/async_await_minimal_int_basic.scoop");

    let output = run_scoop([
        OsStr::new("run"),
        OsStr::new("--no-incremental"),
        fixture.as_os_str(),
    ]);

    assert!(
        output.status.success(),
        "default refactor async/await run should complete: {output:?}"
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "before\nafter\n41\ndone\n42\n"
    );
}

#[test]
fn default_pipeline_matches_explicit_refactor_test_fixtures_cli() {
    let fixture = workspace_path("tests/fixtures/build/emit_llvm_basic.scoop");

    let default = run_scoop([
        OsStr::new("test"),
        OsStr::new("--fixtures"),
        fixture.as_os_str(),
    ]);
    let explicit = run_scoop([
        OsStr::new("--effect-pipeline"),
        OsStr::new("refactor"),
        OsStr::new("test"),
        OsStr::new("--fixtures"),
        fixture.as_os_str(),
    ]);

    assert_same_observable(&default, &explicit, "test --fixtures");
    assert!(
        default.status.success(),
        "default fixture run failed: {default:?}"
    );
    assert!(String::from_utf8_lossy(&default.stdout).contains("fixtures: ok (1)"));
}

#[test]
fn no_hidden_legacy_fallback_for_default_refactor_fixture_harness() {
    let fixture =
        workspace_path("tests/fixtures/build/effect_refactor_no_legacy_handler_stack_calls.scoop");
    let output = run_scoop([
        OsStr::new("test"),
        OsStr::new("--fixtures"),
        fixture.as_os_str(),
    ]);

    assert!(
        output.status.success(),
        "default fixture harness should run the refactor-only build fixture without an explicit selector: {output:?}"
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("fixtures: ok (1)"));
}
