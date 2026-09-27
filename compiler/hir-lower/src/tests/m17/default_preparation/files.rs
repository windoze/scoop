use super::*;

#[test]
fn forward_default_preparation_restores_the_definition_file_and_origin() {
    let first = "fun first(value: Int = second()): Int = value\nfun main() { first() }";
    let second =
        "internal fun helper(): Int = 19\ninternal fun second(value: Int = helper()): Int = value";
    for files in [[first, second], [second, first]] {
        let core = complete_core_file();
        let parsed = identified_test_sources(
            files
                .iter()
                .map(|source| scoop_parser::parse(source).unwrap())
                .collect(),
        );
        let sources = DefinedTestSources::try_new(
            vec![ProviderSource {
                source: &core,
                identity: core_source_identity("src/core.scoop"),
                provider: hir::IntrinsicProviderId::from_raw(0),
                name: "<core>",
                source_text: "",
            }],
            hir::IntrinsicProviderId::from_raw(1),
            parsed,
            |_| CurrentSourceDetails {
                display_locator: "<user>",
                source_text: "",
            },
        )
        .unwrap();
        let output = lower_defined_for_test(
            scoop_identity::RequestedConeKind::Executable,
            &sources,
            IntrinsicDeclarationPolicy::CoreOnly,
        )
        .unwrap();
        let export = output.export.module();
        let (first_id, _) = export
            .functions
            .iter()
            .find(|(_, f)| f.name == "first")
            .unwrap();
        let (second_id, _) = export
            .functions
            .iter()
            .find(|(_, f)| f.name == "second")
            .unwrap();
        for (function, expected_file) in [
            (first_id, if files[0] == first { 1 } else { 2 }),
            (second_id, if files[0] == first { 2 } else { 1 }),
        ] {
            let interface = export
                .source_parameter_interfaces
                .iter()
                .find(|p| p.owner == hir::ExportParameterOwner::Function(function))
                .unwrap();
            let hir::ExportParameterCalling::Default { source, .. } =
                interface.parameters[0].calling
            else {
                panic!("default");
            };
            let body = &export.export_default_exprs
                [export.export_default_sources[source].declared().unwrap().0];
            assert_eq!(body.origin.file, expected_file);
            assert_eq!(interface.parameters[0].origin.file, expected_file);
            assert_eq!(
                body.definition_root,
                hir::LexicalDefinitionRoot::Function(function)
            );
            assert_eq!(body.definition_path.segments().len(), 1);
            if function == first_id {
                let helper = body
                    .references
                    .callables
                    .iter()
                    .find(|r| r.origin.file != expected_file);
                assert!(
                    helper.is_some(),
                    "the inlined nested default keeps the provider file"
                );
            }
        }
    }
}
