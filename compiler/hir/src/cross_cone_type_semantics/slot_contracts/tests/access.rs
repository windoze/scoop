use super::*;

#[test]
fn protected_root_preserves_source_access_and_typed_target_identity() {
    let mut fixture = Fixture::default();
    let base = fixture.add("Base", SourceNominalKind::Class);
    let derived = fixture.add("Derived", SourceNominalKind::Class);
    fixture.inheritance.edges(derived, Some(base), &[]);
    let slot = fixture.function(base, "method");
    let target_slot = fixture.function(derived, "different");
    fixture.schema(base, &[slot]);
    fixture.schema(derived, &[slot]);
    let mut record = fixture.contract(
        base,
        slot,
        InheritanceSlotImplementationV1::Concrete(fixture.concrete(base, slot)),
    );
    record.declaration_access = fixture.access(base, DeclaredVisibilityV1::Protected);
    let graph =
        CheckedNominalInheritanceGraphV1::validate(fixture.inheritance.records.values(), &fixture)
            .unwrap();
    graph
        .validate_slot_contract(derived.exact, &record, &fixture)
        .unwrap();
    assert_eq!(record.declaration_access().lexical_owners(), &[base.source]);
    record.implementation =
        InheritanceSlotImplementationV1::Concrete(fixture.concrete(derived, target_slot));
    assert!(matches!(
        graph.validate_slot_contract(derived.exact, &record, &fixture),
        Err(InheritanceSlotContractSemanticError::TargetName)
    ));
}

#[test]
fn private_declarations_cannot_be_dispatch_targets() {
    let mut fixture = Fixture::default();
    let base = fixture.add("Base", SourceNominalKind::Class);
    let derived = fixture.add("Derived", SourceNominalKind::Class);
    fixture.inheritance.edges(derived, Some(base), &[]);
    let slot = fixture.function(base, "method");
    let target_slot = fixture.function(derived, "method");
    fixture.schema(base, &[slot]);
    fixture.schema(derived, &[slot]);
    let mut target = fixture.concrete(derived, target_slot);
    target.declaration_access = fixture.access(derived, DeclaredVisibilityV1::Private);
    let record = fixture.contract(
        base,
        slot,
        InheritanceSlotImplementationV1::Concrete(target),
    );
    let graph =
        CheckedNominalInheritanceGraphV1::validate(fixture.inheritance.records.values(), &fixture)
            .unwrap();
    assert!(matches!(
        graph.validate_slot_contract(derived.exact, &record, &fixture),
        Err(InheritanceSlotContractSemanticError::PrivateDeclaration)
    ));
}
