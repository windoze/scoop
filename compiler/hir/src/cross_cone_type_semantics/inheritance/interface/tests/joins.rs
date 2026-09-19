use super::*;

#[test]
fn slot_selection_is_compared_with_the_actual_resolved_implementation() {
    let mut bundle = fixture();
    bundle.fixture.inheritance_interfaces.selections.insert(
        (bundle.derived.exact, bundle.slot),
        InheritanceSourceSlotSelectionV1::Concrete(bundle.root),
    );
    assert!(matches!(
        bundle.validate(),
        Err(InheritanceInterfaceSemanticError::SlotSelection)
    ));
}

#[test]
fn full_source_signature_and_target_modality_are_independent_join_inputs() {
    let mut bundle = fixture();
    bundle
        .fixture
        .inheritance_interfaces
        .callables
        .get_mut(&bundle.target)
        .unwrap()
        .1 = CallableModalityV1::Open;
    assert!(matches!(
        bundle.validate(),
        Err(InheritanceInterfaceSemanticError::SourceContract)
    ));
    let mut bundle = fixture();
    let (signature, _, _) = bundle
        .fixture
        .inheritance_interfaces
        .callables
        .get_mut(&bundle.root)
        .unwrap();
    let old = signature.effects();
    *signature = InheritanceCallableSignatureV1::try_new(
        signature.exact_signature().clone(),
        CallableSourceEffectsV1::try_new(
            old.execution(),
            CallableSafetyV1::Unsafe,
            old.gc_effect(),
            old.implementation(),
            old.operator_role(),
            old.infix(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(
        bundle.validate(),
        Err(InheritanceInterfaceSemanticError::SourceContract)
    ));
}

#[test]
fn protected_constructor_and_members_require_the_checked_source_table() {
    for remove_constructor in [false, true] {
        let mut bundle = fixture();
        let records = bundle
            .protected
            .records()
            .iter()
            .filter(|record| {
                matches!(record, ProtectedDeclarationInterfaceV1::Constructor(_))
                    != remove_constructor
            })
            .cloned()
            .collect::<Vec<_>>();
        bundle.fixture.protected_roots = CanonicalProtectedDeclarationRefsV1::try_new(
            records
                .iter()
                .map(ProtectedDeclarationInterfaceV1::reference)
                .collect(),
        )
        .unwrap();
        bundle.protected = CanonicalProtectedDeclarationInterfacesV1::try_new(records).unwrap();
        assert!(matches!(
            bundle.validate(),
            Err(InheritanceInterfaceSemanticError::ProtectedDeclaration)
        ));
    }
}

#[test]
fn constructor_source_and_nominal_domains_cannot_be_forged_by_the_surface() {
    let mut bundle = fixture();
    let base_ctor = bundle
        .table
        .get(bundle.base.exact)
        .unwrap()
        .constructors()
        .records()[0]
        .clone();
    bundle.change(bundle.derived, |record| {
        record.constructors =
            CanonicalInheritanceConstructorsV1::try_new(vec![base_ctor.clone()]).unwrap()
    });
    bundle.fixture.inheritance_interfaces.constructors.insert(
        bundle.derived.exact,
        CanonicalPersistentIdsV1::try_new(vec![base_ctor.declaration()]).unwrap(),
    );
    assert!(matches!(
        bundle.validate(),
        Err(InheritanceInterfaceSemanticError::ConstructorOwner)
    ));
    let mut bundle = fixture();
    bundle.change(bundle.base, |record| {
        record.domains = NominalAccessDomainsV1::new(
            record.domains.lookup().clone(),
            PersistentInheritanceDomainV1::new(PersistentAccessDomainV1::universal()),
            record.domains.slot().clone(),
        )
    });
    assert!(matches!(
        bundle.validate(),
        Err(InheritanceInterfaceSemanticError::Domains(
            AccessDomainSemanticError::NominalDomains
        ))
    ));
}
