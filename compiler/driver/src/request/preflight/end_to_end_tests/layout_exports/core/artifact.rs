use super::*;

mod aliases;
mod bytes;
mod external_boxing;
mod link_archive;
mod link_materializations;
mod link_object_contents;
mod link_symbol_uses;
mod lir_dependencies;
mod machine_selection;
mod mir_constructors;
mod mir_coroutines;
mod mir_dispatch;
mod mir_equality;
mod mir_objects;
mod mir_source_callables;
mod mir_types;
mod mir_units;
mod property_initialization;
mod publication;
mod shape_dependencies;
mod source_calls;
mod type_foundations;

#[allow(clippy::too_many_arguments)]
pub(super) fn check(
    name: &str,
    directory: &Path,
    target: &scoop_toolchain::ResolvedTargetProfile,
    input: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
    lir: &lir::ConeLirOutput,
    mir_section: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
    layout: &lir::CrossConeLayoutAbiSectionV1<'_>,
    objects: scoop_codegen::EmittedConeObjectSetV2,
) {
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
    shared_ordinary::check(name, input, &foundation, lir, layout, &ordinary);
    shared_digests::check(name, lir.foundation(), objects.production());
    shared_initialization::check(input.mir.production(), &ordinary, lir);
    external_boxing::check(name, target, lir, layout, &ordinary, objects.production());
    let generated =
        scoop_codegen::emit_c_bridge_object_set(lir, directory, target.c_bridge_toolchain())
            .unwrap();
    let metadata = crate::CrossConeArtifactMetadataInputV1::new(
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
        input.source,
        mir_section,
        layout,
    )
    .assemble(objects, &generated, &[])
    .unwrap_or_else(|error| panic!("{name} layout artifact assembly: {error:?}"));
    bytes::check(
        name,
        &artifact,
        input.source,
        mir_section,
        layout,
        &production,
        input.mir.foundation(),
        input.mir.production().strong_callable_bridges(),
        input.ordinary,
    );
    if name.starts_with("shared-aliases-") {
        aliases::check(&artifact, target);
    }
    if name == "base" {
        shape_dependencies::check(mir_section, layout, &artifact, target);
        lir_dependencies::check(&artifact, input.ordinary, mir_section, layout);
    }
    if name == "property-initialization-provider" {
        property_initialization::check(directory, target, input, mir_section, layout, &artifact);
    }
}
