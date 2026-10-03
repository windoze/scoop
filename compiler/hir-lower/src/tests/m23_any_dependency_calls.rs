use scoop_hir as hir;
use scoop_identity::{CoreBuiltinNominal, ExactTypeKey, PersistentExactTypeId};

mod support;
use support::with_output;

#[test]
fn any_dependency_calls_preserve_signatures_receivers_accessors_and_defaults() {
    let any = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        CoreBuiltinNominal::Any.identity_record().id(),
    ))
    .unwrap();
    for (case, expected_calls, expected_receivers, expected_defaults) in
        [("standalone", 1, 0, 0), ("combined", 9, 4, 1)]
    {
        with_output(case, |result| {
            let output = result.unwrap_or_else(|errors| panic!("{case}: {errors:?}"));
            let module = output.output().local.module();
            let mut count = 0;
            let mut receivers = 0;
            let mut defaults = 0;
            for call in output.committed_dependency_call_occurrences().unwrap() {
                let hir::CommittedDependencyCallTarget::Direct { callable, .. } = call.target()
                else {
                    panic!("the Any fixture uses parameter-free dependency callables");
                };
                let signature = callable.capability().signature();
                let arguments = signature
                    .receiver()
                    .into_option()
                    .into_iter()
                    .chain(signature.parameters().iter().copied())
                    .collect::<Vec<_>>();
                if !arguments.contains(&any) && signature.result() != any {
                    continue;
                }
                count += 1;
                assert_ne!(callable.provider(), scoop_identity::ConeIdentity::CORE);
                assert_eq!(arguments.len(), call.arguments().len());
                for (argument, expected) in call.arguments().iter().zip(&arguments) {
                    assert_eq!(module.exact_type_identities[argument.ty].id(), *expected);
                }
                assert_eq!(
                    module.exact_type_identities[call.result_type()].id(),
                    signature.result()
                );
                if let hir::SourceCallReceiver::Receiver { static_type } = call.receiver() {
                    receivers += 1;
                    assert_eq!(
                        module.types[static_type].kind,
                        hir::concrete::TypeKind::String
                    );
                    assert_eq!(arguments[0], any);
                    assert_ne!(module.exact_type_identities[static_type].id(), any);
                }
                let copied = call.origin().definition.provider != call.origin().evaluation.provider;
                defaults += usize::from(copied);
            }
            assert_eq!(
                (count, receivers, defaults),
                (expected_calls, expected_receivers, expected_defaults)
            );
        });
    }
}
