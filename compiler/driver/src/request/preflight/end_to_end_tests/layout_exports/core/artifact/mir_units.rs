use super::*;
use scoop_identity::InitializationUnitKey;

pub(super) fn check(
    name: &str,
    source: hir::CheckedSharedTypeFoundationV1<'_>,
    strong: &mir::StrongCallableBridgeSurfaceV1,
    section: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
) -> Vec<mir::MirTypeBridgeInitializationUnitV1> {
    let metadata = source.metadata();
    let units = mir::read_initialization_units(strong, metadata.identities).unwrap();
    assert_eq!(units.len(), section.initialization_units().len());
    for (unit, producer) in units.iter().zip(section.initialization_units()) {
        assert_eq!(unit.unit(), producer.unit());
        assert_eq!(unit.initializer(), producer.initializer());
        assert_eq!(unit.ensure(), producer.ensure());
        assert_eq!(unit.signature(), producer.signature());
    }
    if name.starts_with("shared-units-") {
        assert!(!units.is_empty());
        if name.ends_with("standalone") {
            assert_eq!(
                (metadata.source_initialization_units().len(), units.len()),
                (23, 18)
            );
            // Generic companion templates, including MaybeUninit, have no strong pairs.
            // Scalar companions, Unit codecs and the suspend object keep their pairs.
            assert_eq!(
                metadata
                    .source_initialization_units()
                    .iter()
                    .filter(|source| {
                        !matches!(
                            source.key(),
                            InitializationUnitKey::GenericCompanionTemplate(_)
                        )
                    })
                    .map(|source| source.id())
                    .collect::<std::collections::BTreeSet<_>>(),
                units.iter().map(|unit| unit.unit()).collect(),
            );
        } else {
            assert_eq!(units.len(), 20);
        }
    }
    units
}
