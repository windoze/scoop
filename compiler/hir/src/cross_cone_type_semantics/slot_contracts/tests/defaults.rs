use super::*;
use scoop_identity::AccessorRole;

#[test]
fn interface_default_targets_preserve_implementation_modality() {
    let mut fixture = Fixture::default();
    let interface = fixture.add("Interface", SourceNominalKind::Interface);
    let slot = fixture.function(interface, "method");
    let mut target = fixture.concrete(interface, slot);
    target.modality = CallableModalityV1::InterfaceDefault;
    let record = fixture.contract(
        interface,
        slot,
        InheritanceSlotImplementationV1::InterfaceDefault(target.clone()),
    );
    assert!(matches!(
        InheritanceSlotContractV1::try_new(
            record.role,
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
            record.role,
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
            InheritanceSlotSchemaRoleV1::ClassVtable,
            slot,
            fixture.declaration(slot),
            fixture.signature(owner, vec![]),
            InheritanceSlotImplementationV1::Concrete(target),
            fixture.access(owner, DeclaredVisibilityV1::Public)
        ),
        Err(InheritanceSlotContractBuildError::DeclarationRole)
    ));
}
