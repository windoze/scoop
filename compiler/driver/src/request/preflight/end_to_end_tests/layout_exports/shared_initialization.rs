use super::*;

pub(super) fn check(
    name: &str,
    mir: &mir::CoreBootstrapBridgeSectionV1,
    ordinary: &lir::CrossConeLirBridgeSectionV1,
    lir: &lir::SingleConeStrongLirOutput,
    strong: &lir::StrongProductionSectionV2,
) {
    let source = mir
        .strong_callable_bridges()
        .initialization_cycle()
        .unwrap();
    let scoop_identity::CallableOwner::Function(function) = source.implementation() else {
        panic!("initialization service is a source function")
    };
    let export = ordinary
        .export(scoop_identity::DependencyCallableDeclarationId::Function(
            function,
        ))
        .expect("internal initialization service has an ordinary callable export");
    let abi = export.callable_abi();
    assert_eq!(abi.abi_signature().signature(), source.signature());
    assert_eq!(
        abi.root_plan(),
        lir::ExternalCallableRootPlan::ManagedStatepoint
    );
    abi.validate_against(lir.foundation(), strong.canonical_definitions())
        .unwrap();
    if name.starts_with("shared-init-abi-") {
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
