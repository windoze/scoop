use super::*;

#[test]
fn default_source_origin_walk_stops_at_callback_failure() {
    with_hir_source(ORIGINS, |output, _| {
        let production = Production::from_dependency_hir(output).unwrap();
        let mut visits = 0;
        let result = production.templates().visit_definition_sources(
            &mut |_, _, site, _| -> Result<(), Box<dyn std::error::Error>> {
                assert!(matches!(site, Site::Root));
                visits += 1;
                Err("stop".into())
            },
            &WirePath::root(),
        );
        assert_eq!(result.unwrap_err().to_string(), "stop");
        assert_eq!(visits, 1);
    });
}
