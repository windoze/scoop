use super::*;

#[test]
fn default_root_requires_its_own_callable_or_enum_context() {
    with_sources(SOURCE, |output, fixture, sources, core| {
        let table = templates(output);
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let parameters = members.bind_parameter_protocols(constructors, &sources.protocols, &mut meter()).unwrap();
            for template in table.records() {
                let other = table.records().iter().find(|other| other.definition_origin().origin().context() != template.definition_origin().origin().context()).unwrap();
                assert_eq!(other.definition_origin().origin().source(), template.definition_origin().origin().source());
                let forged = replace(&table, rebuild(template, template.definition_root(), template.body().clone(), other.definition_origin().clone()));
                assert!(matches!(parameters.bind_default_origins(&forged, &[], &mut meter()), Err(Error::RootOrigin(root)) if root == template.definition_root()));
            }
        });
    });
}

#[test]
fn default_location_binding_requires_complete_parameter_coverage() {
    with_sources(SOURCE, |output, fixture, sources, core| {
        let table = templates(output);
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let parameters = members.bind_parameter_protocols(constructors, &sources.protocols, &mut meter()).unwrap();
            for index in 0..table.records().len() {
                let mut incomplete = table.records().to_vec();
                let removed = incomplete.remove(index).key();
                let incomplete = Table::try_new(incomplete, &mut meter()).unwrap();
                assert!(matches!(parameters.bind_default_origins(&incomplete, &[], &mut meter()), Err(Error::Coverage(hir::DefaultSourceTemplateCoverageError::Missing(key))) if key == removed));
            }
        });
    });
}

#[test]
fn default_locations_require_body_points_not_only_declaration_points() {
    with_sources(SOURCE, |output, fixture, sources, core| {
        let removed = (SOURCE.find("= 1)").unwrap() + 2) as u64;
        let mut canonical = fixture.foundation.as_canonical().clone();
        canonical
            .set_sources(
                fixture
                    .foundation
                    .source_records()
                    .iter()
                    .map(|record| {
                        assert!(
                            record
                                .points()
                                .iter()
                                .any(|point| point.byte_offset() == removed)
                        );
                        hir::SourceRecord::from_utf8(
                            record.identity().clone(),
                            SOURCE,
                            record
                                .points()
                                .iter()
                                .map(hir::SourcePointRecord::byte_offset)
                                .filter(|point| *point != removed),
                        )
                        .unwrap()
                    })
                    .collect(),
            )
            .unwrap();
        let incomplete = hir::OdrFreeHirFoundation::try_new(canonical).unwrap();
        let foundation = fixture
            .source
            .bind_to_foundation(&incomplete, &fixture.identities, &mut meter())
            .unwrap();
        let table = templates(output);
        sources.with_bound(&foundation, core, |members, constructors| {
            let parameters = members
                .bind_parameter_protocols(constructors, &sources.protocols, &mut meter())
                .unwrap();
            assert!(matches!(
                parameters.bind_default_origins(&table, &[], &mut meter()),
                Err(Error::Origin {
                    error: hir::TypeFoundationBindingError::MissingSourcePoint(_),
                    ..
                })
            ));
        });
    });
}

#[test]
fn default_root_origin_cannot_borrow_another_files_valid_source_context() {
    super::super::super::source_dispatch::with_hir_sources(
        &[
            ("src/main.scoop", SOURCE),
            (
                "src/other.scoop",
                "public class OtherLocation { public fun get(value: Int = 9): Int = value }",
            ),
        ],
        |output, core| {
            let mut fixture = Fixture::from_output(output);
            let sources = Sources::from_output(output, &mut fixture);
            let table = templates(output);
            let first = &table.records()[0];
            let other = table
                .records()
                .iter()
                .find(|t| {
                    t.definition_origin().origin().source()
                        != first.definition_origin().origin().source()
                })
                .unwrap();
            let forged = replace(
                &table,
                rebuild(
                    first,
                    first.definition_root(),
                    first.body().clone(),
                    other.definition_origin().clone(),
                ),
            );
            let foundation = fixture.bind().unwrap();
            let inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
            sources.with_bound(&foundation, inputs.protocols().fundamental_types(), |members, constructors| {
                let parameters = members.bind_parameter_protocols(constructors, &sources.protocols, &mut meter()).unwrap();
                parameters.bind_default_origins(&table, &[], &mut meter()).unwrap();
                assert!(matches!(parameters.bind_default_origins(&forged, &[], &mut meter()), Err(Error::RootOrigin(root)) if root == first.definition_root()));
            });
        },
    );
}
