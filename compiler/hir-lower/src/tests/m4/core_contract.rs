use super::*;

// --- negative: the core Option contract ---

#[test]
fn missing_core_option_is_an_error() {
    // A user file alone (no core) has no `Option<T>`.
    let errors =
        lower(&[file(vec![fun("main", vec![])])]).expect_err("missing core Option must fail");
    // The same run also reports the missing core `Throwable` (M8).
    assert!(
        errors
            .iter()
            .any(|e| e.message == "scoop.core must define an enum `Option<T>`" && e.file == 0),
        "{errors:?}"
    );
}

#[test]
fn core_option_must_have_one_type_parameter() {
    for type_params in [vec![], vec!["T", "U"]] {
        let core = file(vec![enum_decl(
            "Option",
            type_params.clone(),
            vec![variant_unit("None")],
        )]);
        let errors = lower(&[core, file(vec![fun("main", vec![])])])
            .expect_err("wrong Option arity must fail");
        let count = type_params.len();
        assert!(
            errors.iter().any(|e| e.message
                == format!(
                    "enum `Option` in scoop.core must have exactly one type parameter, found {count}"
                )),
            "{errors:?}"
        );
    }
}

#[test]
fn duplicate_option_in_core_is_an_error() {
    let option = || {
        enum_decl(
            "Option",
            vec!["T"],
            vec![
                variant_positional("Some", vec![ty_named("T")]),
                variant_unit("None"),
            ],
        )
    };
    let errors = lower(&[
        file(vec![option()]),
        file(vec![option()]),
        file(vec![fun("main", vec![])]),
    ])
    .expect_err("a second core Option must fail");
    // The same run also reports the missing core `Throwable` (M8);
    // this test is about the duplicate's file index.
    let duplicate = errors
        .iter()
        .find(|e| e.message == "duplicate enum `Option`")
        .expect("the second core Option must be diagnosed");
    // The second core file is the error's file.
    assert_eq!(duplicate.file, 1);
}
