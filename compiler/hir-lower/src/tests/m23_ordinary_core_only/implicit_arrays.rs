use super::*;

const MESSAGE: &str = "SCOOP_HIR_CROSS_CONE_GENERIC_REQUIRED: implicit array application requires generic/ODR capability from M23-7";

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-implicit-array-capability")
            .join(name),
    )
    .unwrap()
}

fn assert_errors(name: &str, expressions: &[&str]) {
    let source = fixture(name);
    constants::with_input(&source, |input| {
        let errors = lower_current_cone(scoop_identity::RequestedConeKind::Library, input)
            .err()
            .expect("implicit imported array applications need generic capability");
        let expected = expressions
            .iter()
            .flat_map(|expression| {
                source.match_indices(expression).map(move |(start, _)| {
                    scoop_ast::Span::new(start as u32, (start + expression.len()) as u32)
                })
            })
            .collect::<Vec<_>>();
        assert!(!expected.is_empty(), "{name}");
        let actual = errors
            .iter()
            .filter(|error| error.message == MESSAGE && error.file == 0)
            .map(|error| error.span.unwrap())
            .collect::<Vec<_>>();
        assert_eq!(actual.len(), expected.len(), "{name}: {errors:#?}");
        for span in expected {
            assert!(actual.contains(&span), "{name}: {errors:#?}");
        }
    });
}

#[test]
fn imported_array_parameters_report_the_vararg_modifier() {
    for name in ["function.scoop", "class.scoop", "struct.scoop"] {
        assert_errors(name, &["vararg"]);
    }
}

#[test]
fn imported_array_parameters_keep_the_gate_with_binders_defaults_and_variants() {
    assert_errors("combined.scoop", &["vararg"]);
}

#[test]
fn imported_array_literals_report_the_complete_literal() {
    assert_errors("literal.scoop", &["[1, 2, 3]"]);
    assert_errors("empty.scoop", &["[]"]);
}

#[test]
fn imported_array_literals_keep_the_gate_inside_generic_defaults() {
    assert_errors("default.scoop", &["[seed]"]);
}
