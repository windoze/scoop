use super::*;
use scoop_lir_lower::{LayoutAbiExportDependenciesV1, LayoutAbiExportInputV1};

pub(in super::super) fn check_dependency_uses(
    source: hir::SharedTypeMetadataV1<'_>,
    ordinary: &mir::CrossConeMirBridgeSectionV1,
    input: LayoutAbiExportInputV1<'_>,
    dependencies: LayoutAbiExportDependenciesV1<'_>,
    terminal: &lir::CrossConeLirBridgeSectionV1,
    metadata: hir::SharedTypeMetadataV1<'_>,
    foundation: &lir::ConeLirFoundation,
) {
    let layouts = scoop_slib::replay_shared_mir_layouts(
        input.lir.module().meta.target_profile,
        input.bridge.types(),
        input.lir.foundation(),
        input.identities,
        dependencies.layouts,
    )
    .unwrap();
    let replay = |callables: &[&lir::CrossConeLirBridgeSectionV1]| {
        scoop_slib::replay_shared_ordinary_lir_bridge(
            input.lir.module().meta.target_profile,
            source,
            ordinary,
            &layouts,
            SharedOrdinaryLirBridgeDependenciesV1 {
                metadata: &[metadata],
                layouts: dependencies.layouts,
                callables,
            },
            input.lir.foundation(),
        )
    };
    let expected =
        scoop_lir_lower::lower_cross_cone_bridge_section(input.mir, ordinary, input.lir).unwrap();
    let actual = replay(&[terminal]).unwrap();
    assert_eq!(actual, expected);
    let wire: lir::DecodedCrossConeLirBridgeSectionV1 = decoded(&expected);
    assert_eq!(wire.validate_against(actual).unwrap(), expected);
    assert!(matches!(
        replay(&[terminal, terminal]),
        Err(Error::DependencySources)
    ));
    assert!(matches!(replay(&[]), Err(Error::DependencySources)));
    if !ordinary.selected().is_empty() {
        let absent = lir::CrossConeLirBridgeSectionV1::try_new(foundation, vec![], vec![]).unwrap();
        assert!(matches!(
            replay(&[&absent]),
            Err(Error::MissingSelectedExport { .. })
        ));
    }
}

#[test]
fn ordinary_bridge_replays_real_dependency_function_and_getter_calls() {
    let target = resolved_target().expect("ordinary bridges require a supported host target");
    let directory = tempfile::tempdir().unwrap();
    copy_trusted_core_sources(directory.path());
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-core-layout-exports");
    std::fs::copy(
        fixtures.join("shared-ordinary-standalone.scoop"),
        directory
            .path()
            .join("lib/scoop.core/src/ordinary_probe.scoop"),
    )
    .unwrap();
    let core = bootstrap_core(directory.path(), &target);
    let bytes = std::fs::read(core.artifact().path()).unwrap();
    let source = std::fs::read_to_string(fixtures.join("shared-ordinary-consumer.scoop")).unwrap();
    let ordinary = std::cell::RefCell::new(None);
    support::with_inspection(
        directory.path(),
        &target,
        &bytes,
        &source,
        |input, _| {
            assert_eq!(input.ordinary.selected().len(), 2);
            *ordinary.borrow_mut() = Some(input.ordinary.clone());
        },
        |input, _| {
            let ordinary = ordinary.borrow();
            let mir = ordinary.as_ref().unwrap();
            let lir = scoop_lir_lower::lower_cross_cone_bridge_section(input.mir, mir, input.lir)
                .unwrap();
            assert_eq!(lir.selected().len(), 2);
            let dump = format!(
                "MIR exports\n{:?}\nMIR selected\n{:?}\nLIR exports\n{:?}\nLIR selected\n{:?}\n",
                mir.exports(),
                mir.selected(),
                lir.exports(),
                lir.selected()
            );
            let snapshot = fixtures.join("shared-ordinary-consumer.snap");
            if std::env::var_os("SCOOP_UPDATE_CORE_LAYOUT_EXPORTS").is_some() {
                std::fs::write(&snapshot, &dump).unwrap();
            }
            assert_eq!(dump, std::fs::read_to_string(snapshot).unwrap());
        },
    );
}
