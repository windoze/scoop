use super::*;
use scoop_identity::AccessorRole;

#[test]
fn interface_default_target_requires_transitive_conformance_even_without_a_parent_role() {
    let mut fixture = Fixture::default();
    let parent = fixture.add("Parent", SourceNominalKind::Interface);
    let child = fixture.add("Child", SourceNominalKind::Interface);
    let unrelated = fixture.add("Unrelated", SourceNominalKind::Interface);
    fixture.inheritance.edges(child, None, &[parent]);
    fixture.interface_source(child, &[parent], &[]);
    let slot = fixture.function(parent, "method");
    let other_slot = fixture.function(unrelated, "method");
    fixture.schema(parent, &[slot]);
    fixture.schema(child, &[slot]);
    fixture.schema(unrelated, &[other_slot]);
    let mut target = fixture.concrete(parent, slot);
    target.modality = CallableModalityV1::InterfaceDefault;
    let mut record = fixture.contract(
        parent,
        slot,
        InheritanceSlotImplementationV1::InterfaceDefault(target),
    );
    let graph =
        CheckedNominalInheritanceGraphV1::validate(fixture.inheritance.records.values(), &fixture)
            .unwrap();
    let schemas = graph.validate_slot_schemas(child.exact, &fixture).unwrap();
    assert_eq!(schemas.schemas().records().len(), 1);
    assert!(schemas.supports_interface(parent.exact));
    assert!(!schemas.supports_interface(unrelated.exact));
    graph
        .validate_slot_contract(child.exact, &record, &fixture)
        .unwrap();
    let mut unrelated_target = fixture.concrete(unrelated, other_slot);
    unrelated_target.modality = CallableModalityV1::InterfaceDefault;
    record.implementation = InheritanceSlotImplementationV1::InterfaceDefault(unrelated_target);
    assert!(matches!(
        graph.validate_slot_contract(child.exact, &record, &fixture),
        Err(InheritanceSlotContractSemanticError::TargetOwner)
    ));
}

#[test]
fn interface_default_targets_keep_provider_identity_and_conformance() {
    let mut fixture = Fixture::default();
    let interface = fixture.add("Interface", SourceNominalKind::Interface);
    let owner = fixture.add("Value", SourceNominalKind::Struct);
    fixture.inheritance.edges(owner, None, &[interface]);
    let slot = fixture.function(interface, "method");
    fixture.schema(interface, &[slot]);
    fixture.set(
        owner,
        vec![
            InheritanceSlotSchemaV1::try_new(
                InheritanceSlotSchemaRoleV1::Interface {
                    interface_exact: interface.exact,
                },
                vec![slot],
            )
            .unwrap(),
        ],
    );
    let mut target = fixture.concrete(interface, slot);
    target.modality = CallableModalityV1::InterfaceDefault;
    let record = fixture.contract(
        interface,
        slot,
        InheritanceSlotImplementationV1::InterfaceDefault(target.clone()),
    );
    let graph =
        CheckedNominalInheritanceGraphV1::validate(fixture.inheritance.records.values(), &fixture)
            .unwrap();
    graph
        .validate_slot_contract(owner.exact, &record, &fixture)
        .unwrap();
    assert!(matches!(
        InheritanceSlotContractV1::try_new(
            record.slot,
            record.declaration,
            record.signature.clone(),
            InheritanceSlotImplementationV1::Concrete(target),
            record.declaration_access.clone()
        ),
        Err(InheritanceSlotContractBuildError::DefaultModality)
    ));
}

#[test]
fn targets_match_implementation_modality_and_callable_roles() {
    let mut fixture = Fixture::default();
    let owner = fixture.add("Owner", SourceNominalKind::Class);
    let slot = fixture.function(owner, "method");
    let target = fixture.abstract_target(owner, slot);
    let record = fixture.contract(
        owner,
        slot,
        InheritanceSlotImplementationV1::Abstract(target.clone()),
    );
    assert_eq!(record.implementation().target(), &target);
    assert!(matches!(
        InheritanceSlotContractV1::try_new(
            record.slot,
            record.declaration,
            record.signature.clone(),
            InheritanceSlotImplementationV1::Concrete(target),
            record.declaration_access.clone(),
        ),
        Err(InheritanceSlotContractBuildError::AbstractModality)
    ));
    let getter = fixture.accessor(owner, "property", AccessorRole::Getter);
    let target = fixture.concrete(owner, getter);
    assert!(matches!(
        InheritanceSlotContractV1::try_new(
            slot,
            fixture.declaration(slot),
            fixture.signature(owner, vec![]),
            InheritanceSlotImplementationV1::Concrete(target),
            fixture.access(owner, DeclaredVisibilityV1::Public)
        ),
        Err(InheritanceSlotContractBuildError::DeclarationRole)
    ));
}
