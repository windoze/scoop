use super::*;

#[test]
fn full_template_derived_equality_uses_imported_source_identity() {
    with_provider_consumer_export_ir(
        r#"
        package profile.provider
        public struct Box<T>(val value: T)
        public fun same(left: Box<Int>, right: Box<Int>): Boolean = left == right
        "#,
        r#"
        import profile.provider.*
        public fun answer(): Boolean = same(Box<Int>(1), Box<Int>(1))
        "#,
        hir::IrTemplateExport::Full,
        |output, _, _, _, _| {
            hir::CanonicalHirFoundation::from_dependency_output(&output).unwrap();
        },
    )
    .unwrap();
}

#[test]
fn profiling_matches_provider_and_imported_ordinary_body_keys() {
    let provider = r#"
        package profile.provider
        private fun helper(value: Int): Int = value + 1
        public fun ordinary(value: Int): Int = if (value > 0) helper(value) else 0
    "#;
    with_provider_consumer_export_ir(
        provider,
        "import profile.provider.ordinary\npublic fun answer(value: Int): Int = ordinary(value)",
        hir::IrTemplateExport::Full,
        |consumer, _, _, _, core| {
            let coordinate = ConeCoordinate::new("test", "generic-provider", "1.0.0").unwrap();
            let parsed = super::super::m23_ordinary_core_only::support::parsed_ordinary_text_at(
                &coordinate,
                provider,
            );
            let world = core.world(parsed.cone());
            let input = CurrentConeSources::try_new(
                &parsed,
                core.foundation.import_core_inputs(&core.interface).unwrap(),
                &world,
            )
            .unwrap()
            .with_ir_template_export(hir::IrTemplateExport::Full);
            let producer =
                lower_current_cone(scoop_identity::RequestedConeKind::Library, &input).unwrap();
            let lower = |output: &hir::DependencyHirOutput| {
                scoop_mir_lower::lower_current_cone(
                    output,
                    select_library_callables(output),
                    scoop_mir_lower::MirOptimizationOptions::RELEASE,
                    None,
                    scoop_mir_lower::ProfilingInput::Generate,
                )
                .unwrap()
            };
            let producer = lower(&producer);
            let consumer = lower(&consumer);
            for name in ["ordinary", "helper"] {
                let original = producer
                    .module()
                    .meta
                    .profiling
                    .plans
                    .iter()
                    .find(|body| body.name == name)
                    .expect("provider body");
                let imported = consumer
                    .module()
                    .meta
                    .profiling
                    .plans
                    .iter()
                    .find(|body| body.key.body == original.key.body)
                    .expect("imported body");
                assert_eq!(original.key, imported.key, "{name}");
                assert_eq!(original.positions, imported.positions, "{name}");
            }
        },
    )
    .unwrap();
}
