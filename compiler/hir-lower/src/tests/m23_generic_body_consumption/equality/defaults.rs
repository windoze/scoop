use super::*;

#[test]
fn imported_tuple_default_keeps_foreign_origins_for_ordinary_field_calls() {
    with_provider_consumer(
        "package values\npublic fun same(left: (Unit, Unit), right: (Unit, Unit), equal: Boolean = left == right): Boolean = equal",
        "import values.same\npublic fun check(left: (Unit, Unit), right: (Unit, Unit)): Boolean = same(left, right)",
        |output, _, _, _, _| {
            hir::CanonicalHirFoundation::from_dependency_output(&output).unwrap();
            let local = output.output().local.module();
            let calls = output.committed_dependency_call_occurrences().unwrap();
            let fields = calls.iter().filter(|call| {
                matches!(call.position().root.template(),
                    scoop_identity::CallableTemplateOwner::Generated(id)
                    if local.generated_callable_identities.iter().any(|record| {
                        record.id() == id && matches!(record.key(),
                            scoop_identity::GeneratedCallableKey::DerivedEquality { .. })
                    }))
            }).collect::<Vec<_>>();
            assert_eq!(fields.len(), 2);
            for call in fields {
                let hir::CommittedDependencyCallTarget::Direct { callable, .. } = call.target() else {
                    panic!("Unit.equals is an ordinary core member");
                };
                assert_eq!(callable.provider(), scoop_identity::ConeIdentity::CORE);
                let origin = call.origin();
                assert_eq!(origin.evaluation.file, origin.definition.file);
                let source = &output.output().export.source_files[origin.definition.file as usize];
                assert_ne!(source.identity.cone(), local.cone);
            }
        },
    ).unwrap_or_else(|errors| panic!("{errors:?}"));
}
