use super::*;

#[test]
fn target_source_package_must_match_its_canonical_nominal_owner() {
    use scoop_identity::{
        CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerAtom,
        DefinitionOwnerChain, PackagePath, PersistentFunctionId, SourceDeclarationKey,
        SourceDeclarationSite,
    };
    let mut fixture = Fixture::default();
    let owner = fixture.add("Owner", SourceNominalKind::Class);
    let slot = fixture.function(owner, "method");
    fixture.schema(owner, &[slot]);
    let site = SourceDeclarationSite::new(
        ConeIdentity::CORE,
        PackagePath::from_segments(vec![CanonicalIdentifier::new("foreign").unwrap()]),
        DefinitionOwnerChain::from_outer_to_inner(vec![DefinitionOwnerAtom::Type(
            support::nominal(owner),
        )]),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let key = SourceDeclarationKey::function(
        site,
        CanonicalIdentifier::new("method").unwrap(),
        0,
        None,
        vec![],
    );
    let function = PersistentFunctionId::from_source_declaration(&key).unwrap();
    fixture.functions.insert(function, key);
    let mut target = fixture.concrete(owner, slot);
    target.declaration = InheritanceCallableDeclarationV1::Function(function);
    let record = fixture.contract(
        owner,
        slot,
        InheritanceSlotImplementationV1::Concrete(target),
    );
    let graph =
        CheckedNominalInheritanceGraphV1::validate(fixture.inheritance.records.values(), &fixture)
            .unwrap();
    assert!(matches!(
        graph.validate_slot_contract(owner.exact, &record, &fixture),
        Err(InheritanceSlotContractSemanticError::DeclarationIdentity(_))
    ));
}
use scoop_identity::{AccessorRole, SignatureTypeKey};

#[test]
fn inherited_slot_keeps_root_identity_and_joins_the_derived_receiver_and_source() {
    let mut fixture = Fixture::default();
    let base = fixture.add("Base", SourceNominalKind::Class);
    let derived = fixture.add("Derived", SourceNominalKind::Class);
    fixture.inheritance.edges(derived, Some(base), &[]);
    let slot = fixture.function(base, "method");
    let override_slot = fixture.function(derived, "method");
    fixture.schema(base, &[slot]);
    fixture.schema(derived, &[slot]);
    let mut record = fixture.contract(
        base,
        slot,
        InheritanceSlotImplementationV1::Concrete(fixture.concrete(derived, override_slot)),
    );
    let graph =
        CheckedNominalInheritanceGraphV1::validate(fixture.inheritance.records.values(), &fixture)
            .unwrap();
    let checked = graph
        .validate_slot_contract(derived.exact, &record, &fixture)
        .unwrap();
    assert_eq!(checked.record().slot(), slot);
    assert_eq!(
        checked
            .record()
            .implementation()
            .target()
            .unwrap()
            .signature()
            .exact_signature()
            .receiver()
            .into_option(),
        Some(derived.exact)
    );
    record.declaration = fixture.declaration(override_slot);
    assert!(matches!(
        graph.validate_slot_contract(derived.exact, &record, &fixture),
        Err(InheritanceSlotContractSemanticError::DeclarationIdentity(_))
    ));
}

#[test]
fn parameters_accessors_and_receiver_owners_are_replayed_from_foundation() {
    let mut fixture = Fixture::default();
    let owner = fixture.add("Owner", SourceNominalKind::Class);
    let value = fixture.add("Value", SourceNominalKind::Struct);
    let slot = fixture.method(
        owner,
        "method",
        vec![SignatureTypeKey::Nominal(support::nominal(value))],
    );
    fixture.schema(owner, &[slot]);
    let mut record = fixture.contract(owner, slot, InheritanceSlotImplementationV1::Abstract);
    fixture
        .inheritance
        .modality(owner, NominalInheritanceModalityV1::Abstract);
    let graph =
        CheckedNominalInheritanceGraphV1::validate(fixture.inheritance.records.values(), &fixture)
            .unwrap();
    assert!(matches!(
        graph.validate_slot_contract(owner.exact, &record, &fixture),
        Err(InheritanceSlotContractSemanticError::Signature)
    ));
    record.signature = fixture.signature(owner, vec![value.exact]);
    graph
        .validate_slot_contract(owner.exact, &record, &fixture)
        .unwrap();
    record.signature = fixture.signature(value, vec![value.exact]);
    assert!(matches!(
        graph.validate_slot_contract(owner.exact, &record, &fixture),
        Err(InheritanceSlotContractSemanticError::ReceiverOwner)
    ));

    let getter = fixture.accessor(owner, "property", AccessorRole::Getter);
    let setter = fixture.accessor(owner, "property", AccessorRole::Setter);
    fixture.schema(owner, &[slot, getter, setter]);
    let graph =
        CheckedNominalInheritanceGraphV1::validate(fixture.inheritance.records.values(), &fixture)
            .unwrap();
    let mut record = fixture.contract(owner, getter, InheritanceSlotImplementationV1::Abstract);
    graph
        .validate_slot_contract(owner.exact, &record, &fixture)
        .unwrap();
    record.signature = fixture.signature(owner, vec![value.exact]);
    assert!(matches!(
        graph.validate_slot_contract(owner.exact, &record, &fixture),
        Err(InheritanceSlotContractSemanticError::Signature)
    ));
    record = fixture.contract(owner, setter, InheritanceSlotImplementationV1::Abstract);
    assert!(matches!(
        graph.validate_slot_contract(owner.exact, &record, &fixture),
        Err(InheritanceSlotContractSemanticError::Signature)
    ));
    record.signature = fixture.signature(owner, vec![value.exact]);
    graph
        .validate_slot_contract(owner.exact, &record, &fixture)
        .unwrap();
}
