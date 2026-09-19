use scoop_identity::{CanonicalIdentifier, SourceDeclarationKey, SourceNominalKind};

use super::*;
use crate::cross_cone_type_semantics::inheritance::tests::support::site;

#[test]
fn protected_explicit_receivers_require_the_actual_lexical_access_subclass() {
    let mut fixture = Fixture::default();
    let base = fixture.add("Base", SourceNominalKind::Class, &[]);
    let derived = fixture.add("Derived", SourceNominalKind::Class, &[]);
    let sibling = fixture.add("Sibling", SourceNominalKind::Class, &[]);
    fixture.edges(derived, Some(base), &[]);
    fixture.edges(sibling, Some(base), &[]);
    let (key, source) = fixture.member(base);
    let checked = source.validate_for_declaration(&key, &mut fixture).unwrap();
    let graph = CheckedNominalInheritanceGraphV1::validate(
        fixture.records.values(),
        &fixture,
        &mut meter(),
    )
    .unwrap();
    let target = graph
        .protected_declaration_domain(&checked, &mut meter())
        .unwrap();
    let witness = target
        .explicit_receiver(derived.source, derived.exact, &mut meter())
        .unwrap();
    assert_eq!(witness.authorization().access_class(), derived.exact);
    assert_eq!(witness.receiver(), derived.exact);
    assert!(matches!(
        target.explicit_receiver(derived.source, base.exact, &mut meter()),
        Err(AccessDomainSemanticError::ProtectedReceiver)
    ));
    assert!(matches!(
        target.explicit_receiver(derived.source, sibling.exact, &mut meter()),
        Err(AccessDomainSemanticError::ProtectedReceiver)
    ));
    assert_eq!(
        target
            .implicit_this(derived.source, &mut meter())
            .unwrap()
            .authorization()
            .access_class(),
        derived.exact
    );
}

#[test]
fn nested_and_companion_scopes_can_use_explicit_outer_subclass_but_gain_no_implicit_outer_this() {
    let mut fixture = Fixture::default();
    let base = fixture.add("Base", SourceNominalKind::Class, &[]);
    let derived = fixture.add("Derived", SourceNominalKind::Class, &[]);
    fixture.edges(derived, Some(base), &[]);
    let nested = fixture.add("Nested", SourceNominalKind::Class, &[derived]);
    let companion = fixture.add("Companion", SourceNominalKind::Object, &[derived]);
    let (key, source) = fixture.member(base);
    let checked = source.validate_for_declaration(&key, &mut fixture).unwrap();
    let graph = CheckedNominalInheritanceGraphV1::validate(
        fixture.records.values(),
        &fixture,
        &mut meter(),
    )
    .unwrap();
    let target = graph
        .protected_declaration_domain(&checked, &mut meter())
        .unwrap();
    for scope in [nested, companion] {
        let access = target
            .explicit_receiver(scope.source, derived.exact, &mut meter())
            .unwrap();
        assert_eq!(access.authorization().access_class(), derived.exact);
        assert!(target.implicit_this(scope.source, &mut meter()).is_err());
    }
}

#[test]
fn protected_constructor_and_super_witnesses_check_direct_edges_and_declaration_purpose() {
    let mut fixture = Fixture::default();
    let base = fixture.add("Base", SourceNominalKind::Class, &[]);
    let derived = fixture.add("Derived", SourceNominalKind::Class, &[]);
    let grandchild = fixture.add("Grandchild", SourceNominalKind::Class, &[]);
    fixture.edges(derived, Some(base), &[]);
    fixture.edges(grandchild, Some(derived), &[]);
    let (member_key, source) = fixture.member(base);
    let constructor = SourceDeclarationKey::constructor(site(&[base.source]), vec![]);
    let member_checked = source
        .validate_for_declaration(&member_key, &mut fixture)
        .unwrap();
    let constructor_checked = source
        .validate_for_declaration(&constructor, &mut fixture)
        .unwrap();
    let graph = CheckedNominalInheritanceGraphV1::validate(
        fixture.records.values(),
        &fixture,
        &mut meter(),
    )
    .unwrap();
    let member = graph
        .protected_declaration_domain(&member_checked, &mut meter())
        .unwrap();
    let constructor = graph
        .protected_declaration_domain(&constructor_checked, &mut meter())
        .unwrap();
    assert!(
        constructor
            .constructor_delegation(derived.source, &mut meter())
            .is_ok()
    );
    assert!(matches!(
        constructor.constructor_delegation(grandchild.source, &mut meter()),
        Err(AccessDomainSemanticError::InvalidDelegation)
    ));
    assert!(matches!(
        member.constructor_delegation(derived.source, &mut meter()),
        Err(AccessDomainSemanticError::InvalidPurpose)
    ));
    assert!(
        member
            .qualified_super(grandchild.source, derived.exact, &mut meter())
            .is_ok()
    );
    assert!(matches!(
        member.qualified_super(grandchild.source, base.exact, &mut meter()),
        Err(AccessDomainSemanticError::InvalidSuper)
    ));
    assert!(matches!(
        constructor.explicit_receiver(derived.source, derived.exact, &mut meter()),
        Err(AccessDomainSemanticError::InvalidPurpose)
    ));
}

#[test]
fn protected_nested_type_requires_a_nominal_target_and_effective_owner_domain() {
    let mut fixture = Fixture::default();
    let outer = fixture.add("Outer", SourceNominalKind::Class, &[]);
    let hidden = fixture.add("Hidden", SourceNominalKind::Class, &[outer]);
    fixture.visibility(hidden, DeclaredVisibilityV1::Private);
    let base = fixture.add("Base", SourceNominalKind::Class, &[outer, hidden]);
    let derived = fixture.add("Derived", SourceNominalKind::Class, &[]);
    fixture.edges(derived, Some(base), &[]);
    let (_, source) = fixture.member(base);
    let key = SourceDeclarationKey::nominal(
        site(&[outer.source, hidden.source, base.source]),
        CanonicalIdentifier::new("Nested").unwrap(),
        SourceNominalKind::Struct,
        0,
    );
    let checked = source.validate_for_declaration(&key, &mut fixture).unwrap();
    let graph = CheckedNominalInheritanceGraphV1::validate(
        fixture.records.values(),
        &fixture,
        &mut meter(),
    )
    .unwrap();
    let target = graph
        .protected_declaration_domain(&checked, &mut meter())
        .unwrap();
    assert!(matches!(
        target.nested_type(derived.source, &mut meter()),
        Err(AccessDomainSemanticError::OutsideDomain)
    ));
    assert!(matches!(
        target.explicit_receiver(derived.source, derived.exact, &mut meter()),
        Err(AccessDomainSemanticError::InvalidPurpose)
    ));
}
