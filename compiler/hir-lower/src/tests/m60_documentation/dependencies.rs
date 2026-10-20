use super::*;

#[test]
fn m60_documentation_resolves_dependency_aliases_members_without_selecting_machine_uses() {
    let source = r#"
        import api.Cell as ImportedCell
        import api.Entry
        /** [ImportedCell] [Entry] [Entry.value] [ImportedCell.read] [api.create] */
        public fun describe() {}
        "#;
    super::super::m23_generic_body_consumption::with_provider_consumer(
        r#"
        package api
        public class Cell(public val value: Int) {
            public fun read(): Int = value
        }
        public typealias Entry = Cell
        public fun create(): Int = 1
        "#,
        source,
        |ordinary, world, _, _, core| {
            let parsed = support::parsed_ordinary_text(source);
            let file = scoop_parser::parse(source).unwrap();
            let input = crate::CurrentConeSources::try_new(
                &parsed, core.foundation.import_core_inputs(&core.interface).unwrap(), world,
            ).unwrap().with_documentation(vec![ast::IdentifiedSourceDocumentation {
                source: test_source_identity("src/main.scoop"),
                documentation: scoop_doc::prepare_source(&file, source),
            }]).unwrap();
            let output = crate::lower_current_cone(scoop_identity::RequestedConeKind::Library, &input).unwrap();
            let docs = output.output().documentation.as_ref().unwrap();
            assert!(docs.warnings.is_empty(), "{:?}", docs.warnings);
            let links = &comment(&output, Kind::Function, "describe").body.references;
            assert_eq!(links.len(), 5);
            for link in links {
                assert!(matches!(&link.resolution, Link::Symbols(symbols)
                    if symbols.iter().all(|symbol| matches!(symbol, Symbol::Imported(_)))));
            }
            assert!(matches!(&links[1].resolution, Link::Symbols(symbols)
                if matches!(symbols.first(), Symbol::Imported(hir::AnnotationTargetV1::TypeAlias(_)))));
            assert_eq!(output.output().export.functions.len(), ordinary.output().export.functions.len());
            assert_eq!(output.output().export.imported_dependency_callables.len(), ordinary.output().export.imported_dependency_callables.len());
        },
    ).unwrap();
}
