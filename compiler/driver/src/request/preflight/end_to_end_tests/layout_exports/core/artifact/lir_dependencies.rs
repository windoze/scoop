//! Ordinary source artifacts replay their nonempty graph with a real provider.

use super::*;

pub(super) mod assembly;
pub(super) mod corruption;
pub(super) mod reader;

pub(super) fn check(
    core: &scoop_slib::AssembledCrossConeLayoutStrongArtifactV1,
    core_mir: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
    core_lir: &lir::CrossConeLayoutAbiSectionV1<'_>,
) {
    let target = resolved_target().expect("dependency fixtures require the host toolchain");
    let sysroot = tempfile::tempdir().unwrap();
    let installed = bootstrap_core(sysroot.path(), &target);
    let core_bytes = std::fs::read(installed.artifact().path()).unwrap();
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-lir-dependency-graph");
    for name in ["standalone", "combined"] {
        let source = std::fs::read_to_string(fixtures.join(format!("{name}.scoop"))).unwrap();
        support::with_pair(
            sysroot.path(),
            &target,
            &core_bytes,
            &source,
            None,
            None,
            |mir_input, _, lir_input, _, owners| {
                let dependencies = scoop_mir_lower::MirTypeBridgeDependencyTablesV1 {
                    types: &[core_mir.types()],
                    callables: &[core_mir.callables()],
                    dispatch: &[core_mir.dispatch()],
                };
                let exports =
                    scoop_mir_lower::lower_type_bridge_exports(mir_input, dependencies).unwrap();
                let source = scoop_mir_lower::lower_type_bridge_dependencies(mir_input).unwrap();
                let mir = mir::CrossConeMirTypeBridgeSectionV1::try_new(
                    mir::MirTypeBridgeLocalInputV1 {
                        provider: mir_input.mir.module().cone,

                        production: (mir_input.mir).production(),
                        ordinary: mir_input.ordinary,
                    },
                    exports,
                    scoop_mir_lower::lower_type_bridge_initialization_units(mir_input.mir).unwrap(),
                    &[core_mir.dependency_view()],
                    &source,
                    mir_input.identities,
                )
                .unwrap_or_else(|error| panic!("{name} MIR dependency section: {error}"));
                let input = scoop_lir_lower::LayoutAbiExportInputV1 {
                    bridge: mir.exports(),
                    ..lir_input
                };
                let dependencies = scoop_lir_lower::LayoutAbiExportDependenciesV1 {
                    layouts: &[core_lir.layouts()],
                    callables: &[core_lir.callables()],
                };
                let exports =
                    scoop_lir_lower::lower_layout_abi_exports(input, dependencies).unwrap();
                let source = scoop_lir_lower::lower_layout_abi_dependencies(
                    input,
                    dependencies,
                    &exports,
                    &source,
                    &[],
                )
                .unwrap();
                let layout = lir::CrossConeLayoutAbiSectionV1::try_new(
                    exports,
                    &[core_lir.exports()],
                    vec![],
                    &source,
                )
                .unwrap_or_else(|error| panic!("{name} LIR dependency section: {error}"));
                assert!(!layout.selected().is_empty());
                assert!(layout.selected().physical_imports().records().is_empty());
                corruption::check(mir_input, &layout, core_lir);
                let directory = tempfile::tempdir().unwrap();
                let artifact = assembly::assemble(
                    directory.path(),
                    &target,
                    core,
                    mir_input,
                    input.lir,
                    &mir,
                    &layout,
                    owners,
                );
                reader::check(name, &fixtures, core, &artifact, &layout);
            },
        );
    }
}
