use super::*;

#[test]
fn captured_default_references_match_the_exported_body() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-cli-default-captures/closure-references/provider/src/main.scoop"
    ));
    source_dispatch::with_hir_source(source, |output, _| {
        let templates =
            hir::CanonicalExportDefaultTemplatesV1::from_dependency_hir(output).unwrap();
        assert_eq!(templates.records().len(), 3);
        for template in templates.records() {
            template
                .validate_reference_closure(&scoop_wire::WirePath::root())
                .unwrap_or_else(|error| panic!("{:?}: {error}", template.key()));
        }
    });
}
