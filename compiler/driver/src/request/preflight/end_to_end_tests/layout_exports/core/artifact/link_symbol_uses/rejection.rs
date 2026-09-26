use super::super::link_archive::{self, Rewrite};
use super::*;

pub(super) fn mutation(
    core: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    artifact: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    profile: &lir::CBridgeToolchainProfileV1,
    mutation: &Mutation,
    failure: Failure,
) {
    reject(core, artifact, profile, mutation, failure, false);
}

pub(super) fn dependency_owner(
    core: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    artifact: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    profile: &lir::CBridgeToolchainProfileV1,
) {
    reject(
        core,
        artifact,
        profile,
        &Mutation::Projection(Failure::DefinedMember),
        Failure::DefinedMember,
        true,
    );
}

fn reject(
    core: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    artifact: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    profile: &lir::CBridgeToolchainProfileV1,
    mutation: &Mutation,
    failure: Failure,
    dependency: bool,
) {
    let source = if dependency { core } else { artifact };
    let bytes = link_archive::rewrite(source, |record, data| match mutation {
        Mutation::Projection(failure)
            if matches!(record.role(), slib::SlibMemberRole::LirMetadata) =>
        {
            Rewrite::Replace(link_archive::metadata(data, |section| {
                if section.capability() == &wire::capability(*failure) {
                    wire::mutate(section.payload(), *failure)
                } else {
                    section.payload().to_vec()
                }
            }))
        }
        Mutation::Object { member, offset } if record.id() == *member => {
            let mut changed = data.to_vec();
            changed[*offset] ^= 1;
            Rewrite::Replace(changed)
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
    let shared = if dependency {
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
        .replay_link_symbol_uses(profile)
        .map(|_| panic!("partial symbol-use closure escaped"))
        .unwrap_err();
    assert_eq!(error.provider, reader::open_link(source).identity());
    let slib::SharedLirPhysicalError::LinkSymbolUses(source) = *error.source else {
        panic!("expected {failure:?} symbol-use error: {error:?}")
    };
    use slib::LayoutLinkSymbolUseError as Error;
    assert!(
        matches!(
            (failure, source.as_ref()),
            (
                Failure::DefinedMissing | Failure::DefinedDuplicate | Failure::DefinedMember,
                Error::SymbolProjection(slib::LinkSymbolProjectionValidationError::DefinedSymbols(
                    _
                ))
            ) | (
                Failure::LegacyMissing | Failure::LegacyWidth,
                Error::SymbolProjection(
                    slib::LinkSymbolProjectionValidationError::UndefinedSymbols(_)
                )
            ) | (
                Failure::OrdinaryMissing | Failure::OrdinaryDuplicate | Failure::OrdinaryIndex,
                Error::OrdinaryProjection(_)
            ) | (
                Failure::ShapeMissing | Failure::ShapeDuplicate | Failure::ShapeIndex,
                Error::Shape(slib::LayoutLinkClosureError::RequirementsMismatch)
            ) | (Failure::UnknownRuntime, Error::Unclassified(_))
                | (Failure::WrongNativeSymbol, Error::Generated(_))
        ),
        "wrong error for {failure:?}: {source:?}"
    );
}

pub(super) fn views(
    core: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    artifact: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    profile: &lir::CBridgeToolchainProfileV1,
) {
    for (provider, input) in [
        (
            reader::open_link(core).identity(),
            reader::read(core, artifact),
        ),
        (
            reader::open_link(artifact).identity(),
            reader::read_sections(
                reader::open_link(core).into_shared_sections().unwrap(),
                reader::open(artifact),
            ),
        ),
    ] {
        let error = input
            .replay_link_symbol_uses(profile)
            .map(|_| panic!("Compile symbols accepted"))
            .unwrap_err();
        assert_eq!(error.provider, provider);
        assert!(
            matches!(*error.source, slib::SharedLirPhysicalError::LinkObjectContents(error)
            if matches!(*error, slib::LayoutLinkObjectContentsError::CompileView))
        );
    }
}
