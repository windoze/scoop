use super::*;
use crate::tests::m23_ordinary_core_only::support::trusted_core;
use crate::tests::m23_ordinary_dependencies::support::{alias_expansions, project_dependency_text};
use scoop_identity::ConeCoordinate;

#[test]
fn imported_alias_descriptions_keep_the_original_name_and_do_not_select_a_use() {
    let mut core = trusted_core();
    let coordinate = ConeCoordinate::new("test", "type-hover", "1.0.0").unwrap();
    let provider = "package api\npublic struct Box<Element>(val value: Element)\npublic typealias Wrapped<Item> = Box<Item>";
    let (foundation, interface) = project_dependency_text(&core, &coordinate, provider, &[]);
    let aliases = alias_expansions(interface.type_aliases());
    let imported = core.import_dependency_foundation(&coordinate, &foundation, 195);
    let identity = crate::tests::test_source_identity("src/types.scoop");
    let world = hir::ImportedSemanticWorld::from_dependencies(
        identity.cone(),
        vec![
            core.provider(),
            hir::ImportedProviderInput {
                foundation: &imported,
                interface: &interface,
                alias_expansions: &aliases,
            },
        ],
        Vec::new(),
    )
    .unwrap();
    let protocols = core
        .foundation
        .import_core_inputs(&core.interface)
        .unwrap()
        .into();
    let text = "import api.Wrapped as Selected\nimport api.Box as Renamed\nfun broken(): Boolean = missing";
    let parsed = scoop_parser::parse_recovering(text).unwrap();
    let input = scoop_parser::declaration_source(
        &parsed,
        &identity,
        scoop_identity::SourceRole::Production,
        text,
    );
    let mut analysis = analyze_declarations(&[input], &protocols, &world, &[], || false).unwrap();
    let before = analysis
        .lowerer
        .dependencies
        .as_ref()
        .unwrap()
        .selected_type_alias_count();
    let types = analysis.lowerer.types.len();
    for name in ["Wrapped", "Selected"] {
        let alias = description(&mut analysis, text, name, name);
        assert_eq!(alias.label, "typealias Wrapped<Item> = Box<Item>");
        assert!(matches!(
            alias.documentation,
            Some(hir::DocumentationSymbol::Imported(
                hir::AnnotationTargetV1::TypeAlias(_)
            ))
        ));
        let origin = alias.definition.unwrap();
        assert_eq!(origin.source.cone(), coordinate.identity().unwrap());
        assert!(
            provider[origin.span.start as usize..origin.span.end as usize]
                .contains("typealias Wrapped")
        );
    }
    let nominal = description(&mut analysis, text, "Renamed", "Renamed");
    assert_eq!(nominal.label, "struct Box<Element>");
    assert_eq!(
        analysis
            .lowerer
            .dependencies
            .as_ref()
            .unwrap()
            .selected_type_alias_count(),
        before
    );
    assert_eq!(analysis.lowerer.types.len(), types);
    assert!(analysis.body_facts.is_empty());
}
