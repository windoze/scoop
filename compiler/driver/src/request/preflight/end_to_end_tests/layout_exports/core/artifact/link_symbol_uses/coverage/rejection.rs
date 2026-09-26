use super::*;

pub(super) fn check(
    core: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    artifact: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    profile: &lir::CBridgeToolchainProfileV1,
    case: Case,
    dependency: bool,
) {
    let source = if dependency { core } else { artifact };
    let bytes = link_archive::rewrite(source, |record, data| {
        if !matches!(record.role(), slib::SlibMemberRole::LirMetadata) {
            return Rewrite::Keep;
        }
        Rewrite::Replace(link_archive::metadata(data, |section| {
            if section.capability() == &mutation::capability(case) {
                mutation::apply(section.payload(), case)
            } else {
                section.payload().to_vec()
            }
        }))
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
        .map(|_| panic!("partial final object coverage escaped"))
        .unwrap_err();
    assert_eq!(error.provider, reader::open_link(source).identity());
    let slib::SharedLirPhysicalError::LinkSymbolUses(source) = *error.source else {
        panic!("expected final object projection error for {case:?}: {error:?}")
    };
    use slib::LayoutLinkSymbolUseError as Error;
    match case {
        Case::Members(Surface::Identity, _) | Case::ImageField(_) | Case::EntryBranch => {
            let expected = match case {
                Case::Members(_, _) => 6,
                Case::ImageField(_) => 7,
                Case::EntryBranch => 8,
                Case::Digest(_) => unreachable!(),
            };
            assert!(
                matches!(*source, Error::FinalProjection(
                slib::LinkFinalObjectProjectionError::FieldMismatch { field }
            ) if field == expected),
                "wrong error for {case:?}: {source:?}"
            );
        }
        Case::Members(Surface::Ordinary, _) | Case::Digest(Surface::Ordinary) => assert!(
            matches!(
                *source,
                Error::OrdinaryProjection(
                    slib::CrossConeLinkClosureSectionValidationError::ProjectionMismatch
                )
            ),
            "wrong error for {case:?}: {source:?}",
        ),
        Case::Members(Surface::Shape, _) | Case::Digest(Surface::Shape) => assert!(
            matches!(
                *source,
                Error::Shape(slib::LayoutLinkClosureError::ObjectCoverageMismatch)
            ),
            "wrong error for {case:?}: {source:?}",
        ),
        Case::Digest(Surface::Identity) => panic!("identity closure has no coverage digest"),
    }
}
