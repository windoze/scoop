use super::*;
use scoop_wire::{DecodeUsage, ResourceKind, WireErrorKind};

pub(in super::super) fn check(
    core: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    artifact: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    profile: &lir::CBridgeToolchainProfileV1,
    usage: DecodeUsage,
) {
    let current = reader::open_link(artifact).identity();
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
            let shared = reader::read_sections(
                reader::open_link(core).into_shared_sections().unwrap(),
                DecodedSlibEnvelope::open(artifact.as_bytes(), limits, artifact.target_selection())
                    .unwrap()
                    .validate_graph()
                    .unwrap()
                    .decode_cross_cone_layout_link_sections()
                    .unwrap()
                    .into_shared_sections()
                    .unwrap(),
            );
            let checked = shared.with_replayed_link_object_contents(profile, |proof| {
                assert!(
                    inclusive,
                    "exhausted budget published partial object proofs"
                );
                assert_eq!(proof.dependency_first().len(), 2);
            });
            if inclusive {
                checked.unwrap();
                continue;
            }
            let error = checked.unwrap_err();
            assert_eq!(error.provider, current);
            let slib::SharedLirPhysicalError::LinkObjectContents(error) = *error.source else {
                panic!("expected an object replay budget error: {error:?}");
            };
            assert!(
                matches!(*error, slib::LayoutLinkObjectContentsError::Resource(error)
                if matches!(error.kind(), WireErrorKind::LimitExceeded { resource: actual, .. } if *actual == resource))
            );
        }
    }
}
