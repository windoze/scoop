use super::*;

#[test]
fn interface_queries_return_distinct_empty_and_exact_entries() {
    let workspace = workspace_root();
    let runtime = workspace.join("runtime/src/rt.c");
    let output = compile_and_run_configuration(
        &workspace,
        "interface_query_test",
        "runtime/tests/interface_query_test.c",
        false,
        false,
        &["-O2", runtime.to_str().expect("runtime source path")],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"interface queries preserve empty and exact entries\n"
    );
}
