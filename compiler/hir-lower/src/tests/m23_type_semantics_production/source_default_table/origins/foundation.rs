use super::*;

#[test]
fn default_source_points_survive_foundation_bytes_and_public_completion() {
    for source in [
        ORIGINS,
        SOURCE,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-nominals/parameters.scoop"
        )),
    ] {
        with_hir_source(source, |output, _| {
            let production = Production::from_dependency_hir(output, &mut meter()).unwrap();
            let origins = occurrences(production.templates());
            let legacy = hir::OdrFreeHirFoundation::try_new(
                hir::CanonicalHirFoundation::from_dependency_output(output).unwrap(),
            )
            .unwrap();
            let mut completed =
                hir::CanonicalHirFoundation::from_type_semantics_output(output).unwrap();
            completed
                .complete_cross_cone_source_points(
                    &output.output().export,
                    &hir::CanonicalExportDefinitionSourcesV1::default(),
                )
                .unwrap();
            let decoded: hir::DecodedHirFoundation =
                decode_canonical(&encode(&completed).unwrap(), DecodeLimits::default()).unwrap();
            let coordinate =
                scoop_identity::ConeCoordinate::new("test", "scoop-hir-lower", "0.0.0").unwrap();
            let completed = hir::OdrFreeHirFoundation::from_validated(
                decoded
                    .validate_with_dependency_sources(
                        &coordinate,
                        &mut identity_closure(output),
                        &mut meter(),
                    )
                    .unwrap(),
            )
            .unwrap();
            let mut missing_legacy = 0;
            for (_, _, origin, _) in origins {
                let origin = origin.origin();
                let points = [origin.span().start_byte(), origin.span().end_byte()];
                completed
                    .source_record(origin.source())
                    .unwrap()
                    .require_points(points)
                    .unwrap();
                assert_eq!(
                    completed
                        .source_context_key(origin.context())
                        .unwrap()
                        .source(),
                    origin.source()
                );
                if legacy
                    .source_record(origin.source())
                    .unwrap()
                    .require_points(points)
                    .is_err()
                {
                    missing_legacy += 1;
                }
            }
            assert!(
                missing_legacy > 0,
                "fixture must cover otherwise unpublished body locations"
            );
            for record in legacy.source_records() {
                completed
                    .source_record(record.identity())
                    .unwrap()
                    .require_points(
                        record
                            .points()
                            .iter()
                            .map(hir::SourcePointRecord::byte_offset),
                    )
                    .unwrap();
            }
        });
    }
}

#[test]
fn default_source_foundation_projection_uses_the_remaining_caller_budget() {
    with_hir_source(ORIGINS, |output, _| {
        let mut measured = meter();
        hir::CanonicalHirFoundation::from_type_semantics_output_with_budget(output, &mut measured)
            .unwrap();
        let work = measured.usage().validation_work_units;
        let mut shared = BudgetMeter::new(DecodeLimits {
            validation_work_units: work * 2 - 1,
            ..DecodeLimits::default()
        });
        hir::CanonicalHirFoundation::from_type_semantics_output_with_budget(output, &mut shared)
            .unwrap();
        assert!(matches!(
            hir::CanonicalHirFoundation::from_type_semantics_output_with_budget(
                output,
                &mut shared
            ),
            Err(hir::HirFoundationBuildError::DefaultSourceProduction(_))
                | Err(hir::HirFoundationBuildError::DefaultSourceResource(_))
        ));
    });
}
