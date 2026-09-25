use super::*;
use hir::DefaultSourceAccessDomainV1 as Domain;
mod routes;

#[test]
fn default_value_binding_rejects_each_corrupt_target_witness_without_merging_repeats() {
    for source in [SOURCE, COMBINATIONS] {
        with_inputs(source, |inputs| {
            inputs.with_bound(|domains, parameters, _| {
                let mut kinds = BTreeSet::new();
                let mut rejected = 0;
                for original in inputs.templates.records() {
                    let closure = original.bind_reference_occurrences( &scoop_wire::WirePath::root()).unwrap();
                    for occurrence in closure.occurrences() {
                        let record = occurrence.source();
                        if target(record).is_none() { continue; }
                        let kind = record.kind();
                        let index = occurrence.index();
                        let domain = record.witness().target_domain();
                        let replacement = if domain == &Domain::universal() { Domain::empty() } else { Domain::universal() };
                        let changed = super::super::default_declarations::reference_witness::change_at(
                            original, kind, index as usize, |w| hir::DefaultSourceAccessWitnessV1::try_new(w.owner(), w.direct_call_domain().clone(), w.slot_call_domain().clone(), replacement).unwrap(), inputs.output,
                        );
                        let table = super::super::default_origins::replace(&inputs.templates, changed);
                        let declarations = parameters.bind_default_declarations(&table, &[]).unwrap();
                        let error = domains.bind_nominal_default_value_domains(&declarations).unwrap_err();
                        assert!(matches!(error, Error::Witness { key, kind: actual_kind, index: actual_index } if key == original.key() && actual_kind == kind && actual_index == index), "{error:?}");
                        kinds.insert(format!("{kind:?}"));
                        rejected += 1;
                    }
                }
                assert_eq!(kinds.len(), 4);
                assert!(rejected > 8);
            });
        });
    }
}

#[test]
fn default_value_binding_rejects_another_foundation_and_missing_access_records() {
    with_inputs(SOURCE, |inputs| {
        inputs.with_bound(|_, parameters, foundation| {
            let declarations = parameters.bind_default_declarations(&inputs.templates, &[]).unwrap();
            let other = inputs.fixture.bind().unwrap();
            let access = other.bind_default_access_declarations(&inputs.access, &inputs.required).unwrap();
            let domains = Domains::new(&access, &[], inputs.core_types).unwrap();
            assert!(matches!(domains.bind_nominal_default_value_domains(&declarations), Err(Error::Foundation { expected, actual }) if expected == actual));
            let empty = hir::CanonicalDefaultSourceAccessDeclarationsV1::try_new(vec![]).unwrap();
            let access = foundation.bind_default_access_declarations(&empty, &BTreeSet::new()).unwrap();
            let domains = Domains::new(&access, &[], inputs.core_types).unwrap();
            assert!(matches!(domains.bind_nominal_default_value_domains(&declarations), Err(Error::Target { error, .. }) if matches!(*error, hir::DefaultSourceDomainError::Access(_))));
        });
    });
}
