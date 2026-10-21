use super::*;

#[test]
fn m60_documentation_keeps_imported_equality_as_a_conditional_source_member() {
    let source = r#"
        import api.Plain
        import api.Bag
        import api.Conditional
        import api.Explicit
        /** [Plain.equals] [Bag.equals] [Conditional.equals] [Explicit.equals] */
        public fun describe() {}
        "#;
    super::super::super::m23_generic_body_consumption::with_provider_consumer(
        r#"
        package api
        public struct Plain(val value: Int)
        public struct Bag<Element>(val value: Element)
        public class Reference
        public struct Conditional(val value: Reference)
        public struct Explicit(val value: Int) {
            public operator fun equals(other: Explicit): Boolean = true
        }
        "#,
        source,
        |ordinary, world, _, _, core| {
            let parsed = support::parsed_ordinary_text(source);
            let input = crate::CurrentConeSources::try_new(
                &parsed,
                core.foundation.import_core_inputs(&core.interface).unwrap(),
                world,
            )
            .unwrap()
            .with_documentation(vec![ast::IdentifiedSourceDocumentation {
                source: test_source_identity("src/main.scoop"),
                documentation: scoop_doc::prepare_source(
                    &scoop_parser::parse(source).unwrap(),
                    source,
                ),
            }])
            .unwrap();
            let output =
                crate::lower_current_cone(scoop_identity::RequestedConeKind::Library, &input)
                    .unwrap();
            assert!(
                output
                    .output()
                    .documentation
                    .as_ref()
                    .unwrap()
                    .warnings
                    .is_empty(),
                "{:?}",
                output.output().documentation.as_ref().unwrap().warnings
            );
            let references = &comment(&output, Kind::Function, "describe").body.references;
            assert_eq!(references.len(), 4);
            for reference in &references[..3] {
                assert!(matches!(&reference.resolution, Link::Symbols(symbols)
                    if matches!(symbols.first(), Symbol::ImportedDerivedEquality(_))));
            }
            assert!(matches!(&references[3].resolution, Link::Symbols(symbols)
                if matches!(symbols.first(), Symbol::Imported(_))));
            let input = super::super::projection::project(&output);
            for (bound, reference) in references[..3].iter().zip(
                &super::super::projection::item(&input, scoop_doc::ApiKind::Function, "describe")
                    .comment
                    .as_ref()
                    .unwrap()
                    .body
                    .references,
            ) {
                let Link::Symbols(symbols) = &bound.resolution else {
                    panic!()
                };
                let Symbol::ImportedDerivedEquality(owner) = symbols.first() else {
                    panic!()
                };
                assert_eq!(
                    reference.target,
                    scoop_doc::LinkTarget::Symbols(vec![
                        scoop_doc::SymbolKey::derived_equality(*owner).unwrap()
                    ])
                );
            }
            assert_eq!(
                output.output().export.derived_equality_applications.len(),
                ordinary.output().export.derived_equality_applications.len()
            );
            assert_eq!(
                output.output().export.imported_derived_equalities.len(),
                ordinary.output().export.imported_derived_equalities.len()
            );
        },
    )
    .unwrap();
}
