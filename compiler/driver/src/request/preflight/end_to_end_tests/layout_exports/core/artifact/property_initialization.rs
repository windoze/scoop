//! Actual property reads close initialization uses across both artifact views.

use super::lir_dependencies::{assembly, reader};
use super::*;

mod odr;
mod physical;
mod registration_edges;
mod rejections;
mod wire;

pub(super) fn check(
    directory: &Path,
    target: &scoop_toolchain::ResolvedTargetProfile,
    core_input: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
    core_mir: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
    core_lir: &lir::CrossConeLayoutAbiSectionV1<'_>,
    core_artifact: &scoop_slib::AssembledCrossConeLayoutStrongArtifactV1,
) {
    physical::check_provider(core_input);
    let read = scoop_slib::read_cross_cone_layout_artifact_closure(
        scoop_slib::CrossConeArtifactClosureInput::completed(
            core_lir.provider(),
            target.lir_target_selection(),
            vec![],
            vec![],
            core_artifact.as_bytes(),
        ),
        target.c_bridge_toolchain().profile(),
    )
    .unwrap();
    let (semantic, link) = read.artifact(core_lir.provider()).unwrap();
    let provider_exports = semantic.lir_exports();
    assert_eq!(provider_exports, core_lir.exports());
    super::publication::check_provider(core_artifact, target.c_bridge_toolchain().profile());
    let owners = [link.defined_symbols().clone()];
    let provider = lir::ShapeLinkProviderV1::try_new(lir::ShapeLinkProviderPartsV1 {
        foundation: semantic.lir_foundation(),
        production: semantic.lir_strong_production(),
        ordinary: semantic.lir_cross_cone_bridge(),
        layouts: provider_exports.layouts(),
        callables: provider_exports.callables(),
        descriptors: provider_exports.descriptors(),
        dispatch: provider_exports.dispatch(),
    })
    .unwrap();
    let core = bootstrap_core(directory, target);
    let bytes = std::fs::read(core.artifact().path()).unwrap();
    odr::check(directory, target);
    for (family, name, count) in [
        ("m23-property-initialization", "standalone", 1),
        ("m23-property-initialization", "combined", 4),
        ("m23-extension-call-receivers", "standalone", 1),
        ("m23-extension-call-receivers", "combined", 4),
        ("m23-any-call-signatures", "standalone", 1),
        ("m23-any-call-signatures", "combined", 4),
        ("m23-link-object-contents", "standalone", 1),
        ("m23-link-object-contents", "combined", 4),
        ("m23-link-symbol-uses", "standalone", 1),
        ("m23-link-symbol-uses", "combined", 4),
    ] {
        let fixtures = crate::workspace_root().join("tests/fixtures").join(family);
        let source = std::fs::read_to_string(fixtures.join(format!("{name}.scoop"))).unwrap();
        support::with_pair(
            directory,
            target,
            &bytes,
            &source,
            Some((core_mir, core_lir)),
            Some(&provider),
            |input, _, lir_input, _, _| {
                let dependencies = scoop_mir_lower::MirTypeBridgeDependencyTablesV1 {
                    types: &[core_mir.types()],
                    callables: &[core_mir.callables()],
                    dispatch: &[core_mir.dispatch()],
                };
                let exports =
                    scoop_mir_lower::lower_type_bridge_exports(input, dependencies).unwrap();
                let projected = scoop_mir_lower::MirTypeBridgeSourceProjectionV1::from_input(
                    input,
                    dependencies,
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
                )
                .unwrap();
                assert_eq!(mir.initialization_uses().records().len(), count);
                rejections::check(input, core_input, &mir, core_mir, &projected);
                let (selected, initialization) = physical::select(
                    input.mir,
                    lir_input.lir,
                    &mir,
                    &provider,
                    provider_exports,
                    semantic.lir_strong_production(),
                );
                let registration = lir_input
                    .lir
                    .build_production_section_v2(
                        lir_input.coordinates[1].clone(),
                        &[core_mir.provider()],
                        lir::EntryProductionSourceV1::Library,
                        &selected,
                        &initialization,
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
                let exports =
                    scoop_lir_lower::lower_layout_abi_exports(input_lir, dependencies).unwrap();
                let source = scoop_lir_lower::LayoutAbiSourceProjectionV1::from_input(
                    input_lir,
                    dependencies,
                    &projected,
                )
                .unwrap();
                let layout = lir::CrossConeLayoutAbiSectionV1::try_new(
                    exports,
                    &[provider_exports],
                    selected.physical_imports().records().to_vec(),
                    &source,
                )
                .unwrap();
                let production = registration.validate_layout_abi(&layout).unwrap();
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
                super::link_materializations::check(core_artifact, &artifact);
                if family != "m23-property-initialization" {
                    snapshot(
                        &fixtures.join(format!("{name}.hir.snap")),
                        &hir::dump(&input.hir.output().export),
                    );
                    if family == "m23-extension-call-receivers" {
                        super::source_calls::check_receivers(input, core_artifact, &artifact);
                    } else if family == "m23-any-call-signatures" {
                        super::publication::check(
                            name,
                            input.public,
                            core_artifact,
                            &artifact,
                            target.c_bridge_toolchain().profile(),
                        );
                        super::source_calls::check_any(
                            &fixtures.join(format!("{name}.rejections.snap")),
                            input,
                            core_artifact,
                            &artifact,
                        );
                    } else if family == "m23-link-object-contents" {
                        super::link_object_contents::check(
                            name,
                            core_artifact,
                            &artifact,
                            target.c_bridge_toolchain().profile(),
                        );
                    } else if family == "m23-link-symbol-uses" {
                        super::link_symbol_uses::check(
                            &fixtures.join(format!("{name}.symbols.snap")),
                            core_artifact,
                            &artifact,
                            target.c_bridge_toolchain().profile(),
                        );
                    }
                }
                let physical_count = if family == "m23-any-call-signatures" && name == "combined" {
                    3
                } else {
                    1
                };
                assert_eq!(selected.physical_imports().records().len(), physical_count);
                let mut dump = format!("mir-uses={count}\n");
                if family == "m23-any-call-signatures" {
                    dump.push_str(&super::source_calls::check_any_link(
                        core_artifact,
                        &artifact,
                        target.c_bridge_toolchain().profile(),
                    ));
                }
                for (view, closure) in [
                    ("compile", reader::read(core_artifact, &artifact)),
                    ("link", reader::read_link(core_artifact, &artifact)),
                ] {
                    closure
                        .replay_physical_imports()
                        .map(|physical| {
                            let current = physical.artifact(mir.provider()).unwrap();
                            assert_eq!(current.link_sections().is_some(), view == "link");
                            assert_eq!(
                                current.lir_physical_imports().records().len(),
                                physical_count
                            );
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
                                "{view}: physical={physical_count} unit-edges={}\n",
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
