//! Replay nonempty helper imports from the assembled consumer's bytes.

pub(super) use super::super::super::super::lir_dependencies::reader::open;
use super::super::super::super::lir_dependencies::reader::{
    open_link, read, read_link, read_sections,
};
use super::*;

mod link;

pub(super) fn check(
    name: &str,
    provider: &scoop_slib::AssembledCrossConeLayoutArtifactV1,
    artifact: &scoop_slib::AssembledCrossConeLayoutArtifactV1,
    layout: &lir::CrossConeLayoutAbiSectionV1<'_>,
) {
    let compile = open(artifact);
    let link = DecodedSlibEnvelope::open(artifact.as_bytes(), artifact.target_selection())
        .unwrap()
        .validate_graph()
        .unwrap()
        .decode_cross_cone_layout_link_sections()
        .unwrap();
    assert_eq!(
        compile.artifact_fingerprint(),
        artifact.artifact_fingerprint()
    );
    assert_eq!(link.artifact_fingerprint(), artifact.artifact_fingerprint());
    for (compile, link) in [
        (
            encode(compile.hir_type_semantics_wire()).unwrap(),
            encode(link.hir_type_semantics_wire()).unwrap(),
        ),
        (
            encode(compile.mir_type_bridge_wire()).unwrap(),
            encode(link.mir_type_bridge_wire()).unwrap(),
        ),
        (
            encode(compile.lir_layout_abi_wire()).unwrap(),
            encode(link.lir_layout_abi_wire()).unwrap(),
        ),
        (
            encode(compile.lir_strong_production_wire()).unwrap(),
            encode(link.lir_strong_production_wire()).unwrap(),
        ),
    ] {
        assert_eq!(compile, link);
    }
    assert_eq!(
        encode(link.lir_layout_abi_wire()).unwrap(),
        encode(layout).unwrap()
    );
    let closure = read(provider, artifact);
    assert_eq!(closure.dependency_first().count(), 2);
    assert_eq!(closure.dependency_count(layout.provider()), Some(1));
    let current = closure.artifact(layout.provider()).unwrap();
    let expected = layout.selected().semantic_relations().collect::<Vec<_>>();
    assert_eq!(
        current.lir_dependency_transport().selected_relations(),
        expected
    );
    assert_eq!(current.lir_exports(), layout.exports());
    closure
        .replay_physical_imports()
        .map(|physical| {
            let current = physical.artifact(layout.provider()).unwrap();
            assert_eq!(current.lir_exports(), layout.exports());
            let imports = current.lir_physical_imports();
            assert_eq!(
                encode(imports).unwrap(),
                encode(layout.selected().physical_imports()).unwrap(),
            );
            for import in imports.records() {
                assert!(matches!(
                    import.subject(),
                    lir::ExternalStrongShapeSubjectV1::TypeDescriptor(_)
                        | lir::ExternalStrongShapeSubjectV1::TypeRegistration(_)
                ));
            }
        })
        .unwrap_or_else(|error| panic!("{name} shared physical replay: {error}"));
    link::check(provider, artifact, layout, &link);
}
