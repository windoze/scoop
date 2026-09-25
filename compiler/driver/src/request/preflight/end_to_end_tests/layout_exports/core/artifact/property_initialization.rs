//! Actual property reads close initialization uses across both artifact views.

use super::lir_dependencies::{assembly, reader};
use super::*;

mod physical;
mod registration_edges;
mod rejections;
mod wire;

pub(super) fn check(
    directory: &Path,
    target: &scoop_toolchain::ResolvedTargetProfile,
    core_input: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
    producer: &lir::SingleConeStrongLirOutput,
    core_mir: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
    core_lir: &lir::CrossConeLayoutAbiSectionV1<'_>,
    core_artifact: &scoop_slib::AssembledCrossConeLayoutStrongArtifactV1,
) {
    physical::check_provider(core_input);
    let prepared = super::shape_dependencies::provider::objects(producer, core_lir, target);
    let owners = [
        scoop_slib::CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(
            prepared.patch_sites.builtins().strong_relocations(),
        )
        .unwrap(),
    ];
    let ordinary = scoop_lir_lower::lower_cross_cone_bridge_section(
        core_input.mir,
        core_input.ordinary,
        producer,
    )
    .unwrap();
    let provider = lir::ShapeLinkProviderV1::try_new(
        lir::ShapeLinkProviderPartsV1 {
            foundation: &prepared.foundation,
            production: lir::ShapeLinkProductionV1::Reader(&prepared.production),
            ordinary: &ordinary,
            layouts: core_lir.layouts(),
            callables: core_lir.callables(),
            descriptors: core_lir.descriptors(),
            dispatch: core_lir.dispatch(),
        },
        &mut meter(),
    )
    .unwrap();
    let core = bootstrap_core(directory, target);
    let bytes = std::fs::read(core.artifact().path()).unwrap();
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-property-initialization");
    for (name, count) in [("standalone", 1), ("combined", 4)] {
        let source = std::fs::read_to_string(fixtures.join(format!("{name}.scoop"))).unwrap();
        support::with_pair(
            directory,
            target,
            &bytes,
            &source,
            Some((core_mir, core_lir)),
            |input, _, lir_input, _, _| {
                let dependencies = scoop_mir_lower::MirTypeBridgeDependencyTablesV1 {
                    types: &[core_mir.types()],
                    callables: &[core_mir.callables()],
                    dispatch: &[core_mir.dispatch()],
                };
                let exports =
                    scoop_mir_lower::lower_type_bridge_exports(input, dependencies, &mut meter())
                        .unwrap();
                let projected = scoop_mir_lower::MirTypeBridgeSourceProjectionV1::from_input(
                    input,
                    dependencies,
                    &mut meter(),
                )
                .unwrap();
                let mir = mir::CrossConeMirTypeBridgeSectionV1::try_new(
                    mir::MirTypeBridgeLocalAuthorityV1::Producer {
                        provider: input.mir.module().cone,
                        input: input.mir,
                        ordinary: input.ordinary,
                    },
                    exports,
                    &[core_mir],
                    &projected,
                    input.identities,
                    &mut meter(),
                )
                .unwrap();
                assert_eq!(mir.initialization_uses().records().len(), count);
                rejections::check(input, core_input, &mir, core_mir, &projected);
                let (selected, initialization) = physical::select(
                    lir_input.lir,
                    &mir,
                    &provider,
                    core_lir,
                    &prepared.production,
                );
                let registration = lir_input
                    .lir
                    .build_production_section_v2(
                        lir_input.coordinates[1].clone(),
                        &[core_mir.provider()],
                        lir::EntryProductionSourceV1::Library,
                        &selected,
                        &initialization,
                        &mut meter(),
                    )
                    .unwrap();
                let input_lir = scoop_lir_lower::LayoutAbiExportInputV1 {
                    bridge: mir.exports(),
                    registration: &registration,
                    ..lir_input
                };
                let dependencies = scoop_lir_lower::LayoutAbiExportDependenciesV1 {
                    layouts: &[core_lir.layouts()],
                    callables: &[core_lir.callables()],
                };
                registration_edges::check(
                    input_lir,
                    dependencies,
                    &projected,
                    &selected,
                    &initialization,
                );
                let exports = scoop_lir_lower::lower_layout_abi_exports(
                    input_lir,
                    dependencies,
                    &mut meter(),
                )
                .unwrap();
                let source = scoop_lir_lower::LayoutAbiSourceProjectionV1::from_input(
                    input_lir,
                    dependencies,
                    &projected,
                    &mut meter(),
                )
                .unwrap();
                let layout = lir::CrossConeLayoutAbiSectionV1::try_new(
                    exports,
                    &[core_lir],
                    selected.physical_imports().records().to_vec(),
                    &source,
                    &mut meter(),
                )
                .unwrap();
                let production = registration
                    .validate_layout_abi(&layout, &mut meter())
                    .unwrap();
                snapshot(
                    &fixtures.join(format!("{name}.mir.snap")),
                    &mir::dump(input.mir.module()),
                );
                snapshot(
                    &fixtures.join(format!("{name}.lir.snap")),
                    &lir::dump(lir_input.lir.module()),
                );
                let objects = tempfile::tempdir().unwrap();
                let artifact = assembly::assemble_with_production(
                    objects.path(),
                    target,
                    core_artifact,
                    input,
                    lir_input.lir,
                    &mir,
                    &layout,
                    &owners,
                    production,
                );
                super::source_calls::check(input, core_input, core_artifact, &artifact);
                let mut dump = format!("mir-uses={count}\n");
                for (view, closure) in [
                    ("compile", reader::read(core_artifact, &artifact)),
                    ("link", reader::read_link(core_artifact, &artifact)),
                ] {
                    closure
                        .with_replayed_physical_imports(|physical| {
                            let current = physical.artifact(mir.provider()).unwrap();
                            assert_eq!(current.link_sections().is_some(), view == "link");
                            assert_eq!(current.lir_physical_imports().records().len(), 1);
                            let units = current
                                .lir_strong_production()
                                .initialization_registrations();
                            let edges = units
                                .registrations()
                                .iter()
                                .flat_map(|unit| {
                                    unit.semantic()
                                        .dependencies()
                                        .iter()
                                        .map(|edge| (unit.semantic().unit(), edge))
                                })
                                .collect::<Vec<_>>();
                            assert_eq!(edges.len(), if name == "standalone" { 1 } else { 3 });
                            dump.push_str(&format!(
                                "{view}: physical=1 unit-edges={}\n",
                                edges.len()
                            ));
                            for (local, edge) in edges {
                                assert_eq!(edge.definition().provider(), core_mir.provider());
                                dump.push_str(&format!(
                                    "{local} -> {} {}\n",
                                    edge.definition().provider(),
                                    edge.unit()
                                ));
                            }
                        })
                        .unwrap();
                }
                snapshot(&fixtures.join(format!("{name}.artifact.snap")), &dump);
            },
        );
    }
}

fn snapshot(path: &Path, text: &str) {
    if std::env::var_os("SCOOP_UPDATE_PROPERTY_INITIALIZATION").is_some() {
        std::fs::write(path, text).unwrap();
    }
    assert_eq!(text, std::fs::read_to_string(path).unwrap());
}
