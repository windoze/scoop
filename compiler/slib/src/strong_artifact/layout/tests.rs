mod support;

use scoop_identity::ConeCoordinate;
use scoop_mir::*;
use scoop_wire::{decode_canonical, encode};

use super::*;
use crate::{
    CrossConeLayoutProductionManifestV1, DecodedSingleConeProductionManifestV1,
    DecodedSlibEnvelope, SlibMember, ValidatedGraphArtifact,
};
use crate::{EmptyLayoutCodeFixture, with_empty_layout_code_fixture};
use support::{
    empty_hir_interface, empty_hir_type_semantics, hir_foundation_and_production, mir_sections,
};

#[test]
fn layout_writer_is_reproducible_and_both_raw_views_accept_every_exact_projection() {
    let (first, code) = write_artifact(WriterMutation::None);
    let first = first.unwrap();
    let (second, _) = write_artifact(WriterMutation::None);
    let second = second.unwrap();
    assert_eq!(first.as_bytes(), second.as_bytes());
    assert_eq!(first.target_selection(), selection());

    let compile = open_graph(first.as_bytes())
        .decode_cross_cone_layout_compile_sections()
        .unwrap();
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
    let expected_identity = LinkIdentityClosureSectionV1::from_verified_layout_code(&code).unwrap();
    assert_eq!(
        link.link_identity_closure_wire()
            .clone()
            .validate_layout(&code)
            .unwrap(),
        expected_identity
    );
    let decoded_manifest = decode_canonical::<DecodedSingleConeProductionManifestV1>(
        &encode(link.production_manifest_wire()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        decoded_manifest.validate_layout(&code).unwrap(),
        CrossConeLayoutProductionManifestV1::from_verified_code(code)
    );
}

#[test]
fn layout_writer_rejects_mismatched_layout_imports() {
    let (result, _) = write_artifact(WriterMutation::LayoutImports);
    assert!(matches!(
        result,
        Err(CrossConeLayoutStrongArtifactWriteError::LayoutSemanticImportMismatch)
    ));
}

#[test]
fn layout_writer_rejects_mismatched_callable_imports() {
    let (result, _) = write_artifact(WriterMutation::CallableImports);
    assert!(matches!(
        result,
        Err(CrossConeLayoutStrongArtifactWriteError::CallableSemanticImportMismatch)
    ));
}

#[test]
fn layout_writer_rejects_mismatched_object_bytes() {
    let (result, _) = write_artifact(WriterMutation::ObjectBytes);
    assert!(matches!(
        result,
        Err(CrossConeLayoutStrongArtifactWriteError::LinkObjectMismatch { .. })
    ));
}

#[test]
fn layout_writer_rejects_code_proof_for_another_manifest_identity() {
    let (result, _) = write_artifact(WriterMutation::ManifestIdentity);
    assert!(matches!(
        result,
        Err(
            CrossConeLayoutStrongArtifactWriteError::ComponentProducerMismatch {
                component: "Code",
                ..
            }
        )
    ));
}

#[test]
fn layout_raw_decoders_reject_a_corrupted_archive() {
    let (artifact, _) = write_artifact(WriterMutation::None);
    let artifact = artifact.unwrap();
    let mut bytes = artifact.as_bytes().to_vec();
    bytes.pop();
    assert!(DecodedSlibEnvelope::open(&bytes, selection()).is_err());
}

#[derive(Clone, Copy)]
enum WriterMutation {
    None,
    CallableImports,
    LayoutImports,
    ObjectBytes,
    ManifestIdentity,
}

fn write_artifact(
    mutation: WriterMutation,
) -> (
    Result<AssembledCrossConeLayoutStrongArtifactV1, CrossConeLayoutStrongArtifactWriteError>,
    VerifiedCodeFingerprintV2,
) {
    with_empty_layout_code_fixture(|lir| {
        let code = lir.code.code().clone();
        let result = write_artifact_from_fixture(lir, mutation);
        (result, code)
    })
}

fn write_artifact_from_fixture(
    mut lir: EmptyLayoutCodeFixture<'_>,
    mutation: WriterMutation,
) -> Result<AssembledCrossConeLayoutStrongArtifactV1, CrossConeLayoutStrongArtifactWriteError> {
    let provider = lir.cone.identity();
    let (hir_foundation, hir_production) = hir_foundation_and_production(&lir.cone);
    let hir_type_semantics = empty_hir_type_semantics();
    let (mir_foundation, mir_production, mir_ordinary) = mir_sections(provider);
    let graph = scoop_identity::PendingIdentityValidation::new()
        .finish()
        .unwrap();
    let mir_type_bridge = empty_mir_type_bridge(provider, &mir_production, &mir_ordinary, &graph);

    if matches!(mutation, WriterMutation::ObjectBytes) {
        let record = lir.link_objects[0].record();
        lir.link_objects = vec![
            SlibMember::new(
                provider,
                record.stable_key().clone(),
                record.role().clone(),
                vec![0],
            )
            .unwrap(),
        ];
    }
    if matches!(mutation, WriterMutation::ManifestIdentity) {
        lir.cone = crate::ConeRecord::new(
            ConeCoordinate::new("test", "wrong-layout-writer", "0.0.0").unwrap(),
            crate::ConeKind::Library,
            crate::ConeSourceForm::Manifest,
        )
        .unwrap();
    }

    let layout = if matches!(mutation, WriterMutation::LayoutImports) {
        lir.mismatched_layout
    } else {
        lir.layout
    };
    let ordinary = if matches!(mutation, WriterMutation::CallableImports) {
        lir.mismatched_ordinary
    } else {
        lir.ordinary
    };
    AssembledCrossConeLayoutStrongArtifactV1::write(CrossConeLayoutStrongArtifactInputV1::new(
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
        ordinary,
        layout,
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
    ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1
}
