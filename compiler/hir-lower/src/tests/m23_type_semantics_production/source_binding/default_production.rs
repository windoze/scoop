use super::super::source_dispatch::{with_hir_source, with_hir_sources};
use super::*;
use scoop_identity::CallableTemplateOrigin;

mod resources;
mod support;
mod varargs;
use support::*;

const REFERENCES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-protected-production/default-references.scoop"
));
const COMBINATIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-protected-production/default-combinations.scoop"
));

#[test]
fn complete_type_defaults_roundtrip_and_preserve_source_bodies_and_occurrences() {
    for (name, source) in [
        ("default-references", REFERENCES),
        ("default-combinations", COMBINATIONS),
    ] {
        with_hir_source(source, |output, _| {
            let production =
                produce_cross_cone_type_semantics(output, &public_interface(output)).unwrap();
            let section = restore(output, production.section());
            let sources =
                hir::NominalDefaultSourceProductionV1::from_dependency_hir(output, &mut meter())
                    .unwrap();
            let expected_keys = section
                .protected_source_interfaces()
                .records()
                .iter()
                .flat_map(|protocol| protocol.parameters().parameters())
                .filter_map(|parameter| parameter.calling().template())
                .collect::<BTreeSet<_>>();
            assert_eq!(
                section
                    .protected_defaults()
                    .records()
                    .iter()
                    .map(|template| template.key())
                    .collect::<BTreeSet<_>>(),
                expected_keys
            );
            assert!(!expected_keys.is_empty());
            let mut counts = [0; 6];
            let mut inherited = 0;
            let mut generic = 0;
            let domains = direct_domains(output);
            for template in section.protected_defaults().records() {
                let original = sources.templates().get(template.key()).unwrap();
                assert_source_body(template, original);
                let closure = original
                    .bind_reference_occurrences(&mut meter(), &WirePath::root())
                    .unwrap();
                let mut visitor = Occurrences::new(&closure, &domains[&template.key().owner()]);
                template
                    .references()
                    .validate_body_closure(
                        template.key(),
                        template.body(),
                        template.locals(),
                        template.definition_origin(),
                        template.receiver(),
                        &mut visitor,
                        &mut meter(),
                        &WirePath::root(),
                    )
                    .unwrap();
                assert_eq!(visitor.count, closure.occurrences().len());
                for (total, count) in counts
                    .iter_mut()
                    .zip(reference_counts(template.references()))
                {
                    *total += count;
                }
                inherited +=
                    usize::from(template.key().owner() != template.definition_root().declaration());
                generic += visitor.generic;
                if template.key().owner() != template.definition_root().declaration() {
                    assert!(template.references().types().iter().any(|record| {
                        matches!(record.witness().view(), hir::ProtectedDefaultAccessWitnessViewV1::ParamFree(witness) if !witness.slot_call_domains().records().is_empty())
                    }));
                }
            }
            if name == "default-references" {
                assert!(
                    counts.iter().all(|count| *count > 0),
                    "missing reference kind: {counts:?}"
                );
                assert_eq!(inherited, 2);
                assert!(section.protected_defaults().records().iter().any(|template| template.references().types().iter().any(|record| matches!(record.witness().view(), hir::ProtectedDefaultAccessWitnessViewV1::ParamFree(witness) if witness.slot_call_domains().records().len() == 2))));
            } else {
                assert!(generic > 0);
            }
            snapshot(name, &outline(output, section.protected_defaults()));
        });
    }
}

#[test]
fn type_default_production_is_stable_across_unrelated_source_allocation() {
    let produce = |files: &[(&str, &str)]| {
        with_hir_sources(files, |output, _| {
            let production =
                produce_cross_cone_type_semantics(output, &public_interface(output)).unwrap();
            encode(&production.section().index_for_wire(&mut meter()).unwrap()).unwrap()
        })
    };
    let first = produce(&[("src/main.scoop", REFERENCES)]);
    let second = produce(&[
        ("src/a-unused.scoop", "private fun unused(): Int = 99"),
        ("src/main.scoop", REFERENCES),
    ]);
    assert!(
        first == second,
        "default publication changed after unrelated arena allocation"
    );
}

#[test]
fn type_default_body_closure_rejects_missing_references_and_incorrect_uses() {
    with_hir_source(REFERENCES, |output, _| {
        let production =
            produce_cross_cone_type_semantics(output, &public_interface(output)).unwrap();
        let sources =
            hir::NominalDefaultSourceProductionV1::from_dependency_hir(output, &mut meter())
                .unwrap();
        let template = production
            .section()
            .protected_defaults()
            .records()
            .iter()
            .find(|template| !template.references().fields().is_empty())
            .unwrap();
        let original = sources.templates().get(template.key()).unwrap();
        let closure = original
            .bind_reference_occurrences(&mut meter(), &WirePath::root())
            .unwrap();
        let references = template.references();
        let domains = direct_domains(output);
        for incorrect_use in [false, true] {
            let fields = if incorrect_use {
                references
                    .fields()
                    .iter()
                    .map(|record| {
                        hir::ProtectedDefaultReferenceV1::new(
                            record.target().clone(),
                            record.definition_origin().clone(),
                            record.witness().clone(),
                            hir::CanonicalProtectedDefaultExpressionUsesV1::try_new(vec![
                                hir::ProtectedDefaultExpressionUseV1::new(
                                    u32::MAX,
                                    hir::ProtectedDefaultReceiverUseV1::None,
                                ),
                            ])
                            .unwrap(),
                        )
                    })
                    .collect()
            } else {
                Vec::new()
            };
            let changed = hir::ProtectedDefaultReferenceSetV1::try_new(
                references.callables().to_vec(),
                references.constructors().to_vec(),
                references.types().to_vec(),
                references.globals().to_vec(),
                references.singleton_values().to_vec(),
                fields,
            )
            .unwrap();
            assert!(
                changed
                    .validate_body_closure(
                        template.key(),
                        template.body(),
                        template.locals(),
                        template.definition_origin(),
                        template.receiver(),
                        &mut Occurrences::new(&closure, &domains[&template.key().owner()]),
                        &mut meter(),
                        &WirePath::root()
                    )
                    .is_err()
            );
        }
        let empty = hir::CanonicalProtectedDefaultTemplatesV1::try_new(Vec::new()).unwrap();
        assert!(
            production
                .section()
                .protected_source_interfaces()
                .index_templates(empty.keys(), &mut meter())
                .is_err()
        );
    });
}
