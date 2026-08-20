use scoop_hir as hir;

mod inheritance;
mod support;
use support::with_output;

#[test]
fn source_call_receivers_survive_adaptation_defaults_and_property_access() {
    for (case, receiver_count, plain_count, adapted_count) in [
        ("standalone", 1, 0, 1),
        ("combined", 3, 1, 2),
        ("generic", 1, 0, 1),
    ] {
        with_output(case, |output, world| {
            let local = output.output().local.module();
            let calls = output.committed_dependency_call_occurrences().unwrap();
            assert_eq!(calls.len(), receiver_count + plain_count);
            let mut receivers = 0;
            let mut plain = 0;
            for call in &calls {
                match call.receiver() {
                    hir::SourceCallReceiver::NoReceiver => plain += 1,
                    hir::SourceCallReceiver::Receiver { static_type } => {
                        receivers += 1;
                        assert_eq!(
                            local.types[static_type].kind,
                            hir::concrete::TypeKind::String
                        );
                        assert_eq!(
                            local.types[call.arguments()[0].ty].kind,
                            hir::concrete::TypeKind::String
                        );
                    }
                }
            }
            assert_eq!((receivers, plain), (receiver_count, plain_count));
            let foundation = hir::CanonicalHirFoundation::from_dependency_output(&output).unwrap();
            let mut authority = hir::CrossConeHirProductionAuthority::new(
                &foundation,
                &output.output().export.public_export_bindings,
                world,
            );
            let interface = hir::CrossConeHirInterfaceSectionV1::from_dependency_hir(
                &output,
                &[],
                &mut authority,
            )
            .unwrap();
            let mut adapted = 0;
            local
                .visit_executable_expressions(|occurrence| {
                    if let hir::concrete::ExprKind::Call {
                        callee: hir::concrete::CallableTarget::Local(_),
                        args,
                        receiver: hir::SourceCallReceiver::Receiver { static_type },
                        ..
                    } = &occurrence.expression.kind
                    {
                        assert_eq!(
                            local.types[*static_type].kind,
                            hir::concrete::TypeKind::String
                        );
                        assert_eq!(local.types[args[0].ty].kind, hir::concrete::TypeKind::Any);
                        adapted += 1;
                    }
                    Ok::<_, std::convert::Infallible>(())
                })
                .unwrap();
            assert_eq!(adapted, adapted_count);

            let mut copied_defaults = 0;
            for reference in interface.external_references().records() {
                for site in reference.call_sites().records() {
                    let call = calls
                        .iter()
                        .find(|call| call.position() == site.position())
                        .unwrap();
                    let expected = call
                        .receiver()
                        .map(|ty| local.exact_type_identities[ty].id());
                    assert_eq!(site.receiver(), expected);
                    if let hir::SourceCallReceiver::Receiver { static_type } = site.receiver() {
                        assert_eq!(static_type, site.arguments()[0]);
                    }
                    assert_eq!(
                        site.arguments(),
                        call.arguments()
                            .iter()
                            .map(|arg| local.exact_type_identities[arg.ty].id())
                            .collect::<Vec<_>>()
                    );
                    copied_defaults += usize::from(
                        call.origin().definition.provider != call.origin().evaluation.provider,
                    );
                }
            }
            assert_eq!(copied_defaults, usize::from(case == "combined"));
        });
    }
}
