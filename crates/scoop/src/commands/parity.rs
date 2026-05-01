use std::collections::HashMap;
use std::path::{Path, PathBuf};

use clap::Parser as _;

use crate::cli::{Args, Command};
use scoopc::session::SessionOptions;

#[cfg(feature = "llvm")]
use tempfile::tempdir;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParityObservation {
    success: bool,
    stdout: String,
    stderr: String,
}

fn workspace_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn dump_cli_args(mode: &str, subcommand: &str, fixture: &Path) -> Vec<String> {
    vec![
        "scoop".to_string(),
        "--effect-pipeline".to_string(),
        mode.to_string(),
        subcommand.to_string(),
        fixture.display().to_string(),
    ]
}

fn observe_dump_cli(argv: Vec<String>) -> ParityObservation {
    let args = Args::try_parse_from(argv).unwrap();
    let session_options = SessionOptions::new(args.effect_pipeline);
    let result = match args.command {
        Command::DumpAst { input } => super::dump_ast::render_dump_output(input, session_options),
        Command::DumpHir { input } => super::dump_hir::render_dump_output(input, session_options),
        Command::DumpMir { input } => super::dump_mir::render_dump_output(input, session_options),
        Command::DumpIr { input } => super::dump_ir::render_parity_output(input, session_options),
        other => panic!("expected dump command in parity test, got {other:?}"),
    }
    .map(|stdout| canonicalize_debug_type_ids(&stdout));

    match result {
        Ok(stdout) => ParityObservation {
            success: true,
            stdout,
            stderr: String::new(),
        },
        Err(err) => ParityObservation {
            success: false,
            stdout: String::new(),
            stderr: format!("{err:?}"),
        },
    }
}

fn assert_dump_cli_parity(subcommand: &str, fixture: &str) {
    let fixture = workspace_path(fixture);
    let legacy = observe_dump_cli(dump_cli_args("legacy", subcommand, &fixture));
    let refactor = observe_dump_cli(dump_cli_args("refactor", subcommand, &fixture));

    assert_eq!(
        legacy.success, refactor.success,
        "{subcommand} 在 legacy/refactor 下退出状态不一致（fixture: {}）",
        fixture.display()
    );
    assert_eq!(
        legacy.stderr, refactor.stderr,
        "{subcommand} 在 legacy/refactor 下 stderr 不一致（fixture: {}）",
        fixture.display()
    );
    assert_eq!(
        legacy.stdout, refactor.stdout,
        "{subcommand} 在 legacy/refactor 下 dump 输出不一致（fixture: {}）",
        fixture.display()
    );
}

fn canonicalize_debug_type_ids(text: &str) -> String {
    let mut normalized = HashMap::<u32, usize>::new();
    let mut out = String::with_capacity(text.len());
    let mut index = 0;

    while index < text.len() {
        if text[index..].starts_with("TypeId(")
            && let Some((end, raw_id)) = parse_type_id_block(text, index)
        {
            let stable_id = stable_type_id(&mut normalized, raw_id);
            out.push_str("TypeId(T");
            out.push_str(&stable_id.to_string());
            out.push(')');
            index = end;
            continue;
        }

        if text.as_bytes()[index] == b't'
            && let Some((end, raw_id)) = parse_type_token(text, index)
        {
            let stable_id = stable_type_id(&mut normalized, raw_id);
            out.push('T');
            out.push_str(&stable_id.to_string());
            index = end;
            continue;
        }

        let ch = text[index..].chars().next().unwrap();
        out.push(ch);
        index += ch.len_utf8();
    }

    out
}

fn stable_type_id(normalized: &mut HashMap<u32, usize>, raw_id: u32) -> usize {
    let next = normalized.len();
    *normalized.entry(raw_id).or_insert(next)
}

fn parse_type_id_block(text: &str, start: usize) -> Option<(usize, u32)> {
    let bytes = text.as_bytes();
    let mut index = start + "TypeId(".len();
    index = skip_ascii_whitespace(bytes, index);

    let digits_start = index;
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        index += 1;
    }
    if digits_start == index {
        return None;
    }
    let raw_id = text[digits_start..index].parse().ok()?;

    index = skip_ascii_whitespace(bytes, index);
    if bytes.get(index) != Some(&b',') {
        return None;
    }
    index += 1;
    index = skip_ascii_whitespace(bytes, index);
    if bytes.get(index) != Some(&b')') {
        return None;
    }

    Some((index + 1, raw_id))
}

fn parse_type_token(text: &str, start: usize) -> Option<(usize, u32)> {
    if start > 0 {
        let prev = text[..start].chars().next_back().unwrap();
        if !matches!(prev, '[' | '(' | ',' | ':' | ' ' | '\n') {
            return None;
        }
    }

    let bytes = text.as_bytes();
    let mut index = start + 1;
    let digits_start = index;
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        index += 1;
    }
    if digits_start == index {
        return None;
    }

    if let Some(next) = text[index..].chars().next()
        && !matches!(next, ']' | ')' | ',' | ':' | ' ' | '\n')
    {
        return None;
    }

    let raw_id = text[digits_start..index].parse().ok()?;
    Some((index, raw_id))
}

fn skip_ascii_whitespace(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

#[test]
fn canonicalize_debug_type_ids_normalizes_multiline_and_inline_forms() {
    let raw = "TypeId(\n    8,\n) [t8, t3, t8]";
    assert_eq!(
        canonicalize_debug_type_ids(raw),
        "TypeId(T0) [T0, T1, T0]"
    );
}

#[test]
fn dump_ast_cli_parity_matches_legacy_and_refactor() {
    assert_dump_cli_parity("dump-ast", "tests/fixtures/parse/handle_expr_minimal.scoop");
}

#[test]
fn dump_hir_cli_parity_matches_legacy_and_refactor() {
    assert_dump_cli_parity(
        "dump-hir",
        "tests/fixtures/run-pass/continuation_resume_surface_named_tuple_and_unit_basic.scoop",
    );
}

#[test]
fn dump_mir_cli_parity_matches_legacy_and_refactor() {
    assert_dump_cli_parity("dump-mir", "tests/fixtures/mir/handle_perform.scoop");
}

#[test]
fn dump_ir_cli_parity_matches_legacy_and_refactor() {
    assert_dump_cli_parity(
        "dump-ir",
        "tests/fixtures/run-pass/effect_no_perform_handle_elim_basic.scoop",
    );
}

#[cfg(feature = "llvm")]
fn observe_build_emit_llvm_cli(mode: &str, fixture: &Path, output: &Path) -> ParityObservation {
    let _ = std::fs::remove_file(output);
    let args = Args::try_parse_from([
        "scoop",
        "--effect-pipeline",
        mode,
        "build",
        fixture.to_str().unwrap(),
        "--emit-llvm",
        "--no-incremental",
        "-o",
        output.to_str().unwrap(),
    ])
    .unwrap();

    match super::dispatch(args) {
        Ok(()) => ParityObservation {
            success: true,
            stdout: std::fs::read_to_string(output).unwrap(),
            stderr: String::new(),
        },
        Err(err) => ParityObservation {
            success: false,
            stdout: String::new(),
            stderr: format!("{err:?}"),
        },
    }
}

#[cfg(feature = "llvm")]
#[test]
fn build_emit_llvm_cli_parity_matches_legacy_and_refactor() {
    let fixture = workspace_path("tests/fixtures/run-pass/effect_no_perform_handle_elim_basic.scoop");
    let dir = tempdir().unwrap();
    let output = dir.path().join("parity.ll");

    let legacy = observe_build_emit_llvm_cli("legacy", &fixture, &output);
    let refactor = observe_build_emit_llvm_cli("refactor", &fixture, &output);

    assert_eq!(legacy.success, refactor.success, "build --emit-llvm 退出状态不一致");
    assert_eq!(legacy.stderr, refactor.stderr, "build --emit-llvm stderr 不一致");
    assert_eq!(legacy.stdout, refactor.stdout, "build --emit-llvm 产物不一致");
    assert!(
        legacy.stdout.contains("define i32 @main("),
        "LLVM smoke 产物应包含 main 定义"
    );
}
