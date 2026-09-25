use super::*;

mod lir_exports;
mod lir_strong;

#[allow(clippy::too_many_arguments)]
pub(super) fn check(
    name: &str,
    artifact: &scoop_slib::AssembledCrossConeLayoutStrongArtifactV1,
    hir: &hir::CrossConeTypeSemanticsSectionV1,
    mir: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
    lir: &lir::CrossConeLayoutAbiSectionV1<'_>,
    hir_core: &hir::CoreBootstrapInterfaceSectionV1,
    mir_foundation: &mir::OdrFreeMirFoundation,
    strong: &mir::StrongCallableBridgeSurfaceV1,
    ordinary: &mir::CrossConeMirBridgeSectionV1,
) {
    let hir_bytes = encode(&hir.index_for_wire().unwrap()).unwrap();
    let open = || {
        DecodedSlibEnvelope::open(artifact.as_bytes(), artifact.target_selection())
            .unwrap()
            .validate_graph()
            .unwrap()
    };
    let compile = open().decode_cross_cone_layout_compile_sections().unwrap();
    let link = open().decode_cross_cone_layout_link_sections().unwrap();
    assert_eq!(
        compile.artifact_fingerprint(),
        artifact.artifact_fingerprint()
    );
    assert_eq!(link.artifact_fingerprint(), artifact.artifact_fingerprint());
    assert_eq!(
        encode(compile.hir_type_semantics_wire()).unwrap(),
        hir_bytes
    );
    assert_eq!(
        encode(compile.mir_type_bridge_wire()).unwrap(),
        encode(mir).unwrap()
    );
    assert_eq!(
        encode(compile.lir_layout_abi_wire()).unwrap(),
        encode(lir).unwrap()
    );
    assert_eq!(encode(link.hir_type_semantics_wire()).unwrap(), hir_bytes);
    assert_eq!(
        encode(link.mir_type_bridge_wire()).unwrap(),
        encode(mir).unwrap()
    );
    assert_eq!(
        encode(link.lir_layout_abi_wire()).unwrap(),
        encode(lir).unwrap()
    );
    assert_eq!(
        encode(compile.lir_strong_production_wire()).unwrap(),
        encode(link.lir_strong_production_wire()).unwrap()
    );
    let fingerprints = compile.semantic_fingerprints();
    let dump = format!(
        "artifact={}\ncode={:?}\nruntime={:?}\n",
        artifact.artifact_fingerprint(),
        fingerprints.code(),
        fingerprints.runtime_image()
    );
    let mut declarations = scoop_slib::DecodedCrossConeLayoutCompileClosure::with_current_artifact(
        ConeIdentity::CORE,
        artifact.target_selection(),
        Vec::new(),
        Vec::new(),
        compile,
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
    assert_eq!(declarations.current(), ConeIdentity::CORE);
    assert_eq!(declarations.dependency_first().count(), 1);
    assert!(declarations.artifact(ConeIdentity::CORE).is_some());
    let checked = declarations.validate_type_foundations().unwrap();
    assert_eq!(checked.len(), 1);
    assert_eq!(checked[0].provider(), ConeIdentity::CORE);
    assert_eq!(checked[0].facts().records(), hir.exact_facts().records());
    assert_eq!(
        checked[0].representations().table(),
        hir.representation_support()
    );
    super::type_foundations::check(checked[0]);
    super::mir_types::check(name, checked[0], hir_core, mir_foundation, mir);
    super::mir_source_callables::check(name, checked[0], mir_foundation, ordinary, mir);
    super::mir_constructors::check(name, checked[0], mir_foundation, mir);
    super::mir_objects::check(name, checked[0], mir);
    super::mir_dispatch::check(name, checked[0], mir_foundation, mir);
    super::mir_equality::check(name, checked[0], mir_foundation, strong, mir);
    let units = super::mir_units::check(name, checked[0], mir_foundation, strong, mir);
    if name == "base" {
        super::type_foundations::dependencies::check(checked[0]);
    }
    let types = declarations.validate_mir_types().unwrap();
    assert_eq!(types.current(), ConeIdentity::CORE);
    assert_eq!(types.target_selection(), artifact.target_selection());
    assert!(types.direct_providers().is_empty());
    assert_eq!(types.dependency_count(ConeIdentity::CORE), Some(0));
    assert_eq!(types.dependency_first().count(), 1);
    let current = types.artifact(ConeIdentity::CORE).unwrap();
    assert_eq!(current.types(), mir.types());
    assert_eq!(current.shape_support(), mir.shape_support());
    assert_eq!(
        encode(current.lir_layout_abi_wire()).unwrap(),
        encode(lir).unwrap()
    );
    let callables = types
        .validate_source_callables()
        .unwrap_or_else(|error| panic!("{name} shared source callable replay: {error}"));
    assert_eq!(callables.current(), ConeIdentity::CORE);
    assert_eq!(callables.target_selection(), artifact.target_selection());
    assert!(callables.direct_providers().is_empty());
    assert_eq!(callables.dependency_count(ConeIdentity::CORE), Some(0));
    assert_eq!(callables.dependency_first().count(), 1);
    let current = callables.artifact(ConeIdentity::CORE).unwrap();
    assert_eq!(current.types(), mir.types());
    assert_eq!(current.shape_support(), mir.shape_support());
    assert_eq!(current.callables(), mir.callables());
    assert_eq!(current.object_values(), mir.object_values());
    assert_eq!(current.dispatch(), mir.dispatch());
    assert_eq!(current.initialization_units(), units);
    assert_eq!(
        encode(current.lir_layout_abi_wire()).unwrap(),
        encode(lir).unwrap()
    );
    lir_exports::check(name, artifact, callables, &units, mir, lir);
    let snapshot = crate::workspace_root().join(format!(
        "tests/fixtures/m23-core-layout-exports/{name}.artifact.snap"
    ));
    if std::env::var_os("SCOOP_UPDATE_CORE_LAYOUT_EXPORTS").is_some() {
        std::fs::write(&snapshot, &dump).unwrap();
    }
    assert_eq!(dump, std::fs::read_to_string(snapshot).unwrap());
}
