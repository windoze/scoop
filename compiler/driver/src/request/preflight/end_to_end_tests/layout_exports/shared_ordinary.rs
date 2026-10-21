use super::*;

#[test]
fn ordinary_bridge_selects_real_dependency_function_and_getter_calls() {
    let target = resolved_target().expect("ordinary bridges require a supported host target");
    let directory = tempfile::tempdir().unwrap();
    copy_trusted_core_sources(directory.path());
    let fixtures = crate::workspace_root().join("tests/fixtures/language/core-layout-exports");
    std::fs::copy(
        fixtures.join("shared-ordinary-standalone.scoop"),
        directory
            .path()
            .join("src/scoop.core/src/ordinary_probe.scoop"),
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
        },
    );
}
