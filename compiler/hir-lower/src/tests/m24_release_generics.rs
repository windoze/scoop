use super::m23_generic_body_consumption::with_provider_consumer;

const PROVIDER: &str = r#"
    @NoGC private fun <T> keep(value: T): T = value
    @NoGC private fun number(value: Int): Int = value
    public class Owner<T> public constructor(private val value: T) {
        release {
            val copy = keep(value)
            val local = number(1)
        }
    }
"#;

#[test]
fn generic_release_imports_the_original_nominal_body_and_helper() {
    with_provider_consumer(
        PROVIDER,
        "public fun make(): Owner<Int> = Owner(42)",
        |output, _, _, interface, _| {
            let export = output.output().export.module();
            assert!(!output.executable_dependency_callables().unwrap().is_empty());
            assert!(
                output
                    .committed_dependency_call_occurrences()
                    .unwrap()
                    .iter()
                    .any(|call| {
                        matches!(
                            call.position().root.template(),
                            scoop_identity::CallableTemplateOwner::ReleaseHook(_)
                        )
                    })
            );
            assert!(
                export
                    .classes
                    .iter()
                    .all(|(_, declaration)| declaration.name != "Owner")
            );
            let template = interface
                .generic_initializations()
                .records()
                .iter()
                .find(|template| template.release_policy().hook().is_some())
                .unwrap();
            let hook = template.release_policy().hook().unwrap();
            assert!(hook.body().results().is_empty());
            assert!(hook.body().locals().records().iter().all(|local| !matches!(
                local.selector(),
                scoop_identity::LocalValueSelector::This
                    | scoop_identity::LocalValueSelector::Parameter { .. }
            )));
            let local = output.output().local.module();
            assert_eq!(local.release_hooks.len(), 1);
            let (_, hook) = local.release_hooks.iter().next().unwrap();
            let owner = local.exact_type_identities[local.classes[hook.owner].canonical_type].id();
            assert_eq!(
                hook.materialization.template(),
                scoop_identity::CallableTemplateOwner::ReleaseHook(owner)
            );
            assert!(
                local
                    .functions
                    .iter()
                    .any(|(_, function)| function.name == "keep")
            );
            assert_eq!(
                export
                    .release_hooks
                    .iter()
                    .next()
                    .unwrap()
                    .1
                    .requirements
                    .len(),
                1
            );
        },
    )
    .unwrap();
}

#[test]
fn generic_release_conditions_apply_to_unused_dependency_signatures() {
    let errors = with_provider_consumer(
        PROVIDER,
        "public fun <U> onlyType(value: Owner<String>) {}",
        |_, _, _, _, _| (),
    )
    .unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("requires ReleaseValue")),
        "{errors:#?}"
    );
    with_provider_consumer(
        PROVIDER,
        "public fun <U> onlyType(value: Owner<Int>) {}",
        |output, _, _, _, _| {
            assert!(output.output().local.module().release_hooks.is_empty());
        },
    )
    .unwrap();
}
