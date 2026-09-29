use super::*;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-shared-default-bodies")
            .join(format!("{name}.scoop")),
    )
    .unwrap()
}

#[test]
fn loaded_default_closures_keep_original_captures_across_applications() {
    with_provider_consumer(
        &fixture("provider"),
        &fixture("functions"),
        |output, _, _, _, _| {
            let local = output.output().local.module();
            assert!(
                local
                    .functions
                    .values()
                    .any(|function| function.name == "$lambda")
            );
            assert!(
                local
                    .functions
                    .values()
                    .any(|function| function.name == "keep")
            );
        },
    )
    .unwrap();
}

#[test]
fn loaded_constructor_and_method_defaults_share_receiver_substitution() {
    with_provider_consumer(
        &fixture("provider"),
        &fixture("methods"),
        |_, _, _, _, _| {},
    )
    .unwrap();
}
