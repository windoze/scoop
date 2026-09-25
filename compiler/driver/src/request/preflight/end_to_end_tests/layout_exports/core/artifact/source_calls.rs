//! Real call occurrences must join provider declarations in both archive views.

use super::*;
use scoop_slib as slib;

mod candidates;
mod members;
mod reader;
mod receivers;
mod wire;

pub(super) use receivers::check as check_receivers;

pub(super) fn check(
    input: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
    provider: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
    provider_artifact: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    artifact: &slib::AssembledCrossConeLayoutStrongArtifactV1,
) {
    members::check(input, provider_artifact, artifact);
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
            let closure = reader::open(provider_artifact, artifact, &payload, link);
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
