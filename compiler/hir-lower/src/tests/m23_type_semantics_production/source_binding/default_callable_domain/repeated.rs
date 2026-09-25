use super::*;

#[test]
fn callable_domains_keep_each_substituted_occurrence_of_the_same_nested_identity() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-defaults/callable-domain-binding-repeated.scoop"
    ));
    with_inputs(source, |inputs| {
        inputs.with_bound(|domains, parameters, _| {
            let declarations = parameters
                .bind_default_declarations(&inputs.templates, &[])
                .unwrap();
            domains
                .bind_nominal_default_callable_domains(&declarations)
                .unwrap();
            for position in [2, 3] {
                let template = support::named(inputs, "Repeated.pair", position);
                let declaration = declarations.declaration(template.key()).unwrap();
                let nested = declaration.nested_callables().occurrences();
                let [first, second] = nested else {
                    panic!("two source expansions: {nested:?}")
                };
                assert_eq!(
                    first.descriptor().identity(),
                    second.descriptor().identity()
                );
                assert_ne!(first.site(), second.site());
                assert_ne!(
                    first.descriptor().function_type(),
                    second.descriptor().function_type()
                );
                assert!(matches!(
                    (position, first.descriptor().identity()),
                    (2, hir::DefaultNestedCallableIdentityV1::Lambda(_))
                        | (
                            3,
                            hir::DefaultNestedCallableIdentityV1::CallableReference(_)
                        )
                ));
                let mut checked = 0;
                for occurrence in declaration.references().occurrences() {
                    let Reference::Callable(reference) = occurrence.source() else {
                        continue;
                    };
                    if matches!(
                        reference.target(),
                        Target::Lambda { .. } | Target::CallableReference { .. }
                    ) {
                        assert_eq!(
                            reference.witness().target_domain(),
                            &hir::DefaultSourceAccessDomainV1::universal()
                        );
                        checked += 1;
                    }
                }
                assert_eq!(checked, 2);
            }
        });
    });
}
