use super::*;
use hir::DefaultSourceAccessDomainV1 as Domain;
mod nested;

#[test]
fn callable_domains_reject_each_changed_witness_including_repeated_and_nested_uses() {
    for source in [SOURCE, COMBINATIONS] {
        with_inputs(source, |inputs| {
            inputs.with_bound(|domains, parameters, _| {
                let mut kinds = BTreeSet::new();
                let mut rejected = 0;
                for original in inputs.templates.records() {
                    for (index, record) in original.references().callables().iter().enumerate() {
                        let replacement = if record.witness().target_domain() == &Domain::universal() {
                            Domain::empty()
                        } else { Domain::universal() };
                        let changed = super::super::default_declarations::reference_witness::change_at(
                            original, hir::ExportDefaultReferenceKindV1::Callable, index,
                            |w| hir::DefaultSourceAccessWitnessV1::try_new(w.owner(), w.direct_call_domain().clone(), w.slot_call_domain().clone(), replacement).unwrap(),
                            inputs.output,
                        );
                        let table = super::super::default_origins::replace(&inputs.templates, changed);
                        let declarations = parameters.bind_default_declarations(&table, &[]).unwrap();
                        let error = domains.bind_nominal_default_callable_domains(&declarations).unwrap_err();
                        assert!(matches!(error, Error::Witness { key, index: actual } if key == original.key() && actual == index as u32), "{error:?}");
                        kinds.insert(kind(record.target()));
                        rejected += 1;
                    }
                }
                assert!(kinds.len() >= 4);
                assert!(rejected >= 9);
            });
        });
    }
}

#[test]
fn callable_domains_require_the_same_foundation_and_complete_access_sources() {
    with_inputs(SOURCE, |inputs| {
        let foundation = inputs.fixture.bind().unwrap();
        inputs.with_bound(|_, parameters, current| {
            let declarations = parameters.bind_default_declarations(&inputs.templates, &[]).unwrap();
            let access = foundation.bind_default_access_declarations(&inputs.access, &inputs.required).unwrap();
            let domains = Domains::new(&access, &[], inputs.core_types).unwrap();
            assert!(matches!(domains.bind_nominal_default_callable_domains(&declarations), Err(Error::Foundation { expected, actual }) if expected == actual));
            let empty = hir::CanonicalDefaultSourceAccessDeclarationsV1::try_new(vec![]).unwrap();
            let access = current.bind_default_access_declarations(&empty, &BTreeSet::new()).unwrap();
            let domains = Domains::new(&access, &[], inputs.core_types).unwrap();
            assert!(matches!(domains.bind_nominal_default_callable_domains(&declarations), Err(Error::Target { error, .. }) if matches!(*error, hir::DefaultSourceDomainError::Access(_))));
        });
    });
}
