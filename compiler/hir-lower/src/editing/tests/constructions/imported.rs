use super::*;
use crate::tests::m23_ordinary_core_only::support::trusted_core;
use crate::tests::m23_ordinary_dependencies::support::{alias_expansions, project_dependency_text};
use scoop_identity::ConeCoordinate;

#[test]
fn imported_base_initialization_preserves_constructor_parameters_and_storage_targets() {
    let mut core = trusted_core();
    let coordinate = ConeCoordinate::new("test", "construction-bodies", "1.0.0").unwrap();
    let provider = "public open class Base<T> public constructor(public val first: T, public var second: Boolean)";
    let (foundation, interface) = project_dependency_text(&core, &coordinate, provider, &[]);
    let aliases = alias_expansions(interface.type_aliases());
    let imported = core.import_dependency_foundation(&coordinate, &foundation, 211);
    let identity = crate::tests::test_source_identity("src/construction.scoop");
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
    let text = "import Base as ImportedBase\nclass Child<T>(value: T): ImportedBase<T>(first = value, second = true) { val copy: T = first; init { second = false } }\nclass Terminal: ImportedBase<Boolean> { constructor(value: Boolean): super(first = value, second = true) {} }";
    let parsed = scoop_parser::parse_recovering(text).unwrap();
    let source = scoop_parser::declaration_source(
        &parsed,
        &identity,
        scoop_identity::SourceRole::Production,
        text,
    );
    let mut analysis = analyze_declarations(&[source], &protocols, &world, &[], || false).unwrap();
    let mut target = None;
    for needle in ["ImportedBase<T>(first", "super(first"] {
        let body = region(&mut analysis, text, needle);
        let observed = facts(&mut analysis, &body);
        assert_eq!(observed.callables.len(), 1);
        let callee = observed.callables[0].target;
        assert!(
            matches!(
                callee,
                hir::EditingCallee::Constructor(hir::SemanticConstructor::Imported(_))
            ),
            "{callee:?}"
        );
        if let Some(target) = target {
            assert_eq!(target, callee);
        }
        target = Some(callee);
        let span = ast::Span::new(
            body.origin.span.start + needle.find("first").unwrap() as u32,
            body.origin.span.start + needle.find("first").unwrap() as u32 + 5,
        );
        let V::Known(Some(argument)) = analysis.named_argument(body.id, span, || false).unwrap()
        else {
            panic!("imported base named parameter")
        };
        let origin = argument.definition.unwrap();
        assert_eq!(origin.source.cone(), coordinate.identity().unwrap());
        assert_eq!(
            &provider[origin.span.start as usize..origin.span.end as usize],
            "first"
        );
    }
    let body = region(&mut analysis, text, "first; init");
    let observed = facts(&mut analysis, &body);
    assert_eq!(observed.properties.len(), 1);
    let property = &observed.properties[0];
    assert!(matches!(
        property.target,
        hir::SemanticProperty::Imported(_)
    ));
    let V::Known(ty) = analysis
        .describe_body_type(body.id, property.context, property.value_type)
        .unwrap()
    else {
        panic!("inherited generic field type")
    };
    assert_eq!(ty.name, "T");
    let body = region(&mut analysis, text, "second = false");
    let observed = facts(&mut analysis, &body);
    assert_eq!(
        observed.properties[0].access,
        hir::EditingPropertyAccess::Write
    );
    assert!(matches!(
        observed.properties[0].target,
        hir::SemanticProperty::Imported(_)
    ));
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
}
