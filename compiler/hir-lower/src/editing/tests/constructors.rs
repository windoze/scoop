use super::imports::{analyze_files, at};
use super::*;
use hir::{
    EditingCallableIdentity as Identity, EditingCallee as C, EditingReferenceTarget as Target,
};

mod imported;
mod names;
mod signatures;

fn references(analysis: &mut DeclarationAnalysis) -> Vec<hir::EditingCallableReference> {
    let function = function(analysis, "editing");
    match analysis.function_facts(function, || false).unwrap() {
        V::Known(facts) => facts.callables.clone(),
        V::Unavailable(reason) => panic!(
            "missing constructor facts: {reason:?}; {:?}",
            analysis.diagnostics()
        ),
    }
}

#[test]
fn constructors_and_constructor_values_keep_the_selected_overload_and_parameter() {
    let text = "class Choice { constructor(value: Boolean) {}\nconstructor(value: String) {} }\nfun editing() { val first = Choice(value = true); val second = Choice(value = \"text\"); val factory: (Boolean) -> Choice = ::Choice }";
    let mut analysis = analyze_files(&[("src/constructors.scoop", text)]);
    let references = references(&mut analysis);
    assert_eq!(references.len(), 3);
    assert!(matches!(
        references[0].target,
        C::Constructor(hir::SemanticConstructor::CurrentClass(_))
    ));
    assert_eq!(references[0].target, references[2].target);
    assert_ne!(references[0].target, references[1].target);
    let editing = function(&analysis, "editing");
    for (needle, declaration) in [
        ("value = true", "value: Boolean"),
        ("value = \"text\"", "value: String"),
    ] {
        let origin = at("src/constructors.scoop", text, needle, 5);
        let V::Known(Some(argument)) = analysis
            .named_argument(editing, origin.span, || false)
            .unwrap()
        else {
            panic!("checked constructor argument")
        };
        assert_eq!(
            argument.definition,
            Some(at("src/constructors.scoop", text, declaration, 5))
        );
    }
    let index = analysis.reference_index(|| false).unwrap();
    let argument = at("src/constructors.scoop", text, "value = true", 5);
    let targets = index.targets_at(&argument);
    assert!(matches!(
        targets,
        [Target::Parameter {
            callable: Identity::Constructor(_),
            index: 0
        }]
    ));
    assert_eq!(index.references_to(targets[0]).count(), 2);
    let call = at("src/constructors.scoop", text, "Choice(value = true)", 6);
    let target = index.targets_at(&call)[0];
    assert_eq!(index.references_to(target).count(), 3);
}

#[test]
fn generic_alias_constructions_share_the_original_struct_constructor() {
    let text = "struct Box<T>(val item: T)\ntypealias Wrapped<T> = Box<T>\nfun editing() { val first = Wrapped(item = true); val second = Wrapped(item = \"text\"); val factory: (Boolean) -> Box<Boolean> = ::Wrapped }";
    let mut analysis = analyze_files(&[("src/constructors.scoop", text)]);
    let references = references(&mut analysis);
    assert_eq!(references.len(), 3);
    assert!(
        references
            .iter()
            .all(|reference| reference.target == references[0].target)
    );
    assert!(matches!(
        references[0].target,
        C::Constructor(hir::SemanticConstructor::CurrentStruct(_))
    ));
    let function = function(&analysis, "editing");
    let location = at("src/constructors.scoop", text, "item = true", 4);
    let V::Known(Some(argument)) = analysis
        .named_argument(function, location.span, || false)
        .unwrap()
    else {
        panic!("alias preserves its selected constructor")
    };
    assert_eq!(
        argument.definition,
        Some(at("src/constructors.scoop", text, "item: T", 4))
    );
}

#[test]
fn variant_imports_and_unit_values_keep_the_original_variant_declaration() {
    let provider = "package api\nenum Event { Empty, Item(val value: Boolean) }";
    let text = "import api.Event\nimport api.Event.Item as Create\nimport api.Event.Empty as SelectedEmpty\ntypealias Events = Event\nfun editing(): Event { val item = Create(value = true); val copy = Create(value = false); val alias = Events.Empty; return SelectedEmpty }";
    let mut analysis = analyze_files(&[
        ("src/events.scoop", provider),
        ("src/constructors.scoop", text),
    ]);
    let references = references(&mut analysis);
    assert_eq!(references.len(), 4);
    assert!(matches!(references[0].target, C::CurrentVariant(_)));
    assert_eq!(references[0].target, references[1].target);
    assert_ne!(references[0].target, references[2].target);
    assert_eq!(references[2].target, references[3].target);
    for (name, declaration) in [("Create", "Item("), ("SelectedEmpty", "Empty,")] {
        let location = at("src/constructors.scoop", text, name, name.len());
        let origin = analysis
            .import_definitions(&location.source, location.span, || false)
            .unwrap();
        assert_eq!(origin.len(), 1);
        assert!(
            provider[origin[0].span.start as usize..origin[0].span.end as usize]
                .contains(declaration.trim_end_matches([',', '(']))
        );
    }
    let index = analysis.reference_index(|| false).unwrap();
    let origin = at("src/constructors.scoop", text, "value = true", 5);
    assert!(matches!(
        index.targets_at(&origin),
        [Target::Parameter {
            callable: Identity::Variant(_),
            index: 0
        }]
    ));
}

#[test]
fn unused_variant_declarations_publish_their_own_parameter_targets() {
    let text = "enum Event { Empty, Item(val value: Boolean) }";
    let mut analysis = analyze_files(&[("src/constructors.scoop", text)]);
    let index = analysis.reference_index(|| false).unwrap();
    let location = at(
        "src/constructors.scoop",
        text,
        "value: Boolean",
        "value".len(),
    );
    let targets = index.targets_at(&location);
    assert!(matches!(
        targets,
        [Target::Parameter {
            callable: Identity::Variant(_),
            index: 0,
        }]
    ));
    assert_eq!(index.references_to(targets[0]).count(), 1);
    let unit = at("src/constructors.scoop", text, "Empty", 5);
    assert!(matches!(
        index.targets_at(&unit),
        [Target::Callable(Identity::Variant(_))]
    ));
}

#[test]
fn failed_local_type_regions_do_not_publish_partial_constructor_or_property_declarations() {
    let text = "fun editing(): Boolean { class Local(val hidden: Boolean); return missing }";
    let mut analysis = analyze_files(&[("src/constructors.scoop", text)]);
    let index = analysis.reference_index(|| false).unwrap();
    assert!(!index.unavailable.is_empty());
    let name = at("src/constructors.scoop", text, "hidden:", 6);
    assert!(index.targets_at(&name).is_empty());
}
