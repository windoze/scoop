use super::*;

fn source(case: &str) -> String {
    std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-executable-dependency-callables")
            .join(format!("{case}.scoop")),
    )
    .unwrap()
}

#[test]
fn source_only_default_and_generic_body_do_not_select_machine_callables() {
    for case in ["standalone", "uninstantiated"] {
        let output = lower(&source(case));
        assert_eq!(output.imported_dependencies().callable_count(), 1);
        assert_eq!(
            output
                .output()
                .export
                .module()
                .imported_dependency_callables
                .len(),
            1,
        );
        assert!(output.executable_dependency_callables().unwrap().is_empty());
        assert!(output.concrete_dependency_witness_uses().is_empty());
    }
}

#[test]
fn evaluated_defaults_select_one_callable_without_unused_source_routes() {
    let evaluated = lower(&source("evaluated"));
    assert_eq!(
        evaluated
            .committed_dependency_call_occurrences()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        evaluated.executable_dependency_callables().unwrap().len(),
        1
    );
    assert_eq!(evaluated.concrete_dependency_witness_uses().len(), 1);

    let routes = lower(&source("routes"));
    let executed = routes.executable_dependency_callables().unwrap();
    assert_eq!(executed.len(), 1);
    assert_eq!(executed[0].callable().binding().source_count(), 2);
    let actual = routes.concrete_dependency_witness_uses();
    assert_eq!(actual.len(), 1);
    assert_eq!(actual[0].witness().route().hops().len(), 1);
}
