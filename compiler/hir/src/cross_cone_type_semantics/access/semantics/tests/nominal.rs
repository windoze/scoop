use super::*;
use crate::NominalInheritanceModalityV1;

#[test]
fn nominal_access_queries_respect_modality_and_lexical_visibility() {
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
