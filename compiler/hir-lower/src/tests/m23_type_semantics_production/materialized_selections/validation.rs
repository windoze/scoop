use super::*;

#[test]
fn materialized_selections_reject_missing_extra_and_wrong_provider_records() {
    source_dispatch::with_hir_source(STANDALONE, |output, _| {
        let public = public_interface(output);
        let produced = produce_cross_cone_type_semantics(output, &public).unwrap();
        let records = produced.section().selected().records();
        assert!(!records.is_empty());
        production::with_metadata(output, &public, |metadata, dependencies| {
            let check = |records| {
                let selected = CanonicalSelectedExternalTypeUsesV1::try_new(records).unwrap();
                assert!(matches!(
                    metadata.validate_materialized_type_uses(&selected, dependencies),
                    Err(SharedTypeMetadataError::TypeUseInventory)
                ));
            };
            for index in 0..records.len() {
                let mut missing = records.to_vec();
                missing.remove(index);
                check(missing);
            }
            let mut wrong_provider = records.to_vec();
            wrong_provider[0] =
                SelectedExternalTypeUseV1::new(metadata.provider, records[0].usage());
            check(wrong_provider);
            let mut extra = records.to_vec();
            extra.push(SelectedExternalTypeUseV1::new(
                records[0].provider(),
                SelectedTypeUseV1::TypeTest {
                    exact: records[0].usage().exact(),
                },
            ));
            check(extra);
        });
    });
}

#[test]
fn materialized_selections_require_actual_unique_dependency_providers() {
    source_dispatch::with_hir_source(STANDALONE, |output, _| {
        let public = public_interface(output);
        production::with_metadata(output, &public, |metadata, dependencies| {
            assert!(
                matches!(metadata.materialized_type_uses(&[]), Err(SharedTypeMetadataError::MissingProvider(provider)) if dependencies.iter().any(|dependency| dependency.provider == provider))
            );
            assert!(
                matches!(metadata.materialized_type_uses(&[metadata]), Err(SharedTypeMetadataError::CurrentProviderDependency(provider)) if provider == metadata.provider)
            );
            let dependency = dependencies[0];
            assert!(
                matches!(metadata.materialized_type_uses(&[dependency, dependency]), Err(SharedTypeMetadataError::DuplicateProvider(provider)) if provider == dependency.provider)
            );
            let unrelated = hir::SharedTypeMetadataV1 {
                provider: scoop_identity::ConeCoordinate::new("test.unrelated", "types", "1.0.0")
                    .unwrap()
                    .identity()
                    .unwrap(),
                ..dependency
            };
            assert!(matches!(
                metadata.materialized_type_uses(&[unrelated]),
                Err(SharedTypeMetadataError::NominalOwner(_))
            ));
        });
    });
}
