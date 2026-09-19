use super::*;
use scoop_identity::AccessorRole;

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
    let graph = CheckedNominalInheritanceGraphV1::validate(
        fixture.inheritance.records.values(),
        &fixture,
        &mut meter(),
    )
    .unwrap();
    graph
        .validate_slot_contract(owner.exact, &record, &fixture, &mut meter())
        .unwrap();
    assert!(matches!(
        InheritanceSlotContractV1::try_new(
            record.slot,
            record.declaration_owner,
            record.declaration,
            record.signature.clone(),
            record.domain.clone(),
            InheritanceSlotImplementationV1::Concrete(target),
            record.declaration_access.clone()
        ),
        Err(InheritanceSlotContractBuildError::DefaultModality)
    ));
    assert!(matches!(
        graph.validate_slot_contract(
            owner.exact,
            &record,
            &fixture,
            &mut BudgetMeter::new(DecodeLimits {
                validation_work_units: 0,
                ..DecodeLimits::default()
            })
        ),
        Err(InheritanceSlotContractSemanticError::Schema(
            InheritanceSlotSchemaSemanticError::Resource(_)
        ))
    ));
}

#[test]
fn targets_cannot_be_abstract_or_cross_callable_roles() {
    let mut fixture = Fixture::default();
    let owner = fixture.add("Owner", SourceNominalKind::Class);
    let slot = fixture.function(owner, "method");
    assert!(matches!(
        InheritanceSlotTargetV1::try_new(
            fixture.declaration(slot),
            support::nominal(owner),
            fixture.signature(owner, vec![]),
            CallableModalityV1::Abstract,
            fixture.access(owner, DeclaredVisibilityV1::Public)
        ),
        Err(InheritanceSlotContractBuildError::AbstractTarget)
    ));
    let getter = fixture.accessor(owner, "property", AccessorRole::Getter);
    let target = fixture.concrete(owner, getter);
    assert!(matches!(
        InheritanceSlotContractV1::try_new(
            slot,
            support::nominal(owner),
            fixture.declaration(slot),
            fixture.signature(owner, vec![]),
            PersistentSlotContractDomainV1::new(PersistentAccessDomainV1::universal()),
            InheritanceSlotImplementationV1::Concrete(target),
            fixture.access(owner, DeclaredVisibilityV1::Public)
        ),
        Err(InheritanceSlotContractBuildError::DeclarationRole)
    ));
}
