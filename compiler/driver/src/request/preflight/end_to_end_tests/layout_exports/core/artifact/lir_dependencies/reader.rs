use super::*;

pub(in super::super) fn open(
    artifact: &scoop_slib::AssembledCrossConeLayoutStrongArtifactV1,
) -> scoop_slib::DecodedCrossConeLayoutCompileSections<'_> {
    DecodedSlibEnvelope::open(artifact.as_bytes(), artifact.target_selection())
        .unwrap()
        .validate_graph()
        .unwrap()
        .decode_cross_cone_layout_compile_sections()
        .unwrap()
}

pub(in super::super) fn read<'a>(
    core: &'a scoop_slib::AssembledCrossConeLayoutStrongArtifactV1,
    artifact: &'a scoop_slib::AssembledCrossConeLayoutStrongArtifactV1,
) -> scoop_slib::LirDependencyGraphReplayedCrossConeLayoutClosure<'a> {
    read_sections(open(core), open(artifact))
}

pub(in super::super) fn open_link(
    artifact: &scoop_slib::AssembledCrossConeLayoutStrongArtifactV1,
) -> scoop_slib::DecodedCrossConeLayoutLinkSections<'_> {
    DecodedSlibEnvelope::open(artifact.as_bytes(), artifact.target_selection())
        .unwrap()
        .validate_graph()
        .unwrap()
        .decode_cross_cone_layout_link_sections()
        .unwrap()
}

pub(in super::super) fn read_link<'a>(
    core: &'a scoop_slib::AssembledCrossConeLayoutStrongArtifactV1,
    artifact: &'a scoop_slib::AssembledCrossConeLayoutStrongArtifactV1,
) -> scoop_slib::LirDependencyGraphReplayedCrossConeLayoutClosure<'a> {
    read_sections(
        open_link(core).into_shared_sections().unwrap(),
        open_link(artifact).into_shared_sections().unwrap(),
    )
}

pub(in super::super) fn read_sections<'a>(
    core: scoop_slib::DecodedCrossConeLayoutCompileSections<'a>,
    artifact: scoop_slib::DecodedCrossConeLayoutCompileSections<'a>,
) -> scoop_slib::LirDependencyGraphReplayedCrossConeLayoutClosure<'a> {
    let closure = scoop_slib::DecodedCrossConeLayoutCompileClosure::with_current_artifact(
        artifact.identity(),
        artifact.target_selection(),
        vec![core.identity()],
        vec![core],
        artifact,
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
    .validate_hir_declarations()
    .unwrap();
    closure
        .validate_mir_types()
        .unwrap()
        .validate_source_callables()
        .unwrap()
        .validate_lir_layouts()
        .unwrap()
        .validate_lir_callable_abis()
        .unwrap()
        .validate_lir_dispatch()
        .unwrap()
        .validate_lir_descriptors()
        .unwrap()
        .validate_lir_shape_support()
        .unwrap()
        .validate_ordinary_lir_bridges()
        .unwrap()
        .validate_lir_initialization_abi()
        .unwrap()
        .replay_lir_strong_production()
        .unwrap()
        .replay_mir_dependency_graph()
        .unwrap()
        .replay_lir_dependency_graph()
        .unwrap()
}

pub(super) fn check(
    name: &str,
    fixtures: &Path,
    core: &scoop_slib::AssembledCrossConeLayoutStrongArtifactV1,
    artifact: &scoop_slib::AssembledCrossConeLayoutStrongArtifactV1,
    layout: &lir::CrossConeLayoutAbiSectionV1<'_>,
) {
    let closure = read(core, artifact);
    assert_eq!(closure.dependency_first().count(), 2);
    assert_eq!(closure.dependency_count(layout.provider()), Some(1));
    let current = closure.artifact(layout.provider()).unwrap();
    let expected = layout.selected().semantic_relations().collect::<Vec<_>>();
    assert_eq!(
        current.lir_dependency_transport().selected_relations(),
        expected
    );
    assert_eq!(current.lir_exports().layouts(), layout.layouts());
    let link = DecodedSlibEnvelope::open(artifact.as_bytes(), artifact.target_selection())
        .unwrap()
        .validate_graph()
        .unwrap()
        .decode_cross_cone_layout_link_sections()
        .unwrap();
    assert_eq!(
        encode(link.lir_layout_abi_wire()).unwrap(),
        encode(layout).unwrap()
    );
    let mut dump = format!(
        "providers={}\ndirect={}\nselected={}\n",
        closure.dependency_first().count(),
        closure.direct_providers().len(),
        expected.len()
    );
    for relation in expected {
        dump.push_str(&format!(
            "{} {:?}\n",
            relation.provider(),
            relation.target()
        ));
    }
    let snapshot = fixtures.join(format!("{name}.lir.snap"));
    if std::env::var_os("SCOOP_UPDATE_LIR_DEPENDENCY_GRAPH").is_some() {
        std::fs::write(&snapshot, &dump).unwrap();
    }
    assert_eq!(dump, std::fs::read_to_string(snapshot).unwrap());
    closure
        .with_replayed_physical_imports(|physical| {
            assert_eq!(physical.dependency_first().count(), 2);
            let current = physical.artifact(layout.provider()).unwrap();
            assert_eq!(current.lir_exports(), layout.exports());
            assert!(current.lir_physical_imports().records().is_empty());
            assert!(current.link_sections().is_none());
        })
        .unwrap();
    read_link(core, artifact)
        .with_replayed_physical_imports(|physical| {
            assert_eq!(physical.dependency_first().count(), 2);
            for current in physical.dependency_first() {
                assert!(current.link_sections().is_some());
                assert!(current.lir_physical_imports().records().is_empty());
            }
            assert_eq!(
                physical.artifact(layout.provider()).unwrap().lir_exports(),
                layout.exports()
            );
        })
        .unwrap();
}
