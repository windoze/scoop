//! Actual HIR operations select foreign helpers before machine consumption.

use super::*;

mod lir_reader;
mod lower;
mod machine;
mod mir_reader;

pub(super) fn check(
    core_mir: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
    core_lir: &lir::CrossConeLayoutAbiSectionV1<'_>,
    artifact: &scoop_slib::AssembledCrossConeLayoutStrongArtifactV1,
    target: &scoop_toolchain::ResolvedTargetProfile,
) {
    let read = scoop_slib::read_cross_cone_layout_artifact_closure(
        scoop_slib::CrossConeArtifactClosureInput::completed(
            core_lir.provider(),
            target.lir_target_selection(),
            vec![],
            vec![],
            artifact.as_bytes(),
        ),
        target.c_bridge_toolchain().profile(),
    )
    .unwrap();
    let (semantic, link) = read.artifact(core_lir.provider()).unwrap();
    let mir_dependency = semantic
        .mir_type_bridge()
        .dependency_view(semantic.initialization_units());
    let mir_exports = mir_dependency.exports();
    let provider_exports = semantic.lir_exports();
    assert_eq!(provider_exports, core_lir.exports());
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
    let sysroot = tempfile::tempdir().unwrap();
    let installed = bootstrap_core(sysroot.path(), target);
    let bytes = std::fs::read(installed.artifact().path()).unwrap();
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-shape-dependency-graph");
    for (name, names) in [
        ("standalone", &["Int"][..]),
        ("combined", &["Int", "String", "Unit"][..]),
    ] {
        let source = std::fs::read_to_string(fixtures.join(format!("{name}.scoop"))).unwrap();
        lower::with_mir(
            sysroot.path(),
            target,
            &bytes,
            &source,
            |input, callables| {
                let dependencies = scoop_mir_lower::MirTypeBridgeDependencyTablesV1 {
                    types: &[mir_exports.types()],
                    callables: &[mir_exports.callables()],
                    dispatch: &[mir_exports.dispatch()],
                };
                let exports =
                    scoop_mir_lower::lower_type_bridge_exports(input, dependencies).unwrap();
                let projection = scoop_mir_lower::lower_type_bridge_dependencies(input).unwrap();
                let uses = &projection;
                let mut shapes = uses
                    .iter()
                    .filter_map(|usage| match usage.target() {
                        mir::MirTypeBridgeTargetV1::ShapeSupport(owner) => {
                            Some((usage.provider(), owner))
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let mut expected = names
                    .iter()
                    .map(|name| {
                        (
                            core_mir.provider(),
                            source_named(core_mir, input.identities, name),
                        )
                    })
                    .collect::<Vec<_>>();
                shapes.sort_unstable();
                expected.sort_unstable();
                assert_eq!(shapes, expected);
                let section = mir::CrossConeMirTypeBridgeSectionV1::try_new(
                    mir::MirTypeBridgeLocalInputV1 {
                        provider: input.mir.module().cone,

                        production: (input.mir).production(),
                        ordinary: input.ordinary,
                    },
                    exports,
                    scoop_mir_lower::lower_type_bridge_initialization_units(input.mir).unwrap(),
                    &[mir_dependency],
                    &projection,
                    input.identities,
                )
                .unwrap();
                mir_reader::check(name, &fixtures, input, &section, mir_dependency, &expected);
                lir_reader::check(input, core_lir, &expected);
                machine::check(
                    name,
                    &fixtures,
                    input,
                    callables,
                    &expected,
                    machine::Provider {
                        view: &provider,
                        layout: provider_exports,
                        target,
                        artifact,
                        owners: &owners,
                        string: core_lir
                            .shape_support()
                            .records()
                            .iter()
                            .find(|shape| {
                                shape.source_nominal()
                                    == source_named(core_mir, input.identities, "String")
                            })
                            .unwrap()
                            .exact(),
                    },
                    machine::PublicationInput {
                        bridge: &section,
                        source: &projection,
                    },
                );
            },
        );
    }
    assert_eq!(core_lir.target_profile(), target.lir_target());
}

fn source_named(
    core: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
    identities: &ValidatedIdentityGraph,
    name: &str,
) -> scoop_identity::PersistentTypeId {
    core.shape_support().records().iter().find_map(|record| {
        let source = identities.canonical_key::<_, scoop_identity::SourceDeclarationKey>(record.source()).unwrap();
        matches!(source.name(), scoop_identity::DeclarationName::Named(actual) if actual.as_str() == name).then_some(record.source())
    }).unwrap_or_else(|| panic!("missing provider shape {name}"))
}

fn snapshot(path: &Path, text: &str) {
    if std::env::var_os("SCOOP_UPDATE_SHAPE_DEPENDENCY_GRAPH").is_some() {
        std::fs::write(path, text).unwrap();
    }
    assert_eq!(text, std::fs::read_to_string(path).unwrap());
}
