use scoop_identity::CallableTemplateOrigin;

use super::support::*;

#[test]
fn same_target_at_different_origins_remains_two_records_in_every_domain() {
    let f = Fixture::new();
    let mut set = full_set(&f);
    macro_rules! add_origin {
        ($field:ident) => {
            let first = &set.$field[0];
            set.$field.push(ProtectedDefaultReferenceV1::new(
                first.target().clone(),
                other_origin(&f),
                first.witness().clone(),
                uses(4),
            ));
            set.$field.reverse();
        };
    }
    add_origin!(callables);
    add_origin!(constructors);
    add_origin!(types);
    add_origin!(globals);
    add_origin!(singleton_values);
    add_origin!(fields);
    let canonical = ProtectedDefaultReferenceSetV1::try_new(
        set.callables,
        set.constructors,
        set.types,
        set.globals,
        set.singleton_values,
        set.fields,
    )
    .unwrap();
    assert_eq!(
        decoded(&canonical).resolve(&mut f.resolver()).unwrap(),
        canonical
    );
    assert_eq!(canonical.callables().len(), 2);
    assert_eq!(canonical.constructors().len(), 2);
    assert_eq!(canonical.types().len(), 2);
    assert_eq!(canonical.globals().len(), 2);
    assert_eq!(canonical.singleton_values().len(), 2);
    assert_eq!(canonical.fields().len(), 2);
    assert_ne!(
        canonical.globals()[0].definition_origin(),
        canonical.globals()[1].definition_origin()
    );
}

#[test]
fn duplicate_keys_are_rejected_even_when_witness_or_uses_differ() {
    let f = Fixture::new();
    for differing_uses in [false, true] {
        let mut set = empty_set();
        let original = record(&f, f.property);
        let changed = ProtectedDefaultReferenceV1::new(
            f.property,
            f.origin(),
            if differing_uses {
                witness(&f)
            } else {
                ProtectedDefaultAccessWitnessV1::generic_source_metadata(
                    CallableTemplateOrigin::Function(f.function),
                )
                .unwrap()
            },
            if differing_uses { uses(9) } else { uses(0) },
        );
        set.globals = vec![original, changed];
        assert!(matches!(
            decoded(&set).resolve(&mut f.resolver()),
            Err(ProtectedDefaultReferenceSetResolutionError::Build(
                ProtectedDefaultReferenceSetBuildError::Duplicate {
                    kind: ProtectedDefaultReferenceKindV1::Global,
                    index: 1,
                }
            ))
        ));
        assert!(matches!(
            ProtectedDefaultReferenceSetV1::try_new(
                vec![],
                vec![],
                vec![],
                set.globals,
                vec![],
                vec![]
            ),
            Err(ProtectedDefaultReferenceSetBuildError::Duplicate {
                kind: ProtectedDefaultReferenceKindV1::Global,
                index: 1,
            })
        ));
    }
}

#[test]
fn reader_rejects_duplicate_and_descending_keys_in_all_six_domains() {
    let f = Fixture::new();
    macro_rules! check {
        ($field:ident, $kind:ident) => {
            for duplicate in [false, true] {
                let mut set = empty_set();
                let full = full_set(&f);
                let first = &full.$field[0];
                let second = ProtectedDefaultReferenceV1::new(
                    first.target().clone(),
                    if duplicate {
                        first.definition_origin().clone()
                    } else {
                        other_origin(&f)
                    },
                    first.witness().clone(),
                    uses(1),
                );
                set.$field = vec![second, first.clone()];
                let error = decoded(&set).resolve(&mut f.resolver()).unwrap_err();
                let ProtectedDefaultReferenceSetResolutionError::Build(error) = error else {
                    panic!("{error:?}");
                };
                assert_eq!(
                    error,
                    if duplicate {
                        ProtectedDefaultReferenceSetBuildError::Duplicate {
                            kind: ProtectedDefaultReferenceKindV1::$kind,
                            index: 1,
                        }
                    } else {
                        ProtectedDefaultReferenceSetBuildError::NonCanonicalOrder {
                            kind: ProtectedDefaultReferenceKindV1::$kind,
                            index: 1,
                        }
                    }
                );
            }
        };
    }
    check!(callables, Callable);
    check!(constructors, Constructor);
    check!(types, Type);
    check!(globals, Global);
    check!(singleton_values, Singleton);
    check!(fields, Field);
}
