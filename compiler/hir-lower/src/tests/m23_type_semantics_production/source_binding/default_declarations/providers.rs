use super::*;

#[test]
fn valid_artifact_locations_do_not_replace_nominal_provider_and_parameter_ownership() {
    with_sources(SOURCE, |output, fixture, sources, core| {
        let table = templates(output);
        let key = key(output, "ContractChild.pick", 2);
        // Nominal-source projection owns the nominal endpoints; publication
        // also completes the separate public interface's endpoints.
        let public = source_template(output, "declarationSeed", 0);
        let mut origins = std::collections::BTreeSet::new();
        public
            .visit_definition_sources_metered(
                &mut |origin: &hir::ExportDefinitionSourceV1,
                      _,
                      _: &mut BudgetMeter,
                      _: &scoop_wire::WirePath| {
                    origins.insert(origin.clone());
                    Ok::<_, scoop_wire::WireError>(())
                },
                &mut meter(),
                &scoop_wire::WirePath::root(),
            )
            .unwrap();
        let mut canonical = fixture.foundation.as_canonical().clone();
        canonical
            .complete_cross_cone_source_points(
                &output.output().export,
                &hir::CanonicalExportDefinitionSourcesV1::try_new(origins.into_iter().collect())
                    .unwrap(),
            )
            .unwrap();
        let published = hir::OdrFreeHirFoundation::try_new(canonical).unwrap();
        let foundation = fixture
            .source
            .bind_to_foundation(&published, &fixture.identities, &mut meter())
            .unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let parameters = members.bind_parameter_protocols(constructors, &sources.protocols, &mut meter()).unwrap();
            for (name, nominal) in [("declarationSeed", false), ("ContractBase.number", true)] {
                let source = source_template(output, name, 0);
                let forged = Template::try_new(
                    key, source.definition_root(), source.definition_path().clone(),
                    source.locals().clone(), source.body().clone(), source.result().clone(),
                    source.allows_suspend(), source.type_parameters().clone(), source.receiver().clone(),
                    source.value_parameters().clone(), source.references().clone(), source.definition_origin().clone(), &mut meter(),
                ).unwrap();
                let changed = replace(&table, forged);
                // These are genuine artifact locations in the correct source
                // contexts. Only the declaration join rejects their claimed use.
                parameters.bind_default_origins(&changed, &[], &mut meter()).unwrap();
                let Error::Record { key: actual, error } = parameters.bind_default_declarations(&changed, &[], &mut meter()).unwrap_err() else {
                    panic!("expected a provider declaration error");
                };
                assert_eq!(actual, key);
                if nominal {
                    assert!(matches!(*error, Error::DefaultParameter { declaration, position: 2 } if declaration == source.definition_root().declaration()));
                } else {
                    assert!(matches!(*error, Error::Member(error) if matches!(*error, hir::NominalMemberBindingError::MissingCallable(id) if id == source.definition_root().declaration())));
                }
            }
        });
    });
}
