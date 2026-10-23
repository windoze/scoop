use super::*;

#[test]
fn constructor_names_keep_the_selected_type_or_transparent_alias() {
    let text = "struct Box<T>(val item: T)\ntypealias Wrapped<T> = Box<T>\nfun editing() { val direct = Box(item = false); val alias = Wrapped(item = true); val factory: (Boolean) -> Box<Boolean> = ::Wrapped }";
    let path = "src/names.scoop";
    let mut analysis = analyze_files(&[(path, text)]);
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    let alias = index.targets_at(&at(path, text, "Wrapped<T> =", 7))[0];
    let nominal = index.targets_at(&at(path, text, "Box<T>(", 3))[0];
    assert!(matches!(
        alias,
        Target::Type(hir::EditingTypeIdentity::Alias(_))
    ));
    assert!(matches!(
        nominal,
        Target::Type(hir::EditingTypeIdentity::Nominal(_))
    ));
    assert_eq!(index.references_to(alias).count(), 3);
    assert_eq!(index.references_to(nominal).count(), 4);
    for needle in ["Wrapped(item", "Wrapped }"] {
        assert_eq!(index.targets_at(&at(path, text, needle, 7)), &[alias]);
    }
    let direct = index.targets_at(&at(path, text, "Box(item", 3));
    assert!(matches!(
        direct,
        [Target::Callable(Identity::Constructor(_))]
    ));
    assert_eq!(index.references_to(direct[0]).count(), 4);
}

#[test]
fn type_import_aliases_link_only_the_actual_constructor_names() {
    let provider = "package api\nclass Choice { constructor(value: Boolean) {}\nconstructor(value: String) {} }\ntypealias Wrapped = Choice";
    let text = "import api.Choice as Selected\nimport api.Wrapped as WrappedName\nfun editing() { val first = Selected(true); val second = Selected(\"text\"); val factory: (Boolean) -> Selected = ::Selected; val alias = WrappedName(true) }";
    let path = "src/names.scoop";
    let mut analysis = analyze_files(&[("src/api.scoop", provider), (path, text)]);
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    let selected = index.targets_at(&at(path, text, "Selected\n", 8))[0];
    let wrapped = index.targets_at(&at(path, text, "WrappedName\n", 11))[0];
    assert_eq!(index.references_to(selected).count(), 5);
    assert_eq!(index.references_to(wrapped).count(), 2);
    for needle in ["Selected(true)", "Selected(\"text\")", "Selected;"] {
        assert_eq!(index.targets_at(&at(path, text, needle, 8)), &[selected]);
    }
    assert_eq!(
        index.targets_at(&at(path, text, "WrappedName(true)", 11)),
        &[wrapped]
    );
    let Target::ImportAlias(alias) = selected else {
        panic!("source import alias")
    };
    assert_eq!(
        index.aliases[alias]
            .targets
            .iter()
            .filter(|target| matches!(target, Target::Callable(Identity::Constructor(_))))
            .count(),
        2
    );
    assert_eq!(
        index
            .occurrences()
            .iter()
            .filter(|reference| matches!(
                reference.target,
                Target::Callable(Identity::Constructor(_))
            ) && reference.aliases.contains(&index.aliases[alias].origin))
            .count(),
        3
    );
}

#[test]
fn constructor_name_observations_discard_other_candidates_failed_bodies_and_cancellation() {
    let text = "class Shared(value: Boolean)\nfun Shared(value: String): Boolean = true\nfun editing() { val constructor = Shared(true); val function = Shared(\"text\"); val factory: (Boolean) -> Shared = ::Shared; val callable: (String) -> Boolean = ::Shared }\nfun broken() { val value = Shared(false); missing }";
    let path = "src/names.scoop";
    let mut analysis = analyze_files(&[(path, text)]);
    assert!(analysis.reference_index(|| true).is_err());
    let index = analysis.reference_index(|| false).unwrap();
    assert_eq!(index.unavailable.len(), 1);
    let nominal = index.targets_at(&at(path, text, "Shared(value: Boolean)", 6))[0];
    assert_eq!(index.references_to(nominal).count(), 4);
    let callable = index.targets_at(&at(path, text, "Shared(value: String)", 6))[0];
    for needle in ["Shared(\"text\")", "Shared }"] {
        assert_eq!(index.targets_at(&at(path, text, needle, 6)), &[callable]);
    }
    assert!(
        index
            .targets_at(&at(path, text, "Shared(false)", 6))
            .is_empty()
    );
}
