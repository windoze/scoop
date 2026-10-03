use super::*;
use scoop_identity::{CoreBuiltinNominal, SignatureTypeKey};

mod mutations;

pub(super) fn check(
    name: &str,
    source: hir::CheckedSharedTypeFoundationV1<'_>,
    foundation: &mir::CanonicalMirFoundation,
    strong: &mir::StrongCallableBridgeSurfaceV1,
    section: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
) -> Vec<mir::MirTypeBridgeInitializationUnitV1> {
    let metadata = source.metadata();
    let unit_result = metadata
        .signature_exact_type(&SignatureTypeKey::Nominal(
            CoreBuiltinNominal::Unit.identity_record().id(),
        ))
        .unwrap();
    let replay = Replay {
        metadata,
        foundation,
        strong,
        unit_result,
    };
    let units = replay.run().unwrap();
    assert_eq!(units.len(), section.initialization_units().len());
    for (unit, producer) in units.iter().zip(section.initialization_units()) {
        assert_eq!(unit.unit(), producer.unit());
        assert_eq!(unit.initializer(), producer.initializer());
        assert_eq!(unit.ensure(), producer.ensure());
        assert_eq!(unit.signature(), producer.signature());
    }
    if name.starts_with("shared-units-") {
        assert!(!units.is_empty());
        mutations::check(&replay, &units);
        if name.ends_with("standalone") {
            assert_eq!(
                (metadata.source_initialization_units().len(), units.len()),
                (3, 2)
            );
        } else {
            assert_eq!(units.len(), 5);
        }
    }
    units
}

struct Replay<'a> {
    metadata: hir::SharedTypeMetadataV1<'a>,
    foundation: &'a mir::CanonicalMirFoundation,
    strong: &'a mir::StrongCallableBridgeSurfaceV1,
    unit_result: scoop_identity::PersistentExactTypeId,
}

impl Replay<'_> {
    fn run(
        &self,
    ) -> Result<Vec<mir::MirTypeBridgeInitializationUnitV1>, mir::MirTypeBridgeSectionError> {
        mir::replay_source_initialization_units(
            self.metadata.provider,
            self.metadata.source_initialization_units(),
            self.foundation,
            self.strong,
            self.metadata.identities,
        )
    }
}
