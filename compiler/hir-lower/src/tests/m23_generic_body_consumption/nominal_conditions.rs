use super::*;

#[test]
fn dependency_nominal_conditions_reject_invalid_signature_applications() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-shared-nominal-conditions");
    let provider = std::fs::read_to_string(root.join("provider.scoop")).unwrap();
    let mut accepted = Vec::new();
    for case in [
        "bad-struct",
        "bad-enum",
        "bad-pointer",
        "bad-class",
        "bad-interface",
        "bad-nested",
        "bad-body",
        "bad-local-body",
        "bad-wrapper",
        "bad-carrier",
        "bad-alias",
        "bad-pointer-alias",
        "bad-enum-body",
        "bad-root-class",
        "bad-root-enum",
        "bad-default",
    ] {
        let source = std::fs::read_to_string(root.join(format!("{case}.scoop"))).unwrap();
        match with_provider_consumer(&provider, &source, |_, _, _, _, _| ()) {
            Ok(()) => accepted.push(case),
            Err(errors) => {
                assert!(!errors.is_empty(), "{case}");
                assert!(
                    errors
                        .iter()
                        .all(|error| error.file == 0 && error.span.is_some())
                );
                assert!(
                    errors.iter().all(|error| error.message.contains("GC-free")),
                    "{case}: {errors:?}"
                );
            }
        }
    }
    assert!(
        accepted.is_empty(),
        "invalid nominal applications were accepted: {accepted:?}"
    );
}

#[test]
fn dependency_nominal_conditions_accept_gc_free_fields_and_phantom_arguments() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-shared-nominal-conditions");
    let provider = std::fs::read_to_string(root.join("provider.scoop")).unwrap();
    for case in ["standalone", "combined"] {
        let source = std::fs::read_to_string(root.join(format!("{case}.scoop"))).unwrap();
        with_provider_consumer(&provider, &source, |_, _, _, _, _| ())
            .unwrap_or_else(|errors| panic!("{case}: {errors:?}"));
    }
}
