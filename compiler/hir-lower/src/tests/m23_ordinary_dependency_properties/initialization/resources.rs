use super::*;
use hir::HirInitializationUseError as Error;

#[test]
fn property_initialization_replay_rejects_a_root_without_its_source_unit() {
    with_shared("standalone", |_, metadata, dependencies| {
        let mut foundation = metadata.foundation.clone();
        foundation.set_initialization_units(vec![]).unwrap();
        let foundation = hir::OdrFreeHirFoundation::try_new(foundation).unwrap();
        let missing = hir::SharedTypeMetadataV1 {
            foundation: &foundation,
            ..metadata
        };
        assert!(matches!(
            missing.materialized_property_initialization_uses(dependencies,),
            Err(Error::MissingLocalUnit(_))
        ));
    });
}
