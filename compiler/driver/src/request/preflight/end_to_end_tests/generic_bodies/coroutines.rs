use super::*;

mod rebuilt_core;

#[test]
fn imported_coroutine_lifecycle_republishes_and_executes() {
    super::members::check_fixture_cases(
        "m23-coroutines",
        &["lifecycle", "closure-finally"],
        &[],
        "downstream",
    );
}
