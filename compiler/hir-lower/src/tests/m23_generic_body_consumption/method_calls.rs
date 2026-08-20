use super::*;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-shared-method-calls")
            .join(format!("{name}.scoop")),
    )
    .unwrap()
}

#[test]
fn source_and_decoded_bound_members_keep_common_targets_and_receiver_types() {
    with_provider_consumer(
        &fixture("provider"),
        &fixture("combined"),
        |output, _, _, _, _| {
            let module = output.output().export.module();
            let mut targets = [false; 3];
            let mut interfaces = [false; 2];
            let mut class = false;
            let mut substituted_receiver = false;
            for (_, bound) in module.bound_callable_refs.iter() {
                match bound.declared_callable() {
                    hir::CallableTarget::Local(_) => targets[0] = true,
                    hir::CallableTarget::Application(_) => targets[1] = true,
                    hir::CallableTarget::Dependency(_) => targets[2] = true,
                }
                substituted_receiver |=
                    !matches!(module.types[bound.receiver_type], hir::Type::Param(_));
                match bound.source {
                    hir::BoundCallableSource::Class { .. } => class = true,
                    hir::BoundCallableSource::Interface { member, .. } => match member {
                        hir::InterfaceMethodReference::Local(_) => interfaces[0] = true,
                        hir::InterfaceMethodReference::Imported { .. } => interfaces[1] = true,
                    },
                }
            }
            assert_eq!(
                targets, [true; 3],
                "bound targets retain actual implementation kinds"
            );
            assert_eq!(
                interfaces, [true; 2],
                "source and decoded slots use the same bound records"
            );
            assert!(class, "class bounds retain their declaring application");
            assert!(
                substituted_receiver,
                "default expansion substitutes the receiver type"
            );
        },
    )
    .unwrap_or_else(|errors| panic!("{errors:?}"));
}
