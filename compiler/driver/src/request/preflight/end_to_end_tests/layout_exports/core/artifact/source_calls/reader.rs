use super::*;

pub(super) fn open<'a>(
    provider_artifact: &'a slib::AssembledCrossConeLayoutStrongArtifactV1,
    artifact: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    payload: &'a [u8],
    link: bool,
) -> slib::HirProductionValidatedCrossConeLayoutClosure<'a> {
    let open = DecodedSlibEnvelope::open(payload, artifact.target_selection())
        .unwrap()
        .validate_graph()
        .unwrap();
    let (provider, consumer) = if link {
        (
            super::super::lir_dependencies::reader::open_link(provider_artifact)
                .into_shared_sections()
                .unwrap(),
            open.decode_cross_cone_layout_link_sections()
                .unwrap()
                .into_shared_sections()
                .unwrap(),
        )
    } else {
        (
            super::super::lir_dependencies::reader::open(provider_artifact),
            open.decode_cross_cone_layout_compile_sections().unwrap(),
        )
    };
    slib::DecodedCrossConeLayoutCompileClosure::with_current_artifact(
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
    .unwrap()
}
