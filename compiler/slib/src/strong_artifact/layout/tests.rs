mod support;

use scoop_mir::*;
use scoop_wire::encode;

use super::*;
use crate::{CodeFingerprintDataV2, DecodedSlibEnvelope, ValidatedGraphArtifact};
use crate::{EmptyLayoutCodeFixture, with_empty_layout_code_fixture};
use support::{
    empty_hir_interface, empty_hir_type_semantics, hir_foundation_and_production, mir_sections,
};

#[test]
fn layout_writer_is_reproducible_and_sections_round_trip() {
    let (first, code) = write_artifact();
    let first = first.unwrap();
    let (second, _) = write_artifact();
    let second = second.unwrap();
    assert_eq!(first.as_bytes(), second.as_bytes());
    assert_eq!(first.target_selection(), selection());

    let graph = open_graph(first.as_bytes());
    assert!(graph.envelope.manifest().sections().is_empty());
    let compile = graph.decode_cross_cone_layout_compile_sections().unwrap();
    assert_eq!(
        encode(compile.lir_strong_production_wire()).unwrap(),
        encode(code.production().strong_production()).unwrap()
    );

    let link = open_graph(first.as_bytes())
        .decode_cross_cone_layout_link_sections()
        .unwrap();
    assert_eq!(
        encode(link.lir_strong_production_wire()).unwrap(),
        encode(code.production().strong_production()).unwrap()
    );
    let expected_identity = LinkIdentityClosureSectionV1::from_verified_layout_code(&code);
    assert_eq!(
        encode(link.link_identity_closure_wire()).unwrap(),
        encode(&expected_identity).unwrap()
    );
}

fn write_artifact() -> (
    Result<AssembledCrossConeLayoutArtifactV1, CrossConeLayoutArtifactWriteError>,
    CodeFingerprintDataV2,
) {
    with_empty_layout_code_fixture(|lir| {
        let code = lir.code.code().clone();
        let result = write_artifact_from_fixture(lir);
        (result, code)
    })
}

fn write_artifact_from_fixture(
    lir: EmptyLayoutCodeFixture<'_>,
) -> Result<AssembledCrossConeLayoutArtifactV1, CrossConeLayoutArtifactWriteError> {
    let provider = lir.cone.identity();
    let (hir_foundation, hir_production) = hir_foundation_and_production(&lir.cone);
    let hir_type_semantics = empty_hir_type_semantics();
    let (mir_foundation, mir_production, mir_ordinary) = mir_sections(provider);
    let graph = scoop_identity::PendingIdentityValidation::new()
        .finish()
        .unwrap();
    let mir_type_bridge = empty_mir_type_bridge(provider, &mir_production, &mir_ordinary, &graph);

    AssembledCrossConeLayoutArtifactV1::write(CrossConeLayoutArtifactInputV1::new(
        crate::ProducerRecord::new("layout-writer-test").unwrap(),
        lir.cone,
        lir.dependencies,
        &hir_foundation,
        &hir_production,
        empty_hir_interface(),
        &hir_type_semantics,
        &mir_foundation,
        &mir_production,
        &mir_ordinary,
        &mir_type_bridge,
        lir.foundation,
        lir.ordinary,
        lir.layout,
        lir.code,
        lir.link_objects,
    ))
}

fn empty_mir_type_bridge<'a>(
    provider: scoop_identity::ConeIdentity,
    production: &'a CoreBootstrapBridgeSectionV1,
    ordinary: &'a CrossConeMirBridgeSectionV1,
    graph: &scoop_identity::ValidatedIdentityGraph,
) -> CrossConeMirTypeBridgeSectionV1<'a> {
    let types = CanonicalParamFreeMirTypeExportsV1::try_new(vec![]).unwrap();
    let callables = CanonicalMirCallableBindingsV1::try_new(vec![]).unwrap();
    let dispatch = CanonicalMirDispatchSchemasV1::try_new(
        MirDispatchSchemaAuthority {
            identities: graph,
            types: &types,
            callables: &callables,
        },
        vec![],
    )
    .unwrap();
    let objects = CanonicalMirObjectValuesV1::try_new(vec![]).unwrap();
    let shapes = CanonicalMirShapeSupportsV1::try_new(
        provider,
        MirShapeSupportAuthority {
            identities: graph,
            types: &types,
        },
        vec![],
    )
    .unwrap();
    let exports = MirTypeBridgeExportConstituentsV1::new(
        types,
        callables,
        dispatch,
        objects,
        shapes,
        CanonicalMirExternalInitializationUsesV1::try_new(vec![]).unwrap(),
    );
    CrossConeMirTypeBridgeSectionV1::try_new(
        MirTypeBridgeLocalInputV1 {
            provider,
            production,
            ordinary,
        },
        exports,
        Vec::new(),
        &[],
        &[],
        graph,
    )
    .unwrap()
}

fn open_graph(bytes: &[u8]) -> ValidatedGraphArtifact<'_> {
    DecodedSlibEnvelope::open(bytes, selection())
        .unwrap()
        .validate_graph()
        .unwrap()
}

fn selection() -> ValidatedLirTargetSelection {
    scoop_target_data::DARWIN_AARCH64_LLVM_22_1
}
