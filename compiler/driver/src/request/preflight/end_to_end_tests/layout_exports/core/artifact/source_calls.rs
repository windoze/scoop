//! Real call occurrences must join provider declarations in both archive views.

use super::*;
use scoop_slib as slib;

mod candidates;
mod wire;

pub(super) fn check(
    input: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
    provider: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
    provider_artifact: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    artifact: &slib::AssembledCrossConeLayoutStrongArtifactV1,
) {
    let foundation = hir::OdrFreeHirFoundation::try_new(
        hir::CanonicalHirFoundation::from_type_semantics_output(input.hir).unwrap(),
    )
    .unwrap();
    let provider_foundation = hir::OdrFreeHirFoundation::try_new(
        hir::CanonicalHirFoundation::from_type_semantics_output(provider.hir).unwrap(),
    )
    .unwrap();
    let metadata = hir::SharedTypeMetadataV1 {
        provider: input.mir.module().cone,
        identities: input.identities,
        foundation: &foundation,
        public: input.public,
    };
    let dependencies = [hir::SharedTypeMetadataV1 {
        provider: provider.mir.module().cone,
        identities: provider.identities,
        foundation: &provider_foundation,
        public: provider.public,
    }];
    metadata
        .materialized_type_uses(&dependencies, &mut meter())
        .unwrap();
    for candidate in candidates::mutations(input.public) {
        let error = hir::SharedTypeMetadataV1 {
            public: &candidate.public,
            ..metadata
        }
        .materialized_type_uses(&dependencies, &mut meter())
        .unwrap_err();
        let hir::SharedTypeMetadataError::CallSignature { position, source } = error else {
            panic!("expected actual source signature rejection, got {error:?}");
        };
        assert_eq!(position, candidate.position);
        candidate.check_error(&source);
        let payload = wire::replace_public(artifact, candidate.public.clone());
        for link in [false, true] {
            let open = DecodedSlibEnvelope::open(
                &payload,
                DecodeLimits::default(),
                artifact.target_selection(),
            )
            .unwrap()
            .validate_graph()
            .unwrap();
            let (provider, consumer) = if link {
                (
                    super::lir_dependencies::reader::open_link(provider_artifact)
                        .into_shared_sections()
                        .unwrap(),
                    open.decode_cross_cone_layout_link_sections()
                        .unwrap()
                        .into_shared_sections()
                        .unwrap(),
                )
            } else {
                (
                    super::lir_dependencies::reader::open(provider_artifact),
                    open.decode_cross_cone_layout_compile_sections().unwrap(),
                )
            };
            let closure = slib::DecodedCrossConeLayoutCompileClosure::with_current_artifact(
                consumer.identity(),
                consumer.target_selection(),
                vec![provider.identity()],
                vec![provider],
                consumer,
            )
            .validate_profile_graph()
            .unwrap()
            .validate_identities()
            .unwrap()
            .validate_foundation_structure()
            .unwrap()
            .resolve_hir_sections()
            .unwrap()
            .validate_hir_productions()
            .unwrap();
            let error = match closure.validate_hir_declarations() {
                Ok(_) => panic!("invalid source call passed the shared reader (link={link})"),
                Err(error) => error,
            };
            assert_eq!(error.provider, metadata.provider);
            let slib::CrossConeHirDeclarationValidationError::References(
                slib::CrossConeHirReferenceSurfaceError::CallSites(error),
            ) = *error.source
            else {
                panic!("unexpected HIR reader rejection: {error:?}");
            };
            let slib::CrossConeHirCallSiteOriginError::Signature { position, source } = *error
            else {
                panic!("unexpected source call rejection: {error:?}");
            };
            assert_eq!(position, candidate.position);
            candidate.check_error(&source);
        }
    }
}
