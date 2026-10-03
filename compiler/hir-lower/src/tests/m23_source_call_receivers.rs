use scoop_hir as hir;

mod inheritance;
mod support;
use support::{fixture, with_output};

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
            let mut dump = String::new();
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
                        dump.push_str(&format!(
                            "{} local receiver=String argument=Any\n",
                            occurrence.position.expression_index
                        ));
                    }
                    Ok::<_, std::convert::Infallible>(())
                })
                .unwrap();
            assert_eq!(adapted, adapted_count);

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
                    let source = match site.receiver() {
                        hir::SourceCallReceiver::NoReceiver => "absent",
                        hir::SourceCallReceiver::Receiver { static_type } => {
                            assert_eq!(static_type, site.arguments()[0]);
                            "String"
                        }
                    };
                    let arguments = call
                        .arguments()
                        .iter()
                        .map(|arg| format!("{:?}", local.types[arg.ty].kind))
                        .collect::<Vec<_>>();
                    let copied =
                        call.origin().definition.provider != call.origin().evaluation.provider;
                    dump.push_str(&format!(
                        "{} receiver={source} args={arguments:?} provider_default={copied}\n",
                        site.position().expression_index
                    ));
                }
            }
            let mut lines = dump.lines().collect::<Vec<_>>();
            lines.sort_unstable();
            let dump = format!("{}\n", lines.join("\n"));
            let path = fixture(&format!("{case}.snap"));
            if std::env::var_os("SCOOP_UPDATE_SOURCE_RECEIVERS").is_some() {
                std::fs::write(path, dump).unwrap();
            } else {
                assert_eq!(dump, std::fs::read_to_string(path).unwrap());
            }
        });
    }
}
