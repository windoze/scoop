use super::*;

mod candidates;

pub(in super::super) fn check(
    input: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
    provider_artifact: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    artifact: &slib::AssembledCrossConeLayoutStrongArtifactV1,
) {
    let candidates = candidates::mutations(input.public);
    assert!(
        !candidates.is_empty(),
        "the fixture needs actual extension calls"
    );
    for candidate in candidates {
        let payload = wire::replace_public(artifact, candidate.public);
        for link in [false, true] {
            let mut closure = reader::open(provider_artifact, artifact, &payload, link)
                .validate_hir_declarations()
                .unwrap();
            let error = match closure.validate_type_foundations() {
                Ok(_) => panic!("an invalid original receiver passed type replay (link={link})"),
                Err(error) => error,
            };
            assert_eq!(error.provider, input.mir.module().cone);
            assert!(
                matches!(error.source.as_ref(), hir::SharedTypeMetadataError::CallReceiver {
                position, receiver, expected,
            } if **position == candidate.position && *receiver == candidate.receiver && *expected == candidate.expected),
                "unexpected receiver rejection (link={link}): {error}"
            );
        }
    }
}
