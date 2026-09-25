use super::*;

mod bytes;
mod lir_dependencies;
mod mir_constructors;
mod mir_dispatch;
mod mir_equality;
mod mir_objects;
mod mir_source_callables;
mod mir_types;
mod mir_units;
mod type_foundations;

#[allow(clippy::too_many_arguments)]
pub(super) fn check(
    name: &str,
    directory: &Path,
    target: &scoop_toolchain::ResolvedTargetProfile,
    input: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
    lir: &lir::SingleConeStrongLirOutput,
    mir_section: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
    layout: &lir::CrossConeLayoutAbiSectionV1<'_>,
    objects: scoop_codegen::EmittedStrongObjectSetV2,
) {
    let mut foundation =
        hir::CanonicalHirFoundation::from_type_semantics_output(input.hir).unwrap();
    foundation
        .complete_cross_cone_interface_source_points(
            input.hir.output().export.module(),
            input.public,
            &mut scoop_wire::BudgetMeter::new(scoop_wire::DecodeLimits::default()),
        )
        .unwrap();
    let foundation = hir::OdrFreeHirFoundation::try_new(foundation).unwrap();
    let production =
        hir::CoreBootstrapInterfaceSectionV1::from_export(&input.hir.output().export).unwrap();
    let ordinary =
        scoop_lir_lower::lower_cross_cone_bridge_section(input.mir, input.ordinary, lir).unwrap();
    shared_ordinary::check(name, input, &foundation, lir, layout, &ordinary);
    shared_digests::check(name, lir.foundation(), objects.production());
    shared_initialization::check(
        name,
        input.mir.production(),
        lir,
        layout,
        objects.production(),
    );
    let generated =
        scoop_codegen::emit_c_bridge_object_set(lir, directory, target.c_bridge_toolchain())
            .unwrap();
    let metadata = crate::CrossConeStrongArtifactMetadataInputV1::new(
        scoop_slib::ProducerRecord::new(concat!("scoopc/", env!("CARGO_PKG_VERSION"))).unwrap(),
        scoop_slib::ConeRecord::new(
            ConeCoordinate::reserved_core(),
            ConeKind::Library,
            ConeSourceForm::Manifest,
        )
        .unwrap(),
        vec![],
        &foundation,
        &production,
        input.public.clone(),
        input.mir.foundation(),
        input.mir.production(),
        input.ordinary,
        &ordinary,
    );
    let artifact = crate::CrossConeLayoutArtifactMetadataInputV1::new(
        metadata,
        input.source.section(),
        mir_section,
        layout,
    )
    .assemble(objects, &generated, &[], &mut meter())
    .unwrap_or_else(|error| panic!("{name} layout artifact assembly: {error:?}"));
    bytes::check(
        name,
        &artifact,
        input.source.section(),
        mir_section,
        layout,
        &production,
        input.mir.foundation(),
        input.mir.production().strong_callable_bridges(),
        input.ordinary,
    );
    if name == "base" {
        lir_dependencies::check(&artifact, mir_section, layout);
    }
}
