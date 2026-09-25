use scoop_identity::{SourceDeclarationKey, SourceNominalKind};

use super::*;
use crate::DeclarationAccessSourceSemanticError;
use crate::cross_cone_type_semantics::inheritance::tests::support::site;

#[test]
fn source_object_inherits_protected_access_through_its_checked_backing_class_relation() {
    let mut fixture = Fixture::default();
    let base = fixture.add("Base", SourceNominalKind::Class, &[]);
    let object = fixture.add("Singleton", SourceNominalKind::Object, &[]);
    fixture.edges(object, Some(base), &[]);
    let (key, source) = fixture.member(base);
    let constructor = SourceDeclarationKey::constructor(site(&[base.source]), vec![]);
    let checked = source.validate_for_declaration(&key, &mut fixture).unwrap();
    let constructor_checked = source
        .validate_for_declaration(&constructor, &mut fixture)
        .unwrap();
    let graph =
        CheckedNominalInheritanceGraphV1::validate(fixture.records.values(), &fixture).unwrap();
    let relation = graph.object_backing_relation(object.exact).unwrap();
    assert_eq!(relation.source_object(), object.exact);
    assert_ne!(relation.backing_class(), object.exact);
    let target = graph.protected_declaration_domain(&checked).unwrap();
    let implicit = target.implicit_this(object.source).unwrap();
    assert_eq!(implicit.authorization().access_subject(), object.exact);
    assert_eq!(
        implicit.authorization().access_class(),
        relation.backing_class()
    );
    let explicit = target
        .explicit_receiver(object.source, object.exact)
        .unwrap();
    assert_eq!(explicit.authorization(), implicit.authorization());
    assert!(matches!(
        target.explicit_receiver(object.source, base.exact),
        Err(AccessDomainSemanticError::ProtectedReceiver)
    ));
    assert!(
        target
            .explicit_receiver(object.source, relation.backing_class())
            .is_err()
    );
    assert!(
        graph
            .protected_declaration_domain(&constructor_checked)
            .unwrap()
            .constructor_delegation(object.source)
            .is_ok()
    );
}

#[test]
fn object_scope_is_not_a_protected_declaration_owner() {
    let mut fixture = Fixture::default();
    let object = fixture.add("Singleton", SourceNominalKind::Object, &[]);
    let (key, source) = fixture.member(object);
    assert!(matches!(
        source.validate_for_declaration(&key, &mut fixture),
        Err(DeclarationAccessSourceSemanticError::ProtectedOwnerNotClass)
    ));
}
