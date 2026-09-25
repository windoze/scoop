use super::*;
use hir::HirInitializationUseError as Error;

#[test]
fn property_initialization_replay_uses_reachable_provider_and_original_budget() {
    with_shared("standalone", |_, metadata, dependencies| {
        assert!(matches!(
            metadata.materialized_property_initialization_uses(&[], &mut meter()),
            Err(Error::MissingProvider(provider)) if provider == dependencies[1].provider,
        ));
        assert!(matches!(
            metadata.materialized_property_initialization_uses(
                &[dependencies[1], dependencies[1]], &mut meter(),
            ),
            Err(Error::DuplicateProvider(provider)) if provider == dependencies[1].provider,
        ));
        assert!(matches!(
            metadata.materialized_property_initialization_uses(&[metadata], &mut meter()),
            Err(Error::LocalProvider(provider)) if provider == metadata.provider,
        ));
        let mut measured = meter();
        let expected = metadata
            .materialized_property_initialization_uses(dependencies, &mut measured)
            .unwrap();
        let mut exact = BudgetMeter::new(DecodeLimits {
            validation_work_units: measured.usage().validation_work_units,
            ..DecodeLimits::default()
        });
        assert_eq!(
            metadata
                .materialized_property_initialization_uses(dependencies, &mut exact,)
                .unwrap(),
            expected
        );
        assert!(matches!(
            metadata.materialized_property_initialization_uses(dependencies, &mut exact,),
            Err(Error::Resource(_))
        ));
        let mut exhausted = BudgetMeter::new(DecodeLimits {
            owned_bytes: 0,
            ..DecodeLimits::default()
        });
        assert!(matches!(
            metadata.materialized_property_initialization_uses(dependencies, &mut exhausted,),
            Err(Error::Resource(_))
        ));
    });
}

#[test]
fn property_initialization_replay_rejects_a_root_without_its_source_unit() {
    with_shared("standalone", |_, metadata, dependencies| {
        let mut foundation = metadata.foundation.as_canonical().clone();
        foundation.set_initialization_units(vec![]).unwrap();
        let foundation = hir::OdrFreeHirFoundation::try_new(foundation).unwrap();
        let missing = hir::SharedTypeMetadataV1 {
            foundation: &foundation,
            ..metadata
        };
        assert!(matches!(
            missing.materialized_property_initialization_uses(dependencies, &mut meter(),),
            Err(Error::MissingLocalUnit(_))
        ));
    });
}
