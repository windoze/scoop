use scoop_hir as hir;
use scoop_identity::{CoreBuiltinNominal, ExactTypeKey, PersistentExactTypeId};

mod support;
use support::{fixture, snapshot, with_output};

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
            let mut dump = String::new();
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
                dump.push_str(&format!(
                    "{} {:?} receiver={} arguments={} result={:?} copied={}\n",
                    call.position().expression_index,
                    call.declaration(),
                    call.receiver().has_receiver(),
                    arguments.len(),
                    module.types[call.result_type()].kind,
                    copied,
                ));
            }
            assert_eq!(
                (count, receivers, defaults),
                (expected_calls, expected_receivers, expected_defaults)
            );
            snapshot(&format!("{case}.ordinary-calls.snap"), &dump);
            snapshot(
                &format!("{case}.ordinary-hir.snap"),
                &hir::dump(&output.output().export),
            );
        });
    }
}

#[test]
fn any_dependency_results_require_an_explicit_downcast() {
    with_output("wrong-result", |result| {
        let errors = result
            .err()
            .expect("an Any result cannot implicitly narrow to String");
        assert_eq!(errors.len(), 1, "{errors:?}");
        let error = &errors[0];
        assert!(
            error.message.contains("String") && error.message.contains("Any"),
            "{error:?}"
        );
        let source = std::fs::read_to_string(fixture("wrong-result.scoop")).unwrap();
        let span = error
            .span
            .expect("a type mismatch must retain its source position");
        let at = &source[span.start as usize..span.end as usize];
        snapshot(
            "wrong-result.snap",
            &format!("{}..{} {at:?}: {}\n", span.start, span.end, error.message),
        );
    });
}
