use super::*;
use hir::{DefaultSourceReferenceV1 as Reference, ExportDefaultReferenceKindV1 as Kind};

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
    let refs = t.references();
    let mut callables = refs.callables().to_vec();
    let mut constructors = refs.constructors().to_vec();
    let mut types = refs.types().to_vec();
    let mut globals = refs.globals().to_vec();
    let mut singletons = refs.singleton_values().to_vec();
    let mut fields = refs.fields().to_vec();
    match kind {
        Kind::Callable => overwrite(&mut callables[0], domain),
        Kind::Constructor => overwrite(&mut constructors[0], domain),
        Kind::Type => overwrite(&mut types[0], domain),
        Kind::Global => overwrite(&mut globals[0], domain),
        Kind::Singleton => overwrite(&mut singletons[0], domain),
        Kind::Field => overwrite(&mut fields[0], domain),
    }
    let refs = hir::DefaultSourceReferencesV1::try_new(
        callables,
        constructors,
        types,
        globals,
        singletons,
        fields,
    )
    .unwrap();
    let changed = Template::try_new(
        t.key(),
        t.definition_root(),
        t.definition_path().clone(),
        t.locals().clone(),
        t.body().clone(),
        t.result().clone(),
        t.allows_suspend(),
        t.type_parameters().clone(),
        t.receiver().clone(),
        t.value_parameters().clone(),
        refs,
        t.definition_origin().clone(),
        &mut meter(),
    )
    .unwrap();
    let bytes = encode(&changed.index_locals(&mut meter()).unwrap()).unwrap();
    let decoded: hir::DecodedDefaultSourceTemplateV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    decoded
        .resolve(&mut identity_closure(output), &mut meter())
        .unwrap()
}
fn overwrite<T: Clone>(record: &mut Reference<T>, direct: Domain) {
    let source = record.witness();
    let witness = hir::DefaultSourceAccessWitnessV1::try_new(
        source.owner(),
        direct,
        source.slot_call_domain().clone(),
        source.target_domain().clone(),
    )
    .unwrap();
    *record = Reference::new(
        record.target().clone(),
        record.definition_origin().clone(),
        witness,
    );
}
