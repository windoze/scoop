use super::*;
use crate::tests::m23_ordinary_core_only::support::trusted_core;
use crate::tests::m23_ordinary_dependencies::support::{alias_expansions, project_dependency_text};
use scoop_identity::ConeCoordinate;

#[test]
fn dependency_constants_preserve_original_property_and_alias_origins() {
    let mut core = trusted_core();
    let coordinate = ConeCoordinate::new("test", "constant-references", "1.0.0").unwrap();
    let provider = "public const val FLAG: Boolean = true\npublic object Options { public const val FLAG: Boolean = false }";
    let (foundation, interface) = project_dependency_text(&core, &coordinate, provider, &[]);
    let aliases = alias_expansions(interface.type_aliases());
    let imported = core.import_dependency_foundation(&coordinate, &foundation, 220);
    let identity = crate::tests::test_source_identity(FILE);
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
    let text = "import FLAG as enabled\nimport Options.FLAG as optionFlag\nconst val direct = enabled\nval image: Boolean = !enabled\nconst val selected = Options.FLAG\nval exact: Boolean = optionFlag";
    let parsed = scoop_parser::parse_recovering(text).unwrap();
    let input = scoop_parser::declaration_source(
        &parsed,
        &identity,
        scoop_identity::SourceRole::Production,
        text,
    );
    let mut analysis = analyze_declarations(&[input], &protocols, &world, &[], || false).unwrap();
    assert!(
        analysis.diagnostics().is_empty(),
        "{:?}",
        analysis.diagnostics()
    );
    assert_eq!(analysis.constant_references.len(), 4);
    let direct = property(&mut analysis, text, "enabled\nval", "enabled");
    let image = property(&mut analysis, text, "enabled\nconst", "enabled");
    assert_eq!(direct.property, image.property);
    assert!(matches!(
        direct.property,
        hir::SemanticProperty::Imported(_)
    ));
    assert_eq!(
        direct.origin.unwrap().source.cone(),
        coordinate.identity().unwrap()
    );
    let selected = property(&mut analysis, text, "FLAG\nval", "FLAG");
    let exact = property(&mut analysis, text, "optionFlag", "optionFlag");
    assert_eq!(selected.property, exact.property);
    assert_ne!(selected.property, image.property);
    assert_eq!(
        analysis
            .describe_declaration_type(&selected.binders, selected.value_type)
            .unwrap()
            .name,
        "Boolean"
    );
    let index = analysis.reference_index(|| false).unwrap();
    assert_eq!(index.references_to(alias(index, "enabled")).count(), 3);
    assert_eq!(index.references_to(alias(index, "optionFlag")).count(), 2);
    assert!(analysis.body_facts.is_empty());
}
