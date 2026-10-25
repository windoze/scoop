use super::*;
use scoop_identity::{
    CallableInstantiationOwner, CallableMaterializationContext, CallableTemplateOwner,
};

const SOURCE: &str = r#"
package shared.defaults

public class Handler public constructor(
    public val action: suspend (Int) -> Int = suspend { value -> value }
)

public fun nested(seed: Int, action: suspend (Int) -> Int = suspend { value ->
    fun keep(input: Int): Int = seed
    val inner: (Int) -> Int = { input -> keep(input) }
    inner(value)
}): suspend (Int) -> Int = action

public fun localHandler(): Handler = Handler()
public fun localNested(): suspend (Int) -> Int = nested(7)
public fun ordinary(): (Int) -> Int = { value -> value }
"#;

#[test]
fn parameter_free_default_helpers_share_local_and_imported_contexts() {
    use super::super::m23_ordinary_core_only::support::parsed_ordinary_text_at;

    let core = trusted_core();
    let coordinate = ConeCoordinate::new("test", "generic-provider", "1.0.0").unwrap();
    let parsed = parsed_ordinary_text_at(&coordinate, SOURCE);
    let world = core.world(parsed.cone());
    let input = CurrentConeSources::try_new(
        &parsed,
        core.foundation.import_core_inputs(&core.interface).unwrap(),
        &world,
    )
    .unwrap();
    let provider = lower_current_cone(scoop_identity::RequestedConeKind::Library, &input).unwrap();
    let local = provider.output().local.module();
    let expected = local
        .functions
        .values()
        .filter(|function| function.name.starts_with("$lambda") || function.name.ends_with("keep"))
        .filter(|function| {
            matches!(
                function.materialization.context(),
                CallableMaterializationContext::Application(_)
            )
        })
        .map(|function| function.materialization)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        expected.len(),
        4,
        "default lambda, nested lambda, local function, constructor lambda"
    );
    for name in ["nested", "localHandler", "localNested", "ordinary"] {
        let function = local
            .functions
            .values()
            .find(|function| function.name == name)
            .unwrap();
        assert_eq!(
            function.materialization.context(),
            CallableMaterializationContext::NoSubstitution
        );
    }
    assert!(
        local.functions.values().any(|function| {
            matches!(
                function.materialization.template(),
                CallableTemplateOwner::Generated(_)
            ) && function.materialization.context()
                == CallableMaterializationContext::NoSubstitution
        }),
        "an ordinary body lambda keeps its original Strong owner"
    );

    with_provider_consumer(
        SOURCE,
        r#"
        package consumer
        import shared.defaults.*
        public fun handler(): Handler = Handler()
        public fun callback(): suspend (Int) -> Int = nested(9)
    "#,
        |output, _, _, _, _| {
            let local = output.output().local.module();
            let actual = local
                .functions
                .values()
                .filter(|function| {
                    function.name.starts_with("$lambda") || function.name.ends_with("keep")
                })
                .filter(|function| {
                    matches!(
                        function.materialization.context(),
                        CallableMaterializationContext::Application(_)
                    )
                })
                .map(|function| function.materialization)
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(actual, expected);
            assert!(local.callable_applications.records().iter().any(|record| {
                record.key().instantiation_owner() == CallableInstantiationOwner::NoOwner
                    && !record.key().callable_arguments().has_arguments()
            }));
            hir::CanonicalHirFoundation::from_dependency_output(&output).unwrap();
        },
    )
    .unwrap();
}
