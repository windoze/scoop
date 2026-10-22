use super::*;
use crate::tests::m23_ordinary_core_only::support::trusted_core;
use crate::tests::m23_ordinary_dependencies::support::{alias_expansions, project_dependency_text};
use scoop_identity::ConeCoordinate;

#[test]
fn imported_hovers_preserve_declaration_binders_and_default_origins() {
    let mut core = trusted_core();
    let coordinate = ConeCoordinate::new("test", "imported-hover", "1.0.0").unwrap();
    let provider = "public class Box<Element> public constructor(public val item: Element) { public fun read(): Element = item }\npublic fun <Item> pass(value: Item): Item = value\npublic fun choose(value: Boolean = true): Boolean = value\npublic fun <Item> Box<Item>.extension(value: Item): Item = value\n@NoGC public fun native(value: Int): Int = value";
    let (foundation, interface) = project_dependency_text(&core, &coordinate, provider, &[]);
    let aliases = alias_expansions(interface.type_aliases());
    let imported = core.import_dependency_foundation(&coordinate, &foundation, 187);
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
    let text = "import pass as renamed\nimport extension\nimport native as callbackAddress\nfun editing(box: Box<Boolean>): Boolean { val first = box.read(); val second = renamed(value = choose()); val callback: (Boolean) -> Boolean = ::renamed; val method: () -> Boolean = box::read; val equality: (Boolean) -> Boolean = first::equals; val address: FunPtr<(Int) -> Int> = ::callbackAddress; return box.extension(equality(callback(method()))) }";
    let parsed = scoop_parser::parse_recovering(text).unwrap();
    let source = scoop_parser::declaration_source(
        &parsed,
        &identity,
        scoop_identity::SourceRole::Production,
        text,
    );
    let mut analysis =
        analyze_declarations(&[source], &core_inputs, &world, &[], || false).unwrap();
    let alias_offset = text.find("callbackAddress").unwrap() as u32;
    let definitions = analysis
        .import_definitions(
            &identity,
            ast::Span::new(alias_offset, alias_offset + 15),
            || false,
        )
        .unwrap();
    assert_eq!(definitions.len(), 1);
    assert_eq!(definitions[0].source.cone(), coordinate.identity().unwrap());
    assert_eq!(
        &provider[definitions[0].span.start as usize..definitions[0].span.end as usize],
        "@NoGC public fun native(value: Int): Int = value"
    );
    assert!(analysis.body_facts.is_empty());
    let function = function(&analysis, "editing");
    let V::Known(facts) = analysis.function_facts(function, || false).unwrap() else {
        panic!("consumer body failed: {:?}", analysis.diagnostics());
    };
    let references = facts.callables.clone();
    let offset = text.find("value = choose()").unwrap() as u32;
    let V::Known(Some(origin)) = analysis
        .argument_definition(function, ast::Span::new(offset, offset + 5), || false)
        .unwrap()
    else {
        panic!("an imported named argument retains its declaration");
    };
    assert_eq!(origin.source.cone(), coordinate.identity().unwrap());
    assert_eq!(
        origin.span.start as usize,
        provider.find("pass(value").unwrap() + 5
    );
    assert_eq!(
        &provider[origin.span.start as usize..origin.span.end as usize],
        "value"
    );
    assert_eq!(references.len(), 8);
    for reference in references {
        let hir::SemanticCallable::Imported(callable) = reference.target else {
            panic!("imported call");
        };
        let signature = analysis.imported_signature(callable).unwrap().unwrap();
        assert_eq!(signature.function, reference.target);
        assert_eq!(
            signature.origin.source.cone(),
            if signature.name == "equals" {
                ConeIdentity::CORE
            } else {
                coordinate.identity().unwrap()
            }
        );
        let binders = signature.declaration_binders();
        let result = analysis
            .describe_declaration_type(&binders, signature.result)
            .unwrap();
        match signature.name.as_str() {
            "equals" => {
                assert_eq!(result.name, "Boolean");
                assert!(binders.owner.is_empty());
            }
            "pass" | "extension" => {
                assert_eq!(result.name, "Item");
                assert_eq!(binders.callable[0].name, "Item");
            }
            "read" => {
                assert_eq!(result.name, "Element");
                assert_eq!(binders.owner[0].name, "Element");
                assert!(binders.callable.is_empty());
            }
            "choose" => {
                assert_eq!(result.name, "Boolean");
                let hir::SemanticParameterCalling::Default(default) =
                    &signature.parameters[0].calling
                else {
                    panic!("published default");
                };
                let hir::SemanticDefaultReference::Imported(key) = default.definition else {
                    panic!("imported default");
                };
                let origin = analysis.imported_default_origin(key).unwrap();
                assert_eq!(
                    &provider[origin.span.start as usize..origin.span.end as usize],
                    "true"
                );
            }
            "native" => {
                assert_eq!(result.name, "Int");
                assert_eq!(signature.attributes.gc_effect, hir::GcEffect::NoGc);
            }
            other => panic!("unexpected call: {other}"),
        }
    }
    let index = analysis.reference_index(|| false).unwrap();
    for (name, count) in [("renamed", 3), ("callbackAddress", 2)] {
        let (id, alias) = index
            .aliases
            .iter()
            .find(|(_, alias)| alias.name == name)
            .unwrap();
        assert_eq!(
            index
                .references_to(hir::EditingReferenceTarget::ImportAlias(id))
                .count(),
            count
        );
        assert_eq!(
            index
                .references_to(*alias.targets.iter().next().unwrap())
                .count(),
            count
        );
    }
}
