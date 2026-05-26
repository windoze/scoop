use std::path::PathBuf;

fn workspace_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn read_workspace_file(relative: &str) -> String {
    std::fs::read_to_string(workspace_path(relative))
        .unwrap_or_else(|err| panic!("failed to read {relative}: {err}"))
}

#[test]
fn legacy_pipeline_docs_removed_live_docs_omit_pipeline_selector_commands() {
    for relative in [
        "docs/archive/designs/EFFECT_REFACTOR.md",
        "docs/archive/designs/HIR_COMPLETENESS_HANDOFF.md",
        "docs/archive/designs/MIR_REFACTOR_PHASE_EXIT_AUDIT.md",
    ] {
        let text = read_workspace_file(relative);
        for needle in ["--effect-pipeline legacy", "--effect-pipeline refactor"] {
            assert!(
                !text.contains(needle),
                "live doc {relative} should not advertise removed pipeline selector text: {needle}"
            );
        }
    }
}

#[test]
fn legacy_pipeline_docs_removed_spec_and_tool_indexes_drop_deleted_async_task_surface() {
    for (relative, needles) in [
        (
            "docs/spec/language_spec-part1.md",
            vec![
                "第 4 部分：效果系统、异常语法糖与 async/await",
                "handle with perform try catch finally async await",
                "async fun",
                "Task<T>",
            ],
        ),
        (
            "docs/spec/language_spec-part3.md",
            vec![
                "`if`、`when`、`try`、`handle`、`do`、`async` 都是表达式",
                "`async {}`",
                "| prefix | `!`, unary `-`, `~`, `await`, `perform` |",
            ],
        ),
        (
            "docs/spec/language_spec-part4.md",
            vec!["Async.await", "Task<", "async fun"],
        ),
        (
            "tools/scoop_tools/src/fixtures_matrix.rs",
            vec!["Task / Executor (async)", "std_task_"],
        ),
    ] {
        let text = read_workspace_file(relative);
        for needle in needles {
            assert!(
                !text.contains(needle),
                "live doc/tool index {relative} should not expose removed async/task surface text: {needle}"
            );
        }
    }
}

#[test]
fn fixture_contract_docs_freeze_external_command_surfaces() {
    let text = read_workspace_file("docs/fixtures.md");
    let required = [
        "## External compiler and driver command contracts",
        "`scoopc dump-ast <path>`",
        "`scoopc dump-hir <path>`",
        "`scoopc dump-mir <path>`",
        "`scoopc dump-ir <path>`",
        "`scoopc dump-effect-facts <path>`",
        "`scoopc dump-effect-lowered <path>`",
        "`scoopc dump-rtti <path> [--type <TYPE>]`",
        "`scoopc dump-stackmaps [--verify-roots] [--dump-records] <path>`",
        "`scoopc emit-artifact --kind {llvm-ir,obj,asm}",
        "`scoopc build-single-cone --cone-root <dir>",
        "`scoopc link-cone --kind <bin\\|lib\\|syslib>",
        "`scoop build <file-or-cone-dir>",
        "`scoop run [<file-or-cone-dir>]",
        "Success stdout",
        "Success stderr",
        "External runner contract",
    ];

    for needle in required {
        assert!(
            text.contains(needle),
            "fixture contract docs should freeze external command surface: {needle}"
        );
    }
}
