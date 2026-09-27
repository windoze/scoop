//! Assemble the actual consumer with this provider's verified object owners.

use super::*;

mod reader;

pub(super) struct Destination<'a> {
    pub directory: &'a Path,
    pub coordinate: &'a ConeCoordinate,
    pub fixtures: &'a Path,
    pub name: &'a str,
}

pub(super) fn check(
    input: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
    output: &lir::ConeLirOutput,
    source: PublicationInput<'_, '_>,
    layout: &lir::CrossConeLayoutAbiSectionV1<'_>,
    emitted: scoop_codegen::EmittedStrongObjectSetV2,
    provider: Provider<'_, '_>,
    destination: Destination<'_>,
) {
    let generated = scoop_codegen::emit_c_bridge_object_set(
        output,
        destination.directory,
        provider.target.c_bridge_toolchain(),
    )
    .unwrap();
    let mut foundation =
        hir::CanonicalHirFoundation::from_type_semantics_output(input.hir).unwrap();
    foundation
        .complete_cross_cone_interface_source_points(
            input.hir.output().export.module(),
            input.public,
        )
        .unwrap();
    let foundation = hir::OdrFreeHirFoundation::try_new(foundation).unwrap();
    let production =
        hir::CoreBootstrapInterfaceSectionV1::from_export(&input.hir.output().export).unwrap();
    let ordinary =
        scoop_lir_lower::lower_cross_cone_bridge_section(input.mir, input.ordinary, output)
            .unwrap();
    let dependency = reader::open(provider.artifact).dependency_record();
    let metadata = crate::CrossConeStrongArtifactMetadataInputV1::new(
        scoop_slib::ProducerRecord::new(concat!("scoopc/", env!("CARGO_PKG_VERSION"))).unwrap(),
        scoop_slib::ConeRecord::new(
            destination.coordinate.clone(),
            ConeKind::Library,
            ConeSourceForm::Manifest,
        )
        .unwrap(),
        vec![dependency],
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
        source.bridge,
        layout,
    )
    .assemble(emitted, &generated, provider.owners)
    .unwrap_or_else(|error| panic!("{} layout artifact assembly: {error:?}", destination.name));
    reader::check(
        destination.name,
        destination.fixtures,
        provider.artifact,
        &artifact,
        layout,
    );
    super::super::super::link_symbol_uses::check(
        &destination
            .fixtures
            .join(format!("{}.symbols.snap", destination.name)),
        provider.artifact,
        &artifact,
        provider.target.c_bridge_toolchain().profile(),
    );
}
