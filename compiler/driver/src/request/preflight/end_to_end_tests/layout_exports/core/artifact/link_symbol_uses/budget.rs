use super::*;
use scoop_wire::{DecodeUsage, ResourceKind, WireErrorKind};

pub(super) fn check(
    core: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    artifact: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    profile: &lir::CBridgeToolchainProfileV1,
    usage: DecodeUsage,
) {
    for resource in [ResourceKind::ValidationWorkUnits, ResourceKind::OwnedBytes] {
        for inclusive in [true, false] {
            let deficit = u64::from(!inclusive);
            let limits = match resource {
                ResourceKind::ValidationWorkUnits => DecodeLimits {
                    validation_work_units: usage.validation_work_units - deficit,
                    ..DecodeLimits::default()
                },
                ResourceKind::OwnedBytes => DecodeLimits {
                    owned_bytes: usage.owned_bytes - deficit,
                    ..DecodeLimits::default()
                },
                _ => panic!("fixture measures work and copied bytes"),
            };
            let current =
                DecodedSlibEnvelope::open(artifact.as_bytes(), limits, artifact.target_selection())
                    .unwrap()
                    .validate_graph()
                    .unwrap()
                    .decode_cross_cone_layout_link_sections()
                    .unwrap()
                    .into_shared_sections()
                    .unwrap();
            let input = reader::read_sections(
                reader::open_link(core).into_shared_sections().unwrap(),
                current,
            );
            let checked = input.with_replayed_link_symbol_uses(profile, |closure| {
                assert!(inclusive, "exhausted budget published partial symbol uses");
                assert_eq!(closure.dependency_first().len(), 2);
            });
            if inclusive {
                checked.unwrap();
                continue;
            }
            let error = checked.unwrap_err();
            assert_eq!(error.provider, reader::open_link(artifact).identity());
            let slib::SharedLirPhysicalError::LinkSymbolUses(error) = *error.source else {
                panic!("expected symbol-use budget error: {error:?}")
            };
            assert!(matches!(*error,
                slib::LayoutLinkSymbolUseError::SymbolProjection(slib::LinkSymbolProjectionValidationError::Resource(error))
                    if matches!(error.kind(), WireErrorKind::LimitExceeded { resource: actual, .. } if *actual == resource)));
        }
    }
}
