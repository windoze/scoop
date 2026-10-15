use super::*;

mod aliases;
mod bytes;
mod external_boxing;
pub(super) use external_boxing::check as check_boxing;
mod link_symbol_uses;
mod lir_dependencies;
mod machine_selection;
mod mir_constructors;
mod mir_equality;
mod mir_objects;
mod mir_source_callables;
mod mir_units;
mod property_initialization;
mod publication;
mod shape_dependencies;
mod source_calls;

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
    let generated =
        crate::generated_c::emit_c_bridge_object_set(lir, directory, target.c_bridge_toolchain())
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
    if name == "base" {
        bytes::check(name, &artifact, input.source, mir_section, layout);
    }
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

pub(super) fn check_mir(
    name: &str,
    input: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
    section: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
) {
    let foundation = hir::CanonicalHirFoundation::from_type_semantics_output(input.hir).unwrap();
    let metadata = hir::SharedTypeMetadataV1 {
        provider: input.mir.module().cone,
        identities: input.identities,
        foundation: &foundation,
        public: input.public,
    };
    mir_source_callables::check(name, metadata, input.ordinary, section);
    mir_constructors::check(name, metadata, section);
    mir_objects::check(
        name,
        metadata,
        input.source.representation_support(),
        section,
    );
    mir_equality::check(
        name,
        metadata,
        input.mir.production().strong_callable_bridges(),
        section,
    );
    mir_units::check(
        name,
        metadata,
        input.mir.production().strong_callable_bridges(),
        section,
    );
}
