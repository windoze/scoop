use super::*;

#[test]
fn dependency_value_wrappers_reject_inline_cycles() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-shared-value-layouts");
    let provider = std::fs::read_to_string(root.join("provider.scoop")).unwrap();
    for (case, path) in [
        ("struct-cycle", "Recursive -> Recursive"),
        ("enum-cycle", "Recursive -> Recursive"),
        ("mixed-cycle", "Left -> Right -> Left"),
        ("growing-cycle", "Grow -> Grow"),
    ] {
        let source = std::fs::read_to_string(root.join(format!("{case}.scoop"))).unwrap();
        let errors = with_provider_consumer(&provider, &source, |_, _, _, _, _| ())
            .expect_err("an imported wrapper cannot hide a recursive inline layout");
        assert_eq!(errors.len(), 1, "{case}: {errors:?}");
        assert_eq!(errors[0].file, 0);
        assert!(errors[0].span.is_some());
        assert_eq!(
            errors[0].message,
            format!("recursive value layout does not cross a reference boundary: {path}"),
            "{case}",
        );
    }
}
