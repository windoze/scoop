use scoop_identity::{ConeIdentity, SourceNominalKind};

use super::*;
use crate::cross_cone_type_semantics::inheritance::tests::support::Fixture;

mod nominal;
mod objects;
mod protected;

fn domain(constraints: Vec<Constraint>) -> PersistentAccessDomainV1 {
    PersistentAccessDomainV1::try_from_constraints(constraints).unwrap()
}

#[test]
fn nominal_lookup_replays_all_owner_domains_without_widening_declared_visibility() {
    let mut fixture = Fixture::default();
    let outer = fixture.add("Outer", SourceNominalKind::Class, &[]);
    fixture.visibility(outer, DeclaredVisibilityV1::Internal);
    let inner = fixture.add("Inner", SourceNominalKind::Class, &[outer]);
    let graph =
        CheckedNominalInheritanceGraphV1::validate(fixture.records.values(), &fixture).unwrap();
    let access = graph.replay_nominal_access(inner.source).unwrap();
    assert!(access.declared().domain().is_universal());
    assert_eq!(
        access.lookup().domain(),
        &domain(vec![Constraint::Cone(ConeIdentity::CORE)])
    );
}

#[test]
fn domains_reject_contradictory_wire_and_normalize_proven_intersection_to_empty() {
    let mut fixture = Fixture::default();
    let left = fixture.add("Left", SourceNominalKind::Class, &[]);
    let right = fixture.add("Right", SourceNominalKind::Class, &[]);
    let graph =
        CheckedNominalInheritanceGraphV1::validate(fixture.records.values(), &fixture).unwrap();
    let bad = domain(vec![
        Constraint::Cone(ConeIdentity::CORE),
        Constraint::Cone(ConeIdentity::SINGLE_FILE),
    ]);
    assert!(matches!(
        graph.validate_access_domain(&bad),
        Err(AccessDomainSemanticError::NonCanonicalEmpty)
    ));
    let left = graph
        .validate_access_domain(&domain(vec![Constraint::LexicalOwner(left.source)]))
        .unwrap();
    let right = graph
        .validate_access_domain(&domain(vec![Constraint::LexicalOwner(right.source)]))
        .unwrap();
    assert!(left.intersect(&right).unwrap().domain().is_empty());
    let impossible = graph
        .validate_access_domain(&PersistentAccessDomainV1::empty())
        .unwrap();
    assert!(left.covers(&impossible).unwrap());
    assert!(!impossible.covers(&left).unwrap());
}

#[test]
fn nested_scopes_can_satisfy_two_unrelated_subclass_constraints() {
    let mut fixture = Fixture::default();
    let base_a = fixture.add("A", SourceNominalKind::Class, &[]);
    let base_b = fixture.add("B", SourceNominalKind::Class, &[]);
    let outer = fixture.add("Outer", SourceNominalKind::Class, &[]);
    fixture.edges(outer, Some(base_a), &[]);
    let inner = fixture.add("Inner", SourceNominalKind::Class, &[outer]);
    fixture.edges(inner, Some(base_b), &[]);
    let graph =
        CheckedNominalInheritanceGraphV1::validate(fixture.records.values(), &fixture).unwrap();
    let both = graph
        .validate_access_domain(&domain(vec![
            Constraint::SubclassesOf(base_a.exact),
            Constraint::SubclassesOf(base_b.exact),
        ]))
        .unwrap();
    assert!(!both.domain().is_empty());
    assert!(both.allows_scope(inner.source).unwrap());
    assert!(!both.allows_scope(outer.source).unwrap());
    let lexical = graph
        .validate_access_domain(&domain(vec![Constraint::LexicalOwner(inner.source)]))
        .unwrap();
    assert!(both.covers(&lexical).unwrap());
}

#[test]
fn coverage_replays_lexical_file_cone_and_subclass_implications() {
    let mut fixture = Fixture::default();
    let base = fixture.add("Base", SourceNominalKind::Class, &[]);
    let derived = fixture.add("Derived", SourceNominalKind::Class, &[]);
    fixture.edges(derived, Some(base), &[]);
    let nested = fixture.add("Nested", SourceNominalKind::Struct, &[derived]);
    let graph =
        CheckedNominalInheritanceGraphV1::validate(fixture.records.values(), &fixture).unwrap();
    let narrow = graph
        .validate_access_domain(&domain(vec![Constraint::LexicalOwner(nested.source)]))
        .unwrap();
    for constraint in [
        Constraint::Cone(ConeIdentity::CORE),
        Constraint::File(fixture.origins[&nested.source].origin().source().clone()),
        Constraint::LexicalOwner(derived.source),
        Constraint::SubclassesOf(base.exact),
    ] {
        let wider = graph
            .validate_access_domain(&domain(vec![constraint]))
            .unwrap();
        assert!(wider.covers(&narrow).unwrap());
        assert!(!narrow.covers(&wider).unwrap());
    }
    let base_domain = graph
        .validate_access_domain(&domain(vec![Constraint::SubclassesOf(base.exact)]))
        .unwrap();
    let derived_domain = graph
        .validate_access_domain(&domain(vec![Constraint::SubclassesOf(derived.exact)]))
        .unwrap();
    assert!(base_domain.covers(&derived_domain).unwrap());
    assert!(!derived_domain.covers(&base_domain).unwrap());
}
