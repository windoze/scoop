use super::{hir, support::with_sources};

#[test]
fn imported_default_adapts_an_inherited_member_receiver() {
    let provider =
        scoop_identity::ConeCoordinate::new("test", "default-receiver-provider", "1.0.0").unwrap();
    let provider_source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-cli-default-receivers/inherited/provider/src/main.scoop"
    ));
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-cli-default-receivers/inherited/runtime/src/probe.scoop"
    ));
    with_sources(&provider, provider_source, source, |output, _| {
        let local = output.output().local.module();
        let calls = output.committed_dependency_call_occurrences().unwrap();
        assert_eq!(calls.len(), 2);
        let call = calls
            .iter()
            .find(|call| matches!(call.receiver(), hir::SourceCallReceiver::Receiver { .. }))
            .unwrap();
        let hir::SourceCallReceiver::Receiver { static_type } = call.receiver() else {
            unreachable!()
        };
        let hir::concrete::TypeKind::Class(child) = local.types[static_type].kind else {
            panic!("the source receiver retains its class type")
        };
        assert_eq!(local.classes[child].name, "Child");
        let argument = &call.arguments()[0];
        let hir::concrete::TypeKind::Class(parent) = local.types[argument.ty].kind else {
            panic!("the call argument retains its declaring class type")
        };
        assert_eq!(local.classes[parent].name, "Parent");
        let hir::concrete::ExprKind::ReferenceUpcast(value) = &argument.kind else {
            panic!("the inherited receiver requires a reference upcast")
        };
        assert_eq!(value.ty, static_type);
        assert_ne!(
            call.origin().definition.provider,
            call.origin().evaluation.provider
        );
    });
}
