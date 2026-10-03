use super::*;

mod rebuilt_core;

#[test]
fn imported_coroutine_adapters_republish_and_execute() {
    super::members::check_fixture_cases(
        "m23-coroutines",
        &["value-task", "function-values"],
        &[
            "bad-task-variance",
            "bad-completion-variance",
            "ordinary-suspend-call",
            "mismatched-function-effect",
        ],
        "downstream",
    );
}

#[test]
fn imported_coroutine_lifecycle_republishes_and_executes() {
    super::members::check_fixture_cases(
        "m23-coroutines",
        &["lifecycle", "closure-finally"],
        &[],
        "downstream",
    );
}
