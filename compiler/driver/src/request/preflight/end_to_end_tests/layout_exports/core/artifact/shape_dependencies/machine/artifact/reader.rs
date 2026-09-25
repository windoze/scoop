//! Replay nonempty helper imports from the assembled consumer's bytes.

pub(super) use super::super::super::super::lir_dependencies::reader::open;
use super::super::super::super::lir_dependencies::reader::{
    open_link, read, read_link, read_sections,
};
use super::*;

mod link;

pub(super) fn check(
    name: &str,
    fixtures: &Path,
    provider: &scoop_slib::AssembledCrossConeLayoutStrongArtifactV1,
    artifact: &scoop_slib::AssembledCrossConeLayoutStrongArtifactV1,
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
    let fingerprints = compile.semantic_fingerprints();
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
    let mut dump = format!(
        "artifact={}\ncode={:?}\nruntime={:?}\nproviders={}\ndirect={}\nselected={}\n",
        artifact.artifact_fingerprint(),
        fingerprints.code(),
        fingerprints.runtime_image(),
        closure.dependency_first().count(),
        closure.direct_providers().len(),
        expected.len(),
    );
    closure
        .with_replayed_physical_imports(|physical| {
            let current = physical.artifact(layout.provider()).unwrap();
            assert_eq!(current.lir_exports(), layout.exports());
            let imports = current.lir_physical_imports();
            assert_eq!(
                encode(imports).unwrap(),
                encode(layout.selected().physical_imports()).unwrap(),
            );
            assert_eq!(
                imports.records().len(),
                if name == "combined" { 2 } else { 1 }
            );
            dump.push_str(&format!("physical={}\n", imports.records().len()));
            for import in imports.records() {
                assert!(matches!(
                    import.subject(),
                    lir::ExternalStrongShapeSubjectV1::TypeDescriptor(_)
                ));
                dump.push_str(&format!(
                    "{} {:?} {:?}\n",
                    import.provider(),
                    import.subject(),
                    import.required_definition()
                ));
            }
        })
        .unwrap_or_else(|error| panic!("{name} shared physical replay: {error}"));
    dump.push_str(&link::check(provider, artifact, layout, &link));
    snapshot(&fixtures.join(format!("{name}.artifact.snap")), &dump);
}
