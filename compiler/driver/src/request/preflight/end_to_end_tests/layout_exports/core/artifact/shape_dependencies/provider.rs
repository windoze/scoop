//! Dependency ownership comes from verified objects of this layout producer.

use super::*;

pub(in super::super) fn objects(
    producer: &lir::SingleConeStrongLirOutput,
    layout: &lir::CrossConeLayoutAbiSectionV1<'_>,
    target: &scoop_toolchain::ResolvedTargetProfile,
) -> crate::object_production::layout::PreparedLayoutObjects {
    let selected =
        lir::StrongProductionDependencySelectionV2::empty(layout.provider(), target.lir_target())
            .unwrap();
    let production = producer
        .build_production_section_v2(
            ConeCoordinate::reserved_core(),
            &[],
            lir::EntryProductionSourceV1::Library,
            &selected,
            &[],
        )
        .unwrap()
        .validate_layout_abi(layout)
        .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let emitted = scoop_codegen::emit_object_set_v2(
        producer,
        production,
        directory.path(),
        scoop_codegen::ValidatedBackendProfile::from_selection(target.lir_target_selection())
            .unwrap(),
    )
    .unwrap();
    let generated = scoop_codegen::emit_c_bridge_object_set(
        producer,
        directory.path(),
        target.c_bridge_toolchain(),
    )
    .unwrap();
    crate::object_production::layout::prepare(emitted, &generated).unwrap()
}
