use super::*;

pub(super) fn check(
    mir: &mir::CoreBootstrapBridgeSectionV1,
    ordinary: &lir::CrossConeLirBridgeSectionV1,
    lir: &lir::ConeLirOutput,
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
    abi.validate_against(lir.foundation()).unwrap();
}
