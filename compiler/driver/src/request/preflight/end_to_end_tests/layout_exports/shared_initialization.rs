use super::*;
use scoop_slib::SharedLirInitializationAbiValidationError as Error;

mod rejections;

pub(super) fn check(
    name: &str,
    mir: &mir::CoreBootstrapBridgeSectionV1,
    lir: &lir::SingleConeStrongLirOutput,
    layout: &lir::CrossConeLayoutAbiSectionV1<'_>,
    strong: &lir::StrongProductionSectionV2,
) {
    let replay = |callables: &mir::StrongCallableBridgeSurfaceV1,
                  layouts: &lir::CanonicalExactLayoutExportsV1| {
        scoop_slib::replay_shared_initialization_abi(
            lir.module().meta.target_profile,
            callables,
            layouts,
            &[],
            lir.foundation(),
        )
    };
    let callables = mir.strong_callable_bridges();
    let expected =
        replay(callables, layout.layouts()).unwrap_or_else(|error| panic!("{name}: {error}"));
    assert_eq!(expected.as_deref(), strong.initialization_cycle_abi());
    if name.starts_with("shared-init-abi-") {
        rejections::check(callables, layout.layouts(), lir.foundation(), &replay);
        let abi = expected.as_deref().unwrap();
        let snapshot = crate::workspace_root()
            .join("tests/fixtures/m23-core-layout-exports")
            .join(format!("{name}.initialization-abi.snap"));
        let dump = format!(
            "target={:?}\nabi={:?}\nroot={:?}\nconvention={:?}\nsymbol={:?}\ndefinition={:?}\n",
            abi.target(),
            abi.abi_signature(),
            abi.root_plan(),
            abi.calling_convention(),
            abi.expected_symbol(),
            abi.required_definition()
        );
        if std::env::var_os("SCOOP_UPDATE_CORE_LAYOUT_EXPORTS").is_some() {
            std::fs::write(&snapshot, &dump).unwrap();
        }
        assert_eq!(dump, std::fs::read_to_string(snapshot).unwrap());
    }
}

pub(super) fn check_consumption(
    input: scoop_lir_lower::LayoutAbiExportInputV1<'_>,
    layouts: &lir::CanonicalExactLayoutExportsV1,
    dependencies: &[&lir::CanonicalExactLayoutExportsV1],
) {
    assert_eq!(
        scoop_slib::replay_shared_initialization_abi(
            input.lir.module().meta.target_profile,
            input.mir.production().strong_callable_bridges(),
            layouts,
            dependencies,
            input.lir.foundation(),
        )
        .unwrap()
        .as_deref(),
        input.lir.initialization_cycle_abi(),
    );
}
