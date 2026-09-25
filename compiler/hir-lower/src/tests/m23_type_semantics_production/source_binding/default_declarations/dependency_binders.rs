use super::*;

mod identity;

pub(super) const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/local-dependency-binders.scoop"
));
pub(super) const COMBINATIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/local-dependency-combinations.scoop"
));

#[test]
fn default_local_dependencies_project_the_expanded_provider_binders() {
    for (source, expected_captures) in [(SOURCE, 1), (COMBINATIONS, 11)] {
        with_sources(source, |output, fixture, sources, core| {
            let table = templates(output);
            let foundation = fixture.bind().unwrap();
            sources.with_bound(&foundation, core, |members, constructors| {
                let parameters = members
                    .bind_parameter_protocols(constructors, &sources.protocols)
                    .unwrap();
                let bound = parameters.bind_default_declarations(&table, &[]).unwrap();
                let mut captures = 0;
                for contract in bound.declarations() {
                    let template = table.get(contract.key()).unwrap();
                    let expected = SignatureTypeKey::Binder {
                        depth: 0,
                        index: contract.provider_binders().callable_own_binder_arity() - 1,
                    };
                    for occurrence in contract.nested_callables().occurrences() {
                        for capture in occurrence.descriptor().captures() {
                            captures += 1;
                            let local = template
                                .locals()
                                .records()
                                .iter()
                                .find(|local| local.selector() == capture.source())
                                .unwrap();
                            assert_eq!(local.value_type(), capture.value_type());
                            assert_eq!(capture.value_type(), &expected);
                        }
                    }
                }
                assert_eq!(captures, expected_captures);
            });
        });
    }
}
