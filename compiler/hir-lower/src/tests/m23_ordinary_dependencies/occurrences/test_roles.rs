use scoop_identity::ConeCoordinate;

use super::support::with_test_dependencies;

#[test]
fn m43_production_reexport_remains_usable_when_origin_is_a_test_dependency() {
    let origin = ConeCoordinate::new("test", "occurrence-provider", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let facade = ConeCoordinate::new("test", "occurrence-facade", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    for prefix in [
        "package test.facade.api",
        "import test.facade.api.run",
        "import test.facade.api.*",
    ] {
        with_test_dependencies(
            &format!("{prefix}\npublic fun production(): Boolean = run()"),
            &[origin],
            |output, _| {
                let calls = output.committed_dependency_call_occurrences().unwrap();
                assert_eq!(calls.len(), 1);
                let binding = calls[0].binding().unwrap();
                assert_eq!(binding.source_count(), 1);
                assert_eq!(
                    binding
                        .sources()
                        .next()
                        .unwrap()
                        .route()
                        .immediate_provider(),
                    facade
                );
                assert_eq!(
                    output
                        .imported_dependencies()
                        .callables()
                        .next()
                        .unwrap()
                        .provider(),
                    origin
                );
            },
        );
    }
}
