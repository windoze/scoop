use super::imports::{analyze_files, at};
use super::*;
use hir::{EditingReferenceTarget as R, EditingTypeIdentity as T};

fn reference(
    analysis: &mut DeclarationAnalysis,
    text: &str,
    needle: &str,
    length: usize,
) -> hir::EditingTypeReference {
    let origin = at("src/types.scoop", text, needle, length);
    match analysis
        .type_reference(&origin.source, origin.span, || false)
        .unwrap()
    {
        V::Known(Some(reference)) => reference,
        result => panic!(
            "missing type reference {needle}: {result:?}; {:?}",
            analysis.diagnostics()
        ),
    }
}

#[test]
fn type_references_preserve_aliases_and_distinct_source_parameter_binders() {
    let text = "struct Box<T>(val value: T)\ntypealias Wrapped<T> = Box<T>\nfun <T> identity(value: Wrapped<T>): Box<T> = value\nfun editing(value: Wrapped<Boolean>): Wrapped<Boolean> { val copy: Box<Boolean> = value; return copy }";
    let mut analysis = analyze_files(&[("src/types.scoop", text)]);
    let nominal = reference(&mut analysis, text, "Box<T>(", 3);
    let alias = reference(&mut analysis, text, "Wrapped<T> =", 7);
    assert!(nominal.is_declaration);
    assert!(alias.is_declaration);
    assert!(matches!(nominal.target, T::Nominal(_)));
    assert!(matches!(alias.target, T::Alias(_)));
    assert_eq!(
        reference(&mut analysis, text, "Wrapped<Boolean>", 7).target,
        alias.target
    );
    assert_eq!(
        reference(&mut analysis, text, "Box<Boolean>", 3).target,
        nominal.target
    );
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    assert_eq!(index.references_to(R::Type(alias.target)).count(), 4);
    assert_eq!(index.references_to(R::Type(nominal.target)).count(), 4);
    let parameters = index
        .occurrences()
        .iter()
        .filter(|reference| {
            reference.is_declaration && matches!(reference.target, R::Type(T::Parameter(_)))
        })
        .collect::<Vec<_>>();
    assert_eq!(parameters.len(), 3);
    let identities = parameters
        .iter()
        .map(|reference| reference.target)
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(identities.len(), 3);
    for declaration in parameters {
        assert!(index.references_to(declaration.target).count() >= 2);
    }
}

#[test]
fn qualified_types_keep_each_bound_name_and_exact_import_alias_layer() {
    let provider = "package api\nclass Outer { class Inner {}\ntypealias Alias = Inner }";
    let text = "import api.Outer as Selected\nfun editing(value: Selected.Inner): api.Outer.Alias { val copy: Selected.Alias = value; return copy }\nfun <Selected> shadow(value: Selected): Selected = value";
    let mut analysis = analyze_files(&[("src/api.scoop", provider), ("src/types.scoop", text)]);
    let inner = reference(&mut analysis, text, "Inner)", 5);
    assert_eq!(
        inner.definition,
        Some(at("src/api.scoop", provider, "Inner {}", 5))
    );
    let alias = reference(&mut analysis, text, "Alias {", 5);
    assert_eq!(
        alias.definition,
        Some(at("src/api.scoop", provider, "Alias =", 5))
    );
    assert_ne!(inner.target, alias.target);
    let selected = reference(&mut analysis, text, "Selected.Inner", 8);
    assert_eq!(
        selected.definition,
        Some(at("src/api.scoop", provider, "Outer {", 5))
    );
    assert_eq!(selected.imports.len(), 1);
    assert!(
        reference(&mut analysis, text, "Outer.Alias", 5)
            .imports
            .is_empty()
    );
    let parameter = reference(&mut analysis, text, "Selected>", 8);
    assert!(matches!(parameter.target, T::Parameter(_)));
    assert!(parameter.imports.is_empty());
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    let (id, _) = index
        .aliases
        .iter()
        .find(|(_, alias)| alias.name == "Selected")
        .unwrap();
    assert_eq!(index.references_to(R::ImportAlias(id)).count(), 3);
    assert_eq!(index.references_to(R::Type(parameter.target)).count(), 3);
    assert!(
        index
            .references_to(R::Type(parameter.target))
            .all(|reference| reference.aliases.is_empty())
    );
    assert!(
        index
            .targets_at(&at("src/types.scoop", text, "api.Outer.Alias", 3))
            .is_empty()
    );
}

#[test]
fn body_type_references_are_committed_only_after_success_and_cancelled_queries_retry() {
    let text = "class Item {}\nfun good(input: Item): Item { val copy: Item = input; return copy }\nfun bad(input: Item): Item { val failed: Item = missing; return input }";
    let mut analysis = analyze_files(&[("src/types.scoop", text)]);
    let declaration = reference(&mut analysis, text, "Item {}", 4);
    assert!(analysis.body_facts.is_empty());
    let origin = at("src/types.scoop", text, "Item = input", 4);
    assert!(matches!(
        analysis.type_reference(&origin.source, origin.span, || true),
        Err(EditingCancelled)
    ));
    assert!(analysis.body_facts.is_empty());
    assert_eq!(
        reference(&mut analysis, text, "Item = input", 4).target,
        declaration.target
    );
    let bad = at("src/types.scoop", text, "Item = missing", 4);
    assert!(matches!(
        analysis
            .type_reference(&bad.source, bad.span, || false)
            .unwrap(),
        V::Unavailable(U::BodyFailure)
    ));
    let index = analysis.reference_index(|| false).unwrap();
    assert_eq!(index.references_to(R::Type(declaration.target)).count(), 6);
    assert_eq!(index.unavailable.len(), 1);
    assert!(index.targets_at(&bad).is_empty());
}

#[test]
fn local_types_and_parameters_keep_their_original_declaration_sites() {
    let text = "class Value {}\nfun <T> editing(input: T): T { typealias Alias = T; class Local(val value: Alias); val copy: Alias = input; fun <U> nested(value: U): U = value; return nested(copy) }";
    let mut analysis = analyze_files(&[("src/types.scoop", text)]);
    let alias = reference(&mut analysis, text, "Alias = T", 5);
    assert!(alias.is_declaration);
    assert_eq!(
        reference(&mut analysis, text, "Alias = input", 5).target,
        alias.target
    );
    let outer = reference(&mut analysis, text, "T> editing", 1);
    let inner = reference(&mut analysis, text, "U> nested", 1);
    assert_ne!(outer.target, inner.target);
    assert_eq!(
        reference(&mut analysis, text, "U = value", 1).target,
        inner.target
    );
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    assert_eq!(index.references_to(R::Type(alias.target)).count(), 3);
}

#[test]
fn inferred_local_type_sources_are_reused_when_the_body_is_queried_later() {
    let text = "class Item {}\nfun useInferred(): Boolean = inferred(true)()\nfun inferred(flag: Boolean) = { class Local(val input: Item); flag }";
    let mut analysis = analyze_files(&[("src/types.scoop", text)]);
    let caller = function(&analysis, "useInferred");
    assert!(
        matches!(
            analysis.function_facts(caller, || false).unwrap(),
            V::Known(_)
        ),
        "{:?}",
        analysis.diagnostics()
    );
    let declaration = reference(&mut analysis, text, "Item {}", 4);
    let local_type = reference(&mut analysis, text, "Item);", 4);
    assert_eq!(local_type.target, declaration.target);
    assert_eq!(local_type.definition, declaration.definition);
    let index = analysis.reference_index(|| false).unwrap();
    assert!(
        index
            .targets_at(&local_type.origin)
            .contains(&R::Type(declaration.target))
    );
}
