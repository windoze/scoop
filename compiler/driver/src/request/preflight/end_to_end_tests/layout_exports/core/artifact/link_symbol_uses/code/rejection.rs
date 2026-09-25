use super::*;

mod outer;

pub(super) fn check(
    core: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    artifact: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    profile: &lir::CBridgeToolchainProfileV1,
    dependency: bool,
    case: Case,
    seed: &[u8],
) {
    let source = if dependency { core } else { artifact };
    let bytes =
        link_archive::rewrite_production(source, |data| mutation::production(data, case, seed));
    let bytes = if matches!(case, Case::OuterCode | Case::MatchingWrongCode) {
        outer::rewrite(&bytes, source.target_selection())
    } else {
        bytes
    };
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
        .with_replayed_link_symbol_uses(profile, |_| panic!("invalid Code escaped"))
        .unwrap_err();
    assert_eq!(error.provider, reader::open_link(source).identity());
    let slib::SharedLirPhysicalError::LinkSymbolUses(error) = *error.source else {
        panic!("expected Code replay error for {case:?}: {error:?}")
    };
    let field = match case {
        Case::Distribution => 1,
        Case::OutputBranch => 2,
        Case::ManifestCode => 7,
        Case::NativeContractExtra
        | Case::NativeContractMissing
        | Case::NativeContractDuplicate
        | Case::NativeContractOrder
        | Case::NativeContractFingerprint => 8,
        Case::NativeLibraryExtra => 9,
        Case::OuterCode | Case::MatchingWrongCode => {
            assert!(
                matches!(
                    *error,
                    slib::LayoutLinkSymbolUseError::CodeFingerprintMismatch
                ),
                "{error:?}"
            );
            return;
        }
    };
    assert!(
        matches!(*error,
        slib::LayoutLinkSymbolUseError::CodeProjection(slib::CodeProductionProjectionError::FieldMismatch { field: actual })
            if actual == field),
        "wrong field for {case:?}: {error:?}"
    );
}
