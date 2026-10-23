use super::*;
use crate::tests::m23_ordinary_core_only::support::trusted_core;
use crate::tests::m23_ordinary_dependencies::support::{alias_expansions, project_dependency_text};
use scoop_identity::ConeCoordinate;

fn analyze_imported(provider: &str, text: &str) -> DeclarationAnalysis {
    let mut core = trusted_core();
    let coordinate = ConeCoordinate::new("test", "constructor-navigation", "1.0.0").unwrap();
    let (foundation, interface) = project_dependency_text(&core, &coordinate, provider, &[]);
    let aliases = alias_expansions(interface.type_aliases());
    let imported = core.import_dependency_foundation(&coordinate, &foundation, 198);
    let identity = crate::tests::test_source_identity("src/constructors.scoop");
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
    let parsed = scoop_parser::parse_recovering(text).unwrap();
    let source = scoop_parser::declaration_source(
        &parsed,
        &identity,
        scoop_identity::SourceRole::Production,
        text,
    );
    analyze_declarations(&[source], &protocols, &world, &[], || false).unwrap()
}

#[test]
fn imported_constructors_and_variants_keep_parameter_origins_and_callable_values() {
    let coordinate = ConeCoordinate::new("test", "constructor-navigation", "1.0.0").unwrap();
    let provider = "public class Box<T> public constructor(public val value: T)\npublic enum Event { Empty, Item(val value: Boolean) }";
    let text = "import Box as Selected\nimport Event.Item as Make\nimport Event.Empty as Empty\ntypealias Events = Event\nfun editing(): Event { val first = Selected(value = true); val second = Selected(value = \"text\"); val factory: (Boolean) -> Box<Boolean> = ::Selected; val variant = Make(value = false); val copy = Make(value = true); val alias = Events.Empty; return Empty }";
    let mut analysis = analyze_imported(provider, text);
    let references = references(&mut analysis);
    assert_eq!(references.len(), 7, "{references:?}");
    assert!(matches!(
        references[0].target,
        C::Constructor(hir::SemanticConstructor::Imported(_))
    ));
    assert_eq!(references[0].target, references[1].target);
    assert_eq!(references[0].target, references[2].target);
    assert!(matches!(references[3].target, C::ImportedVariant(_)));
    assert_eq!(references[3].target, references[4].target);
    assert_eq!(references[5].target, references[6].target);
    for (index, reference) in references.iter().enumerate() {
        let signature = analysis
            .constructor_signature(reference.target)
            .unwrap()
            .unwrap();
        assert_eq!(signature.callee, reference.target);
        let result = analysis
            .describe_declaration_type(&signature.binders, signature.result)
            .unwrap()
            .name;
        assert_eq!(result, if index < 3 { "Box<T>" } else { "Event" });
        assert_eq!(signature.parameters.len(), usize::from(index < 5));
        assert_eq!(
            signature.origin.source.cone(),
            coordinate.identity().unwrap()
        );
    }
    let function = function(&analysis, "editing");
    for needle in ["value = true", "value = \"text\""] {
        let span = at("src/constructors.scoop", text, needle, 5).span;
        let V::Known(Some(argument)) = analysis.named_argument(function, span, || false).unwrap()
        else {
            panic!("imported constructor parameter")
        };
        let origin = argument.definition.unwrap();
        assert_eq!(origin.source.cone(), coordinate.identity().unwrap());
        assert_eq!(
            &provider[origin.span.start as usize..origin.span.end as usize],
            "value"
        );
    }
    let location = at("src/constructors.scoop", text, "Make", 4);
    assert_eq!(
        analysis
            .import_definitions(&location.source, location.span, || false)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn imported_constructor_aliases_keep_source_names_and_exclude_delegation_keywords() {
    let provider = "package api\npublic open class Box<T> public constructor(value: T)\npublic typealias Wrapped<T> = Box<T>";
    let text = "import api.Box as Selected\nimport api.Wrapped as Another\nfun editing() { val direct = Selected(value = true); val alias = Another(value = true); val factory: (Boolean) -> Selected<Boolean> = ::Another }\nclass Derived : api.Box<Boolean> { constructor(): super(true) {} }";
    let path = "src/constructors.scoop";
    let mut analysis = analyze_imported(provider, text);
    let references = references(&mut analysis);
    assert_eq!(references.len(), 3);
    assert!(
        references
            .iter()
            .all(|reference| reference.target == references[0].target)
    );
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    let selected = index.targets_at(&at(path, text, "Selected\n", 8))[0];
    let another = index.targets_at(&at(path, text, "Another\n", 7))[0];
    assert_eq!(index.references_to(selected).count(), 3);
    assert_eq!(index.references_to(another).count(), 3);
    for needle in ["Another(value", "Another }"] {
        assert_eq!(index.targets_at(&at(path, text, needle, 7)), &[another]);
    }
    let delegation = at(path, text, "super(true)", 5);
    assert!(matches!(
        index.targets_at(&delegation),
        [Target::Callable(Identity::Constructor(_))]
    ));
    assert!(
        !index
            .occurrences()
            .iter()
            .any(|reference| reference.origin == delegation
                && matches!(reference.target, Target::Type(_)))
    );
}
