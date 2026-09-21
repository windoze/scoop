use super::*;
use hir::DefaultSourceAccessDomainV1 as Domain;

#[test]
fn default_type_binding_rejects_losing_one_of_multiple_generic_constraints() {
    with_inputs(COMBINATIONS, |inputs| {
        inputs.with_bound(|domains, parameters, _| {
            let (original, index, reference) = inputs
                .templates
                .records()
                .iter()
                .flat_map(|t| {
                    t.references()
                        .types()
                        .iter()
                        .enumerate()
                        .map(move |(i, r)| (t, i, r))
                })
                .find(|(_, _, r)| {
                    r.witness()
                        .target_domain()
                        .generic_subclasses()
                        .values()
                        .len()
                        == 2
                })
                .unwrap();
            let target = reference.witness().target_domain();
            let generic = hir::CanonicalPersistentIdsV1::try_new(
                target.generic_subclasses().values()[1..].to_vec(),
            )
            .unwrap();
            let replacement = Domain::try_new(target.persistent().clone(), generic).unwrap();
            let changed = super::super::default_declarations::reference_witness::change_at(
                original,
                hir::ExportDefaultReferenceKindV1::Type,
                index,
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
            let table = super::super::default_origins::replace(&inputs.templates, changed);
            let bound = parameters
                .bind_default_declarations(&table, &[], &mut meter())
                .unwrap();
            let error = domains
                .bind_nominal_default_type_domains(&bound, &mut meter())
                .unwrap_err();
            let Error::Witness { key, index: actual } = error else {
                panic!("unexpected failure: {error:?}");
            };
            assert_eq!(key, original.key());
            assert_eq!(actual as usize, index);
        });
    });
}

#[test]
fn default_type_binding_rejects_corrupt_target_witness_with_exact_occurrence() {
    for source in [SOURCE, COMBINATIONS] {
        with_inputs(source, |inputs| {
            inputs.with_bound(|domains, parameters, _| {
                let mut rejected = 0;
                for original in inputs.templates.records() {
                    for (index, reference) in original.references().types().iter().enumerate() {
                        let target = reference.witness().target_domain();
                        let replacement = if target == &Domain::universal() {
                            Domain::empty()
                        } else {
                            Domain::universal()
                        };
                        let changed =
                            super::super::default_declarations::reference_witness::change_at(
                                original,
                                hir::ExportDefaultReferenceKindV1::Type,
                                index,
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
                        let table =
                            super::super::default_origins::replace(&inputs.templates, changed);
                        // Declaration binding alone cannot authenticate target domains.
                        let bound = parameters
                            .bind_default_declarations(&table, &[], &mut meter())
                            .unwrap();
                        let error = domains
                            .bind_nominal_default_type_domains(&bound, &mut meter())
                            .unwrap_err();
                        let Error::Witness { key, index: actual } = error else {
                            panic!("unexpected failure: {error:?}");
                        };
                        assert_eq!(key, original.key());
                        assert_eq!(actual as usize, index);
                        rejected += 1;
                    }
                }
                assert!(rejected > 5);
            });
        });
    }
}

#[test]
fn default_type_binding_rejects_another_bound_foundation_and_missing_access_source() {
    with_inputs(SOURCE, |inputs| {
        inputs.with_bound(|_, parameters, foundation| {
            let bound = parameters
                .bind_default_declarations(&inputs.templates, &[], &mut meter())
                .unwrap();
            let other = inputs.fixture.bind().unwrap();
            let access = other
                .bind_default_access_declarations(&inputs.access, &inputs.required, &mut meter())
                .unwrap();
            let domains = Domains::new(&access, &[], inputs.core, inputs.core_types, &mut meter()).unwrap();
            let error = domains.bind_nominal_default_type_domains(&bound, &mut meter()).unwrap_err();
            assert!(matches!(error, Error::Foundation { expected, actual } if expected == actual));

            let empty = hir::CanonicalDefaultSourceAccessDeclarationsV1::try_new(vec![], &mut meter()).unwrap();
            let access = foundation
                .bind_default_access_declarations(&empty, &BTreeSet::new(), &mut meter())
                .unwrap();
            let domains = Domains::new(&access, &[], inputs.core, inputs.core_types, &mut meter()).unwrap();
            let error = domains.bind_nominal_default_type_domains(&bound, &mut meter()).unwrap_err();
            assert!(matches!(error, Error::Target { error, .. } if matches!(*error, hir::DefaultSourceTypeDomainError::Access(_))));
        });
    });
}

#[test]
fn default_type_binding_requires_actual_core_access_sources_for_boolean() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-defaults/local-own-binders.scoop"
    ));
    with_inputs(source, |inputs| {
        inputs.with_bound(|domains, parameters, _| {
            let bound = parameters
                .bind_default_declarations(&inputs.templates, &[], &mut meter())
                .unwrap();
            let error = domains
                .bind_nominal_default_type_domains(&bound, &mut meter())
                .unwrap_err();
            let Error::Target { key, index, error } = error else {
                panic!("unexpected failure: {error:?}");
            };
            assert!(matches!(
                *error,
                hir::DefaultSourceTypeDomainError::MissingProvider(
                    scoop_identity::ConeIdentity::CORE
                )
            ));
            let reference =
                &inputs.templates.get(key).unwrap().references().types()[index as usize];
            assert_eq!(
                reference.target(),
                &scoop_identity::SignatureTypeKey::Nominal(
                    inputs.core_types.boolean().persistent()
                )
            );
        });
    });
}
