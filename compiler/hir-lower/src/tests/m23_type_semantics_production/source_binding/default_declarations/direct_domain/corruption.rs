use super::*;
use hir::ExportDefaultReferenceKindV1 as Kind;

#[test]
fn default_direct_domains_reject_witness_replacement_in_every_reference_kind() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-defaults/reference-closure.scoop"
    ));
    with_sources(source, |output, fixture, sources, core| {
        let table = templates(output);
        let original = table
            .get(key(output, "ReferenceClosureHost.all", 0))
            .unwrap();
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let parameters = members.bind_parameter_protocols(constructors, &sources.protocols, &mut meter()).unwrap();
            for kind in [Kind::Callable, Kind::Constructor, Kind::Type, Kind::Global, Kind::Singleton, Kind::Field] {
                for domain in [Domain::empty(), Domain::universal()] {
                    let changed = replace(&table, changed(original, kind, domain, output));
                    let error = parameters.bind_default_declarations(&changed, &[], &mut meter()).unwrap_err();
                    let Error::Record { key, error } = error else { panic!("record error") };
                    assert_eq!(key, original.key());
                    assert!(matches!(*error, Error::DirectDomain(error) if matches!(*error, hir::DefaultSourceDirectDomainError::Witness { kind: actual, index: 0 } if actual == kind)));
                }
            }
        });
    });
}

#[test]
fn default_direct_domains_reject_removing_generic_outer_visibility_from_static_nested() {
    with_sources(COMBINATIONS, |output, fixture, sources, core| {
        let table = templates(output);
        let original = table
            .get(key(output, "DirectGenericDomain.Static.exposed", 0))
            .unwrap();
        let changed = replace(
            &table,
            changed(original, Kind::Type, Domain::universal(), output),
        );
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let parameters = members.bind_parameter_protocols(constructors, &sources.protocols, &mut meter()).unwrap();
            assert!(matches!(parameters.bind_default_declarations(&changed, &[], &mut meter()), Err(Error::Record { error, .. }) if matches!(*error, Error::DirectDomain(_))));
        });
    });
}

fn changed(
    t: &Template,
    kind: Kind,
    domain: Domain,
    output: &hir::OrdinaryHirOutput<'_>,
) -> Template {
    super::super::reference_witness::change(
        t,
        kind,
        |source| {
            hir::DefaultSourceAccessWitnessV1::try_new(
                source.owner(),
                domain,
                source.slot_call_domain().clone(),
                source.target_domain().clone(),
            )
            .unwrap()
        },
        output,
    )
}
