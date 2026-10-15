use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn check(
    name: &str,
    artifact: &scoop_slib::AssembledCrossConeLayoutArtifactV1,
    hir: &hir::CrossConeTypeSemanticsSectionV1,
    mir: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
    lir: &lir::CrossConeLayoutAbiSectionV1<'_>,
) {
    let hir_bytes = encode(hir).unwrap();
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
    let declarations = scoop_slib::DecodedCrossConeLayoutCompileClosure::with_current_artifact(
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
    let callables = declarations
        .read_mir_records()
        .unwrap_or_else(|error| panic!("{name} MIR record reading: {error}"));
    let complete = callables
        .validate_lir_layouts()
        .unwrap()
        .validate_ordinary_lir_bridges()
        .unwrap()
        .validate_lir_callable_abis()
        .unwrap()
        .validate_lir_dispatch()
        .unwrap()
        .validate_lir_descriptors()
        .unwrap()
        .validate_lir_shape_support()
        .unwrap()
        .read_lir_production_data()
        .unwrap()
        .resolve_mir_dependencies()
        .unwrap()
        .resolve_lir_dependencies()
        .unwrap()
        .read_physical_imports()
        .unwrap();
    assert_eq!(complete.current(), ConeIdentity::CORE);
    assert_eq!(complete.dependency_first().count(), 1);
    let current = complete.artifact(ConeIdentity::CORE).unwrap();
    assert_eq!(current.lir_exports(), lir.exports());
    assert_eq!(current.initialization_units(), mir.initialization_units());
    assert!(current.lir_physical_imports().records().is_empty());
}
