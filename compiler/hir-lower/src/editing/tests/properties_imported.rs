use super::*;
use crate::tests::m23_ordinary_core_only::support::trusted_core;
use crate::tests::m23_ordinary_dependencies::support::{alias_expansions, project_dependency_text};
use scoop_identity::ConeCoordinate;

#[test]
fn imported_properties_preserve_declarations_through_storage_and_generic_accessors() {
    let mut core = trusted_core();
    let coordinate = ConeCoordinate::new("test", "editing-properties", "1.0.0").unwrap();
    let provider = "public class Box<T> public constructor(public var item: T)\npublic var <T> Box<T>.property: T get() = item\nset(value) { item = value }\npublic const val FLAG: Boolean = true";
    let (foundation, interface) = project_dependency_text(&core, &coordinate, provider, &[]);
    let aliases = alias_expansions(interface.type_aliases());
    let imported = core.import_dependency_foundation(&coordinate, &foundation, 188);
    let identity = crate::tests::test_source_identity("src/editing.scoop");
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
    let core_inputs =
        CoreProtocolInput::from(core.foundation.import_core_inputs(&core.interface).unwrap());
    let text = "import property\nimport FLAG as enabled\nfun editing(value: Box<Boolean>): Boolean { value.item = enabled; value.property = value.item; return value.property }";
    let parsed = scoop_parser::parse_recovering(text).unwrap();
    let source = scoop_parser::declaration_source(
        &parsed,
        &identity,
        scoop_identity::SourceRole::Production,
        text,
    );
    let mut analysis =
        analyze_declarations(&[source], &core_inputs, &world, &[], || false).unwrap();
    let function = function(&analysis, "editing");
    let V::Known(facts) = analysis.function_facts(function, || false).unwrap() else {
        panic!("{:?}", analysis.diagnostics());
    };
    let references = facts.properties.clone();
    assert_eq!(references.len(), 5, "{references:?}");
    assert_eq!(
        references
            .iter()
            .map(|reference| reference.name.as_str())
            .collect::<Vec<_>>(),
        ["item", "FLAG", "property", "item", "property"]
    );
    for reference in &references {
        assert!(matches!(
            reference.target,
            hir::SemanticProperty::Imported(_)
        ));
        assert_eq!(
            reference.definition.as_ref().unwrap().source.cone(),
            coordinate.identity().unwrap()
        );
        let V::Known(ty) = analysis
            .describe_body_type(function, reference.context, reference.value_type)
            .unwrap()
        else {
            panic!("applied property type");
        };
        assert_eq!(ty.name, "Boolean");
    }
    assert_eq!(references[0].access, hir::EditingPropertyAccess::Write);
    assert_eq!(references[2].access, hir::EditingPropertyAccess::Write);
    assert_eq!(references[4].access, hir::EditingPropertyAccess::Read);
    assert_eq!(references[0].target, references[3].target);
    assert_eq!(references[2].target, references[4].target);
    assert_ne!(references[0].target, references[2].target);
}
