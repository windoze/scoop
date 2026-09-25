use super::*;
mod corruption;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/local-own-binders.scoop"
));
const COMBINATIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/local-own-binder-combinations.scoop"
));

#[test]
fn local_generic_signatures_keep_their_own_frame_and_shift_all_provider_frames() {
    for source in [SOURCE, COMBINATIONS] {
        with_sources(source, |output, fixture, sources, core| {
            let table = templates(output);
            let foundation = fixture.bind().unwrap();
            sources.with_bound(&foundation, core, |members, constructors| {
                let parameters = members
                    .bind_parameter_protocols(constructors, &sources.protocols)
                    .unwrap();
                let bound = parameters.bind_default_declarations(&table, &[]).unwrap();
                assert!(!bound.declarations().is_empty());
                for contract in bound.declarations() {
                    let template = table.get(contract.key()).unwrap();
                    let occurrences = contract.nested_callables().occurrences();
                    assert_eq!(occurrences.len(), 1);
                    let descriptor = occurrences[0].descriptor();
                    let SignatureTypeKey::Function {
                        parameters, result, ..
                    } = descriptor.function_type()
                    else {
                        panic!("a local signature has function kind")
                    };
                    for (index, parameter) in parameters.iter().enumerate() {
                        assert_eq!(
                            parameter,
                            &SignatureTypeKey::Binder {
                                depth: 0,
                                index: index as u32
                            }
                        );
                    }
                    match result.as_ref() {
                        SignatureTypeKey::Binder { depth, .. } => assert_eq!(*depth, 1),
                        SignatureTypeKey::Tuple(elements) => assert_eq!(
                            elements.as_slice(),
                            &[
                                SignatureTypeKey::Binder { depth: 2, index: 0 },
                                SignatureTypeKey::Binder { depth: 1, index: 1 },
                            ]
                        ),
                        other => panic!("unexpected local result: {other:?}"),
                    }
                    assert!(
                        template
                            .references()
                            .types()
                            .iter()
                            .any(|reference| reference.target() == descriptor.function_type())
                    );
                }
            });
        });
    }
}
