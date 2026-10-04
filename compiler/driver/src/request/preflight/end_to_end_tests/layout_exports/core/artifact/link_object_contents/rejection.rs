use super::super::link_archive::{self, Rewrite};
use super::*;

pub(super) fn mutation(
    core: &slib::AssembledCrossConeLayoutArtifactV1,
    artifact: &slib::AssembledCrossConeLayoutArtifactV1,
    profile: &lir::CBridgeToolchainProfileV1,
    provider: ConeIdentity,
    mutation: &Mutation,
    failure: Failure,
) {
    let core_id = reader::open_link(core).identity();
    let source = if core_id == provider { core } else { artifact };
    let bytes = link_archive::rewrite(source, |record, data| match mutation {
        Mutation::Object {
            member,
            offset,
            value,
        } if record.id() == *member => {
            let mut bytes = data.to_vec();
            bytes[*offset] = *value;
            Rewrite::Replace(bytes)
        }
        Mutation::Projection(failure)
            if matches!(record.role(), slib::SlibMemberRole::LirMetadata) =>
        {
            Rewrite::Replace(link_archive::metadata(data, |section| {
                if section.capability() == &slib::lir_link_identity_closure_capability() {
                    projection::mutate(section.payload(), *failure)
                } else {
                    section.payload().to_vec()
                }
            }))
        }
        _ => Rewrite::Keep,
    });
    let changed = DecodedSlibEnvelope::open(&bytes, source.target_selection())
        .unwrap()
        .validate_graph()
        .unwrap()
        .decode_cross_cone_layout_link_sections()
        .unwrap()
        .into_shared_sections()
        .unwrap();
    let shared = if provider == core_id {
        reader::read_sections(
            changed,
            reader::open_link(artifact).into_shared_sections().unwrap(),
        )
    } else {
        reader::read_sections(
            reader::open_link(core).into_shared_sections().unwrap(),
            changed,
        )
    };
    let error = shared
        .replay_link_object_contents(profile)
        .map(|_| panic!("partial object closure escaped"))
        .unwrap_err();
    assert_eq!(error.provider, provider);
    let slib::SharedLirPhysicalError::LinkObjectContents(source) = *error.source else {
        panic!("expected {failure:?} object error: {error:?}");
    };
    use slib::LayoutLinkObjectContentsError as Error;
    assert!(
        matches!(
            (failure, source.as_ref()),
            (
                Failure::Envelope,
                Error::Objects(slib::BuiltinObjectSetValidationError::ScoopEnvelope {
                    source: slib::ScoopLirObjectEnvelopeValidationError::Envelope(
                        slib::ObjectEnvelopeValidationError::MalformedHeader
                    ),
                    ..
                })
            ) | (Failure::CBridgeEnvelope, Error::CBridgeEnvelopes(_))
                | (Failure::Symbols | Failure::Relocations, Error::Objects(_))
                | (Failure::Stackmaps, Error::Stackmaps(_))
                | (Failure::Safepoints, Error::Safepoints(_))
                | (Failure::Callables, Error::Callables(_))
                | (Failure::Types, Error::Types(_))
                | (Failure::Immortals, Error::Immortals(_))
                | (Failure::Storages, Error::Storages(_))
                | (Failure::Initializations, Error::Initializations(_))
                | (
                    Failure::AtomRange,
                    Error::ObjectProjection(
                        slib::LinkObjectProjectionValidationError::ProjectionMismatch
                    )
                )
                | (
                    Failure::DigestIntent,
                    Error::DigestInputs(
                        slib::LinkDigestPatchInputValidationError::UnknownPatchIntent(_)
                    )
                )
        ),
        "wrong error for {failure:?}: {source:?}"
    );
}

pub(super) fn views_and_profile(
    core: &slib::AssembledCrossConeLayoutArtifactV1,
    artifact: &slib::AssembledCrossConeLayoutArtifactV1,
    profile: &lir::CBridgeToolchainProfileV1,
    bridge_provider: ConeIdentity,
) {
    let core_id = reader::open_link(core).identity();
    let current = reader::open_link(artifact).identity();
    for (provider, shared) in [
        (core_id, reader::read(core, artifact)),
        (
            current,
            reader::read_sections(
                reader::open_link(core).into_shared_sections().unwrap(),
                reader::open(artifact),
            ),
        ),
    ] {
        let error = shared
            .replay_link_object_contents(profile)
            .map(|_| panic!("Compile objects accepted"))
            .unwrap_err();
        assert_eq!(error.provider, provider);
        assert!(
            matches!(*error.source, slib::SharedLirPhysicalError::LinkObjectContents(error)
            if matches!(*error, slib::LayoutLinkObjectContentsError::CompileView))
        );
    }
    let compiler = profile
        .contract()
        .compiler()
        .expect("Darwin test toolchain");
    let wrong = lir::CBridgeToolchainProfileV1::new_darwin_aarch64_apple_clang(
        profile
            .contract()
            .deployment()
            .expect("Darwin test toolchain")
            .clone(),
        lir::AppleClangCompilerIdentityV1::new(
            compiler.version_major() + 1,
            compiler.version_minor(),
            compiler.version_patch(),
            compiler.build(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_ne!(wrong.fingerprint(), profile.fingerprint());
    let error = reader::read_link(core, artifact)
        .replay_link_object_contents(&wrong)
        .map(|_| panic!("wrong C profile accepted"))
        .unwrap_err();
    assert_eq!(error.provider, bridge_provider);
    assert!(
        matches!(*error.source, slib::SharedLirPhysicalError::LinkObjectContents(error)
        if matches!(*error, slib::LayoutLinkObjectContentsError::CBridgeProduction(lir::CBridgeProductionValidationError::ProjectionMismatch)))
    );
}
