use super::super::nominal_parameters::support::Sources;
use super::*;

#[test]
fn core_source_foundation_supplies_real_dependency_domains_to_default_binding() {
    let text = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-defaults/local-own-binder-combinations.scoop"
    ));
    super::super::super::source_dispatch::with_hir_source(text, |output, core| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let templates = super::super::default_origins::templates(output);
        let core_output = support::lower_protocol_core();
        let core_source = Production::from_hir(&core_output, &mut meter())
            .unwrap()
            .source_transcript(&mut meter())
            .unwrap();
        let decoded: hir::DecodedTypeFoundationSourceAuthorityV1 =
            decode_canonical(&encode(&core_source).unwrap(), DecodeLimits::default()).unwrap();
        let core_source = decoded
            .resolve(&mut fixture.identities, &mut meter())
            .unwrap();
        let core_required = core_source
            .entries()
            .source_roots
            .values()
            .iter()
            .map(subject)
            .collect();
        let local_required = sources
            .members
            .nominals
            .records()
            .iter()
            .map(|r| subject(&r.owner()))
            .collect();
        let core_access = access(&core_output.export, &core_required, &mut fixture.identities);
        let local_access = access(
            &output.output().export,
            &local_required,
            &mut fixture.identities,
        );
        let foundation = fixture.bind().unwrap();
        let core_bound = core_source
            .bind_to_foundation(&core.source_foundation, &fixture.identities, &mut meter())
            .unwrap();
        let core_access = core_bound
            .bind_default_access_declarations(&core_access, &core_required, &mut meter())
            .unwrap();
        let local_access = foundation
            .bind_default_access_declarations(&local_access, &local_required, &mut meter())
            .unwrap();
        let imported = core.foundation.import_core_inputs(&core.interface).unwrap();
        let types = imported.protocols().fundamental_types();
        let dependencies = [&core_access];
        let domains =
            hir::DefaultSourceTypeDomainsV1::new(&local_access, &dependencies, types, &mut meter())
                .unwrap();
        sources.with_bound(&foundation, types, |members, constructors| {
            let parameters = members.bind_parameter_protocols(constructors, &sources.protocols, &mut meter()).unwrap();
            let bound = parameters.bind_default_declarations(&templates, &[], &mut meter()).unwrap();
            let proof = domains.bind_nominal_default_type_domains(&bound, &mut meter()).unwrap();
            assert_eq!(proof.declarations().declarations().len(), 2);
            assert!(bound.declarations().iter().flat_map(|d| d.references().occurrences()).any(|o| {
                matches!(o.source(), hir::DefaultSourceReferenceRecordV1::Type(r) if r.target() == &Type::Nominal(types.boolean().persistent()))
            }));
        });
    });
}
fn subject(owner: &hir::SourceNominalId) -> Subject {
    match *owner {
        hir::SourceNominalId::Concrete(id) => Subject::Type(id),
        hir::SourceNominalId::GenericTemplate(id) => Subject::GenericType(id),
    }
}
fn access(
    output: &hir::ExportHirOutput,
    required: &BTreeSet<Subject>,
    identities: &mut scoop_identity::ValidatedIdentityGraph,
) -> hir::CanonicalDefaultSourceAccessDeclarationsV1 {
    let table = hir::CanonicalDefaultSourceAccessDeclarationsV1::from_export_hir(
        output,
        required,
        &mut meter(),
    )
    .unwrap();
    let decoded: hir::DecodedCanonicalDefaultSourceAccessDeclarationsV1 =
        decode_canonical(&encode(&table).unwrap(), DecodeLimits::default()).unwrap();
    decoded.resolve(identities, &mut meter()).unwrap()
}
