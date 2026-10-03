use super::*;

#[allow(clippy::too_many_arguments)]
pub(in super::super) fn assemble_with_production(
    directory: &Path,
    target: &scoop_toolchain::ResolvedTargetProfile,
    core: &scoop_slib::AssembledCrossConeLayoutArtifactV1,
    input: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
    lir: &lir::ConeLirOutput,
    mir: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
    layout: &lir::CrossConeLayoutAbiSectionV1<'_>,
    owners: &[scoop_slib::CanonicalDefinedLinkSymbolOwnerSetV1],
    production: lir::ConeProductionSectionV2,
) -> scoop_slib::AssembledCrossConeLayoutArtifactV1 {
    let coordinate = ConeCoordinate::new("dev.example", "layout-library", "0.1.0").unwrap();
    let emitted = scoop_codegen::emit_object_set_v2(
        lir,
        production,
        directory,
        scoop_codegen::ValidatedBackendProfile::from_selection(target.lir_target_selection())
            .unwrap(),
    )
    .unwrap();
    let generated =
        scoop_codegen::emit_c_bridge_object_set(lir, directory, target.c_bridge_toolchain())
            .unwrap();
    let mut foundation =
        hir::CanonicalHirFoundation::from_type_semantics_output(input.hir).unwrap();
    foundation
        .complete_cross_cone_interface_source_points(
            input.hir.output().export.module(),
            input.public,
        )
        .unwrap();
    let production =
        hir::CoreBootstrapInterfaceSectionV1::from_export(&input.hir.output().export).unwrap();
    let ordinary =
        scoop_lir_lower::lower_cross_cone_bridge_section(input.mir, input.ordinary, lir).unwrap();
    let core = reader::open(core);
    let metadata = crate::CrossConeArtifactMetadataInputV1::new(
        scoop_slib::ProducerRecord::new(concat!("scoopc/", env!("CARGO_PKG_VERSION"))).unwrap(),
        scoop_slib::ConeRecord::new(coordinate, ConeKind::Library, ConeSourceForm::Manifest)
            .unwrap(),
        vec![core.dependency_record()],
        &foundation,
        &production,
        input.public.clone(),
        input.mir.foundation(),
        input.mir.production(),
        input.ordinary,
        &ordinary,
    );
    crate::CrossConeLayoutArtifactMetadataInputV1::new(metadata, input.source, mir, layout)
        .assemble(emitted, &generated, owners)
        .unwrap()
}
