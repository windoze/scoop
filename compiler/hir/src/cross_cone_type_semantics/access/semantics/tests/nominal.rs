use super::*;
use crate::{
    NominalAccessDomainsV1, NominalInheritanceModalityV1, PersistentInheritanceDomainV1,
    PersistentLookupDomainV1, PersistentSlotContractDomainV1,
};

#[test]
fn nominal_regions_replay_modality_and_never_acquire_a_callable_slot() {
    let mut fixture = Fixture::default();
    let outer = fixture.add("Outer", SourceNominalKind::Class, &[]);
    fixture.visibility(outer, DeclaredVisibilityV1::Internal);
    let open = fixture.add("Open", SourceNominalKind::Class, &[outer]);
    let final_class = fixture.add("Final", SourceNominalKind::Class, &[]);
    fixture.modality(final_class, NominalInheritanceModalityV1::Final);
    let interface = fixture.add("Interface", SourceNominalKind::Interface, &[]);
    let value = fixture.add("Value", SourceNominalKind::Struct, &[]);
    let object = fixture.add("Object", SourceNominalKind::Object, &[]);
    let graph =
        CheckedNominalInheritanceGraphV1::validate(fixture.records.values(), &fixture).unwrap();
    let domains = graph.replay_nominal_domains(open.exact).unwrap();
    assert_eq!(
        domains.lookup().domain(),
        &domain(vec![Constraint::Cone(ConeIdentity::CORE)])
    );
    assert_eq!(
        domains.inheritance().domain(),
        &domain(vec![
            Constraint::Cone(ConeIdentity::CORE),
            Constraint::SubclassesOf(open.exact)
        ])
    );
    assert!(domains.to_record().slot().domain().is_empty());
    graph
        .validate_nominal_domains(open.exact, &domains.to_record())
        .unwrap();
    let tampered = NominalAccessDomainsV1::new(
        PersistentLookupDomainV1::new(domains.lookup().domain().clone()),
        PersistentInheritanceDomainV1::new(domains.inheritance().domain().clone()),
        PersistentSlotContractDomainV1::new(PersistentAccessDomainV1::universal()),
    );
    assert!(matches!(
        graph.validate_nominal_domains(open.exact, &tampered),
        Err(AccessDomainSemanticError::NominalDomains)
    ));
    for node in [final_class, value, object] {
        assert!(
            graph
                .replay_nominal_domains(node.exact)
                .unwrap()
                .inheritance()
                .domain()
                .is_empty()
        );
    }
    assert!(
        graph
            .replay_nominal_domains(interface.exact)
            .unwrap()
            .inheritance()
            .domain()
            .is_universal()
    );
}
