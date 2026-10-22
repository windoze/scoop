use super::*;
use crate::tests::m23_ordinary_core_only::support::trusted_core;
use crate::tests::m23_ordinary_dependencies::support::{alias_expansions, project_dependency_text};
use scoop_identity::ConeCoordinate;

#[test]
fn unused_dependency_properties_read_original_metadata_without_selecting_execution() {
    let mut core = trusted_core();
    let coordinate = ConeCoordinate::new("test", "property-hover", "1.0.0").unwrap();
    let provider = "public class Box<T> public constructor(public var item: T)\npublic var <T> Box<T>.projected: T get() = item\nset(value) { item = value }\npublic const val FLAG: Boolean = true\npublic object Values { public val flag: Boolean = true }";
    let (foundation, interface) = project_dependency_text(&core, &coordinate, provider, &[]);
    let aliases = alias_expansions(interface.type_aliases());
    let imported = core.import_dependency_foundation(&coordinate, &foundation, 196);
    let identity = crate::tests::test_source_identity("src/properties.scoop");
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
    let text = "import projected as selected\nimport FLAG as enabled\nimport Values.flag as saved\nfun broken(): Boolean = missing";
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
        .selected_callable_count();
    let original = declaration(&mut analysis, text, "projected", "projected");
    let alias = declaration(&mut analysis, text, "selected", "selected");
    assert_eq!(original.property, alias.property);
    assert!(matches!(alias.property, hir::SemanticProperty::Imported(_)));
    assert_eq!(alias.name, "projected");
    assert_eq!(type_name(&mut analysis, &alias), "T");
    assert!(alias.binders.owner.is_empty());
    assert_eq!(alias.binders.callable[0].name, "T");
    assert_eq!(
        analysis
            .describe_declaration_type(&alias.binders, alias.receiver.unwrap())
            .unwrap()
            .name,
        "Box<T>"
    );
    assert!(alias.mutable);
    for (name, original) in [("enabled", "FLAG"), ("saved", "flag")] {
        let property = declaration(&mut analysis, text, name, name);
        assert_eq!(property.name, original);
        assert!(!property.mutable);
        assert_eq!(type_name(&mut analysis, &property), "Boolean");
        assert_eq!(
            property.origin.unwrap().source.cone(),
            coordinate.identity().unwrap()
        );
    }
    assert_eq!(
        analysis
            .lowerer
            .dependencies
            .as_ref()
            .unwrap()
            .selected_callable_count(),
        before
    );
    assert!(analysis.body_facts.is_empty());
}
