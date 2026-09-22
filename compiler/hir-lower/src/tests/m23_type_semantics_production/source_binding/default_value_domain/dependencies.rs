use super::super::default_type_domain::dependencies::support::artifacts;
use super::*;

#[test]
fn default_value_domains_route_same_named_targets_to_their_actual_providers() {
    let core = trusted_core();
    let (artifacts, identities) = artifacts(&core);
    let foundations = artifacts.each_ref().map(|a| {
        a.source
            .bind_to_foundation(&a.foundation, &identities, &mut meter())
            .unwrap()
    });
    let declarations = std::array::from_fn::<_, 3, _>(|i| {
        foundations[i]
            .bind_default_access_declarations(
                &artifacts[i].table,
                &artifacts[i].required,
                &mut meter(),
            )
            .unwrap()
    });
    let core_inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
    let core_types = core_inputs.protocols().fundamental_types();
    let mut dependencies = [&declarations[1], &declarations[2]];
    dependencies.sort_by_key(|d| d.provider());
    let domains = Domains::new(&declarations[0], &dependencies, core_types, &mut meter()).unwrap();
    let missing = Domains::new(&declarations[0], &[], core_types, &mut meter()).unwrap();
    let mut targets = BTreeSet::new();
    for (i, artifact) in artifacts.iter().enumerate() {
        let mut kinds = BTreeSet::new();
        for template in artifact.defaults.records() {
            let closure = template
                .bind_reference_occurrences(&mut meter(), &scoop_wire::WirePath::root())
                .unwrap();
            for occurrence in closure.occurrences() {
                let record = occurrence.source();
                let Some(target) = target(record) else {
                    continue;
                };
                let actual = domains.value_source_domain(target, &mut meter()).unwrap();
                assert_eq!(&actual, record.witness().target_domain());
                let provider = artifact.coordinate.identity().unwrap();
                assert!(
                    actual
                        .persistent()
                        .constraints()
                        .contains(&hir::PersistentAccessConstraintV1::Cone(provider))
                );
                kinds.insert(format!("{:?}", record.kind()));
                targets.insert(format!("{target:?}"));
                if i != 0 {
                    assert!(
                        matches!(missing.value_source_domain(target, &mut meter()), Err(hir::DefaultSourceDomainError::MissingProvider(actual)) if actual == provider)
                    );
                }
            }
        }
        assert_eq!(kinds.len(), 4);
    }
    assert_eq!(targets.len(), 12);
}
