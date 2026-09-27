use super::*;
use scoop_identity::ConeIdentity;

#[test]
fn protected_override_preserves_only_the_inherited_protected_slot_region() {
    let mut fixture = Fixture::default();
    let base = fixture.add("Base", SourceNominalKind::Class);
    let derived = fixture.add("Derived", SourceNominalKind::Class);
    fixture.inheritance.edges(derived, Some(base), &[]);
    let slot = fixture.function(base, "method");
    let target_slot = fixture.function(derived, "method");
    fixture.schema(base, &[slot]);
    fixture.schema(derived, &[slot]);
    let mut target = fixture.concrete(derived, target_slot);
    target.declaration_access = fixture.access(derived, DeclaredVisibilityV1::Protected);
    let mut record = fixture.contract(
        base,
        slot,
        InheritanceSlotImplementationV1::Concrete(target),
    );
    record.declaration_access = fixture.access(base, DeclaredVisibilityV1::Protected);
    record.domain = PersistentSlotContractDomainV1::new(
        PersistentAccessDomainV1::try_from_constraints(vec![
            PersistentAccessConstraintV1::SubclassesOf(base.exact),
        ])
        .unwrap(),
    );
    let graph =
        CheckedNominalInheritanceGraphV1::validate(fixture.inheritance.records.values(), &fixture)
            .unwrap();
    graph
        .validate_slot_contract(derived.exact, &record, &fixture)
        .unwrap();
    record.declaration_access = fixture.access(base, DeclaredVisibilityV1::Public);
    record.domain = PersistentSlotContractDomainV1::new(PersistentAccessDomainV1::universal());
    assert!(matches!(
        graph.validate_slot_contract(derived.exact, &record, &fixture),
        Err(InheritanceSlotContractSemanticError::Domain)
    ));
}

#[test]
fn override_access_covers_root_domain_without_exporting_its_hidden_owner() {
    let mut fixture = Fixture::default();
    let base = fixture.add("Base", SourceNominalKind::Class);
    let hidden = fixture.add("Hidden", SourceNominalKind::Class);
    fixture
        .inheritance
        .visibility(hidden, DeclaredVisibilityV1::Internal);
    fixture.inheritance.edges(hidden, Some(base), &[]);
    let slot = fixture.function(base, "method");
    let target_slot = fixture.function(hidden, "method");
    fixture.schema(base, &[slot]);
    fixture.schema(hidden, &[slot]);
    let mut record = fixture.contract(
        base,
        slot,
        InheritanceSlotImplementationV1::Concrete(fixture.concrete(hidden, target_slot)),
    );
    let graph =
        CheckedNominalInheritanceGraphV1::validate(fixture.inheritance.records.values(), &fixture)
            .unwrap();
    graph
        .validate_slot_contract(hidden.exact, &record, &fixture)
        .unwrap();
    assert!(
        !graph
            .replay_nominal_domains(hidden.exact)
            .unwrap()
            .lookup()
            .domain()
            .is_universal()
    );
    let InheritanceSlotImplementationV1::Concrete(target) = &mut record.implementation else {
        unreachable!()
    };
    target.declaration_access = fixture.access(hidden, DeclaredVisibilityV1::Internal);
    assert!(matches!(
        graph.validate_slot_contract(hidden.exact, &record, &fixture),
        Err(InheritanceSlotContractSemanticError::Domain)
    ));
    let InheritanceSlotImplementationV1::Concrete(target) = &mut record.implementation else {
        unreachable!()
    };
    target.declaration_access = fixture.access(hidden, DeclaredVisibilityV1::Private);
    assert!(matches!(
        graph.validate_slot_contract(hidden.exact, &record, &fixture),
        Err(InheritanceSlotContractSemanticError::PrivateDeclaration)
    ));
}

#[test]
fn abstract_obligations_must_be_reachable_and_implementable_by_the_owner() {
    let mut fixture = Fixture::default();
    let hidden = fixture.add("Hidden", SourceNominalKind::Class);
    let public = fixture.add("Public", SourceNominalKind::Class);
    fixture
        .inheritance
        .modality(hidden, NominalInheritanceModalityV1::Abstract);
    fixture
        .inheritance
        .modality(public, NominalInheritanceModalityV1::Abstract);
    fixture
        .inheritance
        .visibility(hidden, DeclaredVisibilityV1::Internal);
    fixture.inheritance.edges(public, Some(hidden), &[]);
    let slot = fixture.function(hidden, "method");
    fixture.schema(hidden, &[slot]);
    fixture.schema(public, &[slot]);
    let mut record = fixture.contract(
        hidden,
        slot,
        InheritanceSlotImplementationV1::Abstract(fixture.abstract_target(hidden, slot)),
    );
    record.domain = PersistentSlotContractDomainV1::new(
        PersistentAccessDomainV1::try_from_constraints(vec![PersistentAccessConstraintV1::Cone(
            ConeIdentity::CORE,
        )])
        .unwrap(),
    );
    let graph =
        CheckedNominalInheritanceGraphV1::validate(fixture.inheritance.records.values(), &fixture)
            .unwrap();
    graph
        .validate_slot_contract(hidden.exact, &record, &fixture)
        .unwrap();
    assert!(matches!(
        graph.validate_slot_contract(public.exact, &record, &fixture),
        Err(InheritanceSlotContractSemanticError::AbstractObligation)
    ));
    record.domain = PersistentSlotContractDomainV1::new(PersistentAccessDomainV1::universal());
    assert!(matches!(
        graph.validate_slot_contract(hidden.exact, &record, &fixture),
        Err(InheritanceSlotContractSemanticError::Domain)
    ));
}

#[test]
fn protected_root_domain_preserves_typed_target_identity() {
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
    record.domain = PersistentSlotContractDomainV1::new(
        PersistentAccessDomainV1::try_from_constraints(vec![
            PersistentAccessConstraintV1::SubclassesOf(base.exact),
        ])
        .unwrap(),
    );
    let graph =
        CheckedNominalInheritanceGraphV1::validate(fixture.inheritance.records.values(), &fixture)
            .unwrap();
    graph
        .validate_slot_contract(derived.exact, &record, &fixture)
        .unwrap();
    record.implementation =
        InheritanceSlotImplementationV1::Concrete(fixture.concrete(derived, target_slot));
    assert!(matches!(
        graph.validate_slot_contract(derived.exact, &record, &fixture),
        Err(InheritanceSlotContractSemanticError::TargetName)
    ));
}
