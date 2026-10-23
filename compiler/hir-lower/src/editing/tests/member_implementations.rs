use super::imports::{analyze_files, at};
use super::*;
use hir::EditingImplementationMember as Member;

mod imported;

fn target(analysis: &mut DeclarationAnalysis, text: &str, needle: &str, length: usize) -> Member {
    let origin = at("src/members.scoop", text, needle, length);
    let V::Known(targets) = analysis
        .member_implementation_targets(&origin.source, origin.span, || false)
        .unwrap()
    else {
        panic!("known member: {:?}", analysis.diagnostics());
    };
    assert_eq!(targets.len(), 1, "{targets:?}");
    targets[0]
}

fn has_relation(
    index: &hir::EditingMemberImplementationIndex,
    requirement: Member,
    implementation: Member,
) -> bool {
    index
        .relations
        .contains(&hir::EditingMemberImplementationRelation {
            requirement,
            implementation,
        })
}

#[test]
fn abstract_methods_connect_real_overrides_and_defaults_without_reading_bodies() {
    let text = "interface Root<T> { fun select(value: T): T }\ninterface Default<T> : Root<T> { override fun select(defaultValue: T): T = defaultValue }\nabstract class Base<T> : Root<T> { abstract override fun select(baseValue: T): T }\nclass Concrete : Base<Boolean>() { override fun select(concreteValue: Boolean): Boolean = concreteValue }\nclass UsesDefault : Default<Boolean> {}\nfun damaged(value: Root<Boolean>) { value. }";
    let mut analysis = analyze_files(&[("src/members.scoop", text)]);
    let root = target(&mut analysis, text, "select(value:", 6);
    let default = target(&mut analysis, text, "select(defaultValue:", 6);
    let base = target(&mut analysis, text, "select(baseValue:", 6);
    let concrete = target(&mut analysis, text, "select(concreteValue:", 6);
    let V::Known(index) = analysis.member_implementation_index(|| false).unwrap() else {
        panic!("known graph");
    };
    for (requirement, implementation) in [
        (root, default),
        (root, base),
        (base, concrete),
        (root, concrete),
    ] {
        assert!(
            has_relation(index, requirement, implementation),
            "{index:?}"
        );
    }
    let definitions = index
        .implementations
        .iter()
        .map(|implementation| implementation.declaration)
        .collect::<Vec<_>>();
    assert!(definitions.contains(&default));
    assert!(definitions.contains(&concrete));
    assert!(!definitions.contains(&root));
    assert!(!definitions.contains(&base));
    assert!(analysis.body_facts.is_empty());
    assert!(analysis.recovered_facts.is_empty());
}

#[test]
fn properties_keep_logical_overrides_and_default_accessors() {
    let text = "interface Values { var value: Boolean }\ninterface Defaults : Values { override var value: Boolean get() = true\nset(updated) {} }\nopen class Base : Values { open override var value: Boolean = true }\nclass Child : Base() { override var value: Boolean = false }\nclass UsesDefaults : Defaults {}";
    let mut analysis = analyze_files(&[("src/members.scoop", text)]);
    let root = target(&mut analysis, text, "value: Boolean }", 5);
    let default = target(&mut analysis, text, "value: Boolean get", 5);
    let base = target(&mut analysis, text, "value: Boolean = true", 5);
    let child = target(&mut analysis, text, "value: Boolean = false", 5);
    assert!(
        [root, default, base, child]
            .iter()
            .all(|target| matches!(target, Member::Property(_)))
    );
    let V::Known(index) = analysis.member_implementation_index(|| false).unwrap() else {
        panic!("known graph");
    };
    for (requirement, implementation) in [(root, default), (root, base), (base, child)] {
        assert!(
            has_relation(index, requirement, implementation),
            "{index:?}"
        );
    }
    for member in [default, base, child] {
        assert_eq!(
            index
                .implementations
                .iter()
                .filter(|implementation| implementation.declaration == member)
                .count(),
            1
        );
    }
    assert!(
        !index
            .implementations
            .iter()
            .any(|implementation| implementation.declaration == root)
    );
    assert!(analysis.body_facts.is_empty());
}

#[test]
fn overload_selection_and_cancellation_keep_member_indexes_precise() {
    let text = "interface Value { fun choose(flag: Boolean): Boolean\nfun choose(text: String): String }\nclass Concrete : Value { override fun choose(result: Boolean): Boolean = result\noverride fun choose(result: String): String = result }\nfun editing(value: Value): Boolean = value.choose(true)";
    let mut analysis = analyze_files(&[("src/members.scoop", text)]);
    let boolean = target(&mut analysis, text, "choose(flag:", 6);
    let string = target(&mut analysis, text, "choose(text:", 6);
    let boolean_implementation = target(&mut analysis, text, "choose(result: Boolean)", 6);
    let string_implementation = target(&mut analysis, text, "choose(result: String)", 6);
    assert_eq!(target(&mut analysis, text, "choose(true)", 6), boolean);
    let checks = std::cell::Cell::new(0);
    assert!(matches!(
        analysis.member_implementation_index(|| {
            checks.set(checks.get() + 1);
            checks.get() == 3
        }),
        Err(EditingCancelled)
    ));
    assert!(analysis.member_implementations.is_none());
    let V::Known(index) = analysis.member_implementation_index(|| false).unwrap() else {
        panic!("known graph");
    };
    assert!(has_relation(index, boolean, boolean_implementation));
    assert!(has_relation(index, string, string_implementation));
    assert!(!has_relation(index, boolean, string_implementation));
    assert!(!has_relation(index, string, boolean_implementation));
    checks.set(0);
    analysis
        .member_implementation_index(|| {
            checks.set(checks.get() + 1);
            false
        })
        .unwrap();
    assert_eq!(checks.get(), 1);
}

#[test]
fn a_partial_interface_default_keeps_its_concrete_accessor_source() {
    let text = "interface Value { var property: Boolean get() = true }";
    let mut analysis = analyze_files(&[("src/members.scoop", text)]);
    let property = target(&mut analysis, text, "property:", 8);
    let V::Known(index) = analysis.member_implementation_index(|| false).unwrap() else {
        panic!("known graph");
    };
    assert_eq!(index.implementations.len(), 1);
    assert_eq!(index.implementations[0].declaration, property);
    assert_eq!(
        index.implementations[0].origin,
        at("src/members.scoop", text, "property:", 8)
    );
    assert!(analysis.body_facts.is_empty());
}
