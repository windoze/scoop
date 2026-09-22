use super::*;

#[test]
fn profile_binding_rejects_each_wrong_classification_and_incomplete_inventory() {
    for source in [STANDALONE, COMBINED, DOMAINS] {
        with_inputs(source, |inputs| {
            inputs.with_bound(|domains, parameters, _| {
            let declarations = parameters.bind_default_declarations(inputs.production.templates(), &[], &mut meter()).unwrap();
            let targets = || domains.bind_nominal_default_target_domains(&declarations, &mut meter()).unwrap();
            let records = inputs.production.profiles().records();
            for original in records {
                let replacement = match original.profile() {
                    Profile::ParamFree => Profile::GenericSourceMetadata,
                    Profile::GenericSourceMetadata => Profile::ParamFree,
                };
                let changed = Profiles::try_new(records.iter().map(|r| if r.key() == original.key() {
                    hir::DefaultSourceProfileV1::new(r.key(), replacement)
                } else { *r }).collect(), &mut meter()).unwrap();
                assert!(matches!(targets().bind_source_profiles(&changed, &mut meter()), Err(Error::Mismatch { key, expected, actual }) if key == original.key() && expected == original.profile() && actual == replacement));
            }
            let missing = Profiles::try_new(records[1..].to_vec(), &mut meter()).unwrap();
            assert!(matches!(targets().bind_source_profiles(&missing, &mut meter()), Err(Error::Coverage(hir::SourceInventoryError::MissingDefaultProfile(key))) if key == records[0].key()));
            let unknown = hir::ProtectedDefaultTemplateKeyV1::try_new(records[0].key().owner(), u32::MAX).unwrap();
            let extra = Profiles::try_new(records.iter().copied().chain([hir::DefaultSourceProfileV1::new(unknown, Profile::ParamFree)]).collect(), &mut meter()).unwrap();
            assert!(matches!(targets().bind_source_profiles(&extra, &mut meter()), Err(Error::Coverage(hir::SourceInventoryError::UnexpectedDefaultProfile(key))) if key == unknown));
            let profiles = targets().bind_source_profiles(inputs.production.profiles(), &mut meter()).unwrap();
            assert!(matches!(profiles.default_access_profile(unknown, &mut meter()), Err(Error::MissingProfile(key)) if key == unknown));
        })
        });
    }
}

#[test]
fn complete_domain_binding_rejects_each_changed_target_witness() {
    use hir::{
        DefaultSourceAccessDomainV1 as Domain,
        DefaultSourceTargetDomainBindingError as TargetError, ExportDefaultReferenceKindV1 as Kind,
    };
    with_inputs(DOMAINS, |inputs| {
        inputs.with_bound(|domains, parameters, _| {
            let mut kinds = BTreeSet::new();
            for original in inputs.production.templates().records() {
                let references = original
                    .bind_reference_occurrences(&mut meter(), &scoop_wire::WirePath::root())
                    .unwrap();
                for occurrence in references.occurrences() {
                    let kind = occurrence.source().kind();
                    let index = occurrence.index();
                    let replacement =
                        if occurrence.source().witness().target_domain() == &Domain::universal() {
                            Domain::empty()
                        } else {
                            Domain::universal()
                        };
                    let changed = super::super::default_declarations::reference_witness::change_at(
                        original,
                        kind,
                        index as usize,
                        |w| {
                            hir::DefaultSourceAccessWitnessV1::try_new(
                                w.owner(),
                                w.direct_call_domain().clone(),
                                w.slot_call_domain().clone(),
                                replacement,
                            )
                            .unwrap()
                        },
                        inputs.output,
                    );
                    let table = super::super::default_origins::replace(
                        inputs.production.templates(),
                        changed,
                    );
                    let declarations = parameters
                        .bind_default_declarations(&table, &[], &mut meter())
                        .unwrap();
                    let error = domains
                        .bind_nominal_default_target_domains(&declarations, &mut meter())
                        .unwrap_err();
                    let (actual_key, actual_index) = match error {
                        TargetError::Type(hir::DefaultSourceTypeDomainBindingError::Witness {
                            key,
                            index,
                        }) if kind == Kind::Type => (key, index),
                        TargetError::Value(
                            hir::DefaultSourceValueDomainBindingError::Witness {
                                key,
                                kind: actual,
                                index,
                            },
                        ) if kind == actual => (key, index),
                        TargetError::Callable(
                            hir::DefaultSourceCallableDomainBindingError::Witness { key, index },
                        ) if kind == Kind::Callable => (key, index),
                        error => panic!("unexpected target domain error: {error:?}"),
                    };
                    assert_eq!((actual_key, actual_index), (original.key(), index));
                    kinds.insert(kind);
                }
            }
            assert_eq!(kinds.len(), 6);
        })
    });
}

#[test]
fn complete_domains_require_real_type_providers_and_the_same_bound_foundation() {
    with_inputs(STANDALONE, |inputs| {
        inputs.with_bound(|_, parameters, current| {
        let declarations = parameters.bind_default_declarations(inputs.production.templates(), &[], &mut meter()).unwrap();
        for foundation in [current, &inputs.fixture.bind().unwrap()] {
            let access = foundation.bind_default_access_declarations(&inputs.access, &inputs.required, &mut meter()).unwrap();
            let domains = hir::DefaultSourceDomainsV1::new(&access, &[], inputs.core_types, &mut meter()).unwrap();
            let error = domains.bind_nominal_default_target_domains(&declarations, &mut meter()).unwrap_err();
            if std::ptr::eq(foundation, current) {
                assert!(matches!(error, hir::DefaultSourceTargetDomainBindingError::Type(hir::DefaultSourceTypeDomainBindingError::Target { error, .. }) if matches!(*error, hir::DefaultSourceDomainError::MissingProvider(_))));
            } else {
                assert!(matches!(error, hir::DefaultSourceTargetDomainBindingError::Type(hir::DefaultSourceTypeDomainBindingError::Foundation { expected, actual }) if expected == actual));
            }
        }
    })
    });
}
