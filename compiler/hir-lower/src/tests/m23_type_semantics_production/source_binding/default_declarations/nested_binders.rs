use super::*;
use hir::{
    DefaultSourceNestedCallableDescriptorV1 as Descriptor,
    DefaultSourceNestedIdentityFailureV1 as Failure,
};
use scoop_wire::WirePath;

mod corruption;
const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/nested-owner-binders.scoop"
));
const COMBINATIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/nested-owner-binder-combinations.scoop"
));

#[test]
fn nested_owner_binders_follow_canonical_parents_and_static_nested_boundaries() {
    with_sources(SOURCE, |output, fixture, sources, core| {
        let table = templates(output);
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let parameters = members
                .bind_parameter_protocols(constructors, &sources.protocols)
                .unwrap();
            let bound = parameters.bind_default_declarations(&table, &[]).unwrap();
            let mut snapshot = String::new();
            for (name, expected) in [
                ("BinderHost.lambda", 4),
                ("BinderHost.anonymous", 4),
                ("BinderHost.reference", 4),
                ("BinderHost.local", 4),
                ("BinderHost.Plain.callback", 1),
                ("BinderHost.Generic.callback", 2),
            ] {
                let contract = bound.declaration(key(output, name, 0)).unwrap();
                let occurrences = contract.nested_callables().occurrences();
                assert_eq!(occurrences.len(), 1, "{name}");
                let descriptor = occurrences[0].descriptor();
                assert_eq!(descriptor.owner_type_parameter_count(), expected, "{name}");
                assert!(descriptor.captures().is_empty());
                snapshot.push_str(&format!(
                    "{name}: {:?}, owner binders {expected}, captures 0\n",
                    descriptor.identity().kind()
                ));
            }
            for constructor in [false, true] {
                let contract = bound
                    .declarations()
                    .iter()
                    .find(|d| {
                        if constructor {
                            matches!(d.key().owner(), CallableTemplateOrigin::Constructor(_))
                        } else {
                            matches!(
                                d.key().owner(),
                                CallableTemplateOrigin::VariantConstructor(_)
                            )
                        }
                    })
                    .unwrap();
                let occurrences = contract.nested_callables().occurrences();
                assert_eq!(occurrences.len(), 1);
                assert_eq!(occurrences[0].descriptor().owner_type_parameter_count(), 1);
            }
            assert_eq!(
                snapshot,
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../tests/fixtures/m23-type-source-defaults/nested-owner-binders.snap"
                ))
            );
        });
    });
}

#[test]
fn expanded_nested_binders_keep_the_original_unused_and_local_parent_parameters() {
    with_sources(COMBINATIONS, |output, fixture, sources, core| {
        let table = templates(output);
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let parameters = members.bind_parameter_protocols(constructors, &sources.protocols).unwrap();
            let bound = parameters.bind_default_declarations(&table, &[]).unwrap();
            for name in ["BinderCombinationHost.expanded", "BinderCombinationHost.anonymous"] {
                let contract = bound.declaration(key(output, name, 0)).unwrap();
                assert_eq!(contract.provider_binders().binder_arity(), 2);
                let occurrences = contract.nested_callables().occurrences();
                assert_eq!(occurrences.len(), 1);
                let descriptor = occurrences[0].descriptor();
                assert_eq!(descriptor.owner_type_parameter_count(), 3);
                assert!(descriptor.captures().is_empty());
                assert!(matches!(descriptor.body_arguments(), hir::DefaultNestedCallableBodyArgumentsV1::Explicit(args) if args.len() == 3));
            }
            let contract = bound.declaration(key(output, "BinderCombinationHost.local", 0)).unwrap();
            let counts = contract.nested_callables().occurrences().iter()
                .map(|o| o.descriptor().owner_type_parameter_count()).collect::<Vec<_>>();
            assert_eq!(counts, [2, 3, 2]);
        });
    });
}
