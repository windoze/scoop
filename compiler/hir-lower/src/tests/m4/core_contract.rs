use super::*;

// --- negative: the core Option contract ---

fn core_with_option_variants(variants: Vec<VariantDecl>) -> SourceFile {
    let mut core = core_file();
    let option = core
        .declarations
        .iter_mut()
        .find_map(|declaration| match declaration {
            Decl::Enum(declaration) if declaration.name.text == "Option" => Some(declaration),
            _ => None,
        })
        .expect("the test core contains Option");
    option.variants = variants;
    core
}

fn malformed_option_errors(variants: Vec<VariantDecl>) -> Vec<Diagnostic> {
    lower(&[
        core_with_option_variants(variants),
        file(vec![fun("main", Vec::new())]),
    ])
    .expect_err("a malformed Option contract must fail")
}

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

#[test]
fn core_option_requires_exactly_the_some_and_none_variants() {
    let missing = malformed_option_errors(vec![variant_unit("None")]);
    assert!(missing.iter().any(|error| {
        error.message
            == "enum `Option` in scoop.core must define exactly `Some(T)` and `None`, found 1 variant(s)"
    }), "{missing:?}");
    assert!(
        missing.iter().any(|error| {
            error.message == "enum `Option` in scoop.core must define positional variant `Some(T)`"
        }),
        "{missing:?}"
    );

    let extra = malformed_option_errors(vec![
        variant_positional("Some", vec![ty_named("T")]),
        variant_unit("None"),
        variant_unit("Extra"),
    ]);
    assert!(extra.iter().any(|error| {
        error.message
            == "enum `Option` in scoop.core must define exactly `Some(T)` and `None`, found 3 variant(s)"
    }), "{extra:?}");
}

#[test]
fn core_option_requires_the_exact_typed_variant_shapes() {
    let wrong_payload = malformed_option_errors(vec![
        variant_positional("Some", vec![ty_named("Int")]),
        variant_unit("None"),
    ]);
    assert!(
        wrong_payload.iter().any(|error| {
            error.message == "enum `Option` in scoop.core must define positional variant `Some(T)`"
        }),
        "{wrong_payload:?}"
    );

    let named_payload = malformed_option_errors(vec![
        variant_named("Some", vec![("value", ty_named("T"))]),
        variant_unit("None"),
    ]);
    assert!(
        named_payload.iter().any(|error| {
            error.message == "enum `Option` in scoop.core must define positional variant `Some(T)`"
        }),
        "{named_payload:?}"
    );

    let called_none = malformed_option_errors(vec![
        variant_positional("Some", vec![ty_named("T")]),
        variant_positional("None", Vec::new()),
    ]);
    assert!(
        called_none.iter().any(|error| {
            error.message == "enum `Option` in scoop.core must define unit variant `None`"
        }),
        "{called_none:?}"
    );
}
